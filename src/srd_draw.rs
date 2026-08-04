use std::fmt;

use crate::image::{ImageDefinition, ImageReferenceChannel, SrdTextureBindingSource};
use crate::projection::{
    Matrix4x4, identity_matrix4x4_game, inverse_matrix4x4_game, mul_matrix4x4_game,
};
use crate::reference_runtime::{ProjectLayerRuntimeState, ProjectRuntime};
use crate::render::{
    CeylonDepthState, CeylonDrawPacketPresetState, CeylonRasterState,
    CeylonSrdFixedShaderConstants, SrdD3d9BlendPreset, SrdQuadDraw,
    apply_srd_image_alpha_stencil_packet_fields, apply_srd_image_field_0c_shader_bits,
    apply_srd_special_depth_packet_fields, ceylon_d3d9_blend_preset,
    select_srd_image_render_preset,
};
use crate::scene::{Layer, Project};
use crate::shader::CEYLON_SIMPLE_SHADER_KEY_LENGTH;
use crate::shader_bytecode::embedded_simple_shader_pair;
use crate::texture::{TextureList, TextureSamplerState};
use crate::transform::{Affine3x4, SpatialTransform};
use crate::{csli::add_color_saturating_game, csli::multiply_color_game};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrdDrawError(pub String);

impl fmt::Display for SrdDrawError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SrdDrawError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvidenceCompleteSrdDraw {
    pub scene_index: usize,
    pub layer_index: usize,
    pub node_index: usize,
    pub is_2d: bool,
    pub shader_key: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH],
    pub quad: SrdQuadDraw,
    pub packet: CeylonDrawPacketPresetState,
    pub fixed_constants: CeylonSrdFixedShaderConstants,
    pub blend: SrdD3d9BlendPreset,
    pub raster: CeylonRasterState,
    pub depth: CeylonDepthState,
    pub texture_bindings: [Option<EvidenceSrdTextureBinding>; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceSrdTextureBinding {
    pub texture_index: usize,
    pub sampler: TextureSamplerState,
}

/// Inputs owned by the scene/target hosting an SrPlayer, rather than by the
/// SRD file itself. There is deliberately no `Default`: an independent SRD
/// does not identify a unique game target, Camera, or scene-node placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrdHostDrawContext {
    pub first_calc_matrix: Affine3x4,
    pub target_projection_view: Matrix4x4,
    pub target_render_size: [u32; 2],
    pub target_screen_size: [u32; 2],
}

impl SrdHostDrawContext {
    pub const fn new(
        first_calc_matrix: Affine3x4,
        target_projection_view: Matrix4x4,
        target_render_size: [u32; 2],
        target_screen_size: [u32; 2],
    ) -> Self {
        Self {
            first_calc_matrix,
            target_projection_view,
            target_render_size,
            target_screen_size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct InitialWorldColorState {
    multiply: [u8; 4],
    additive: [u8; 4],
    visible: bool,
}

/// Builds only the initial ImageCast subset whose complete Simple shader pair
/// and packet/device inputs are proven. TEXT, explicit texture overrides,
/// special CAST matrix branches, alpha-test/stencil base contexts and
/// unsupported shader keys are rejected or excluded explicitly.
pub fn build_evidence_complete_initial_image_draws(
    project: &Project,
    textures: &TextureList,
    scene_index: usize,
    host: SrdHostDrawContext,
) -> Result<Vec<EvidenceCompleteSrdDraw>, SrdDrawError> {
    build_evidence_complete_image_draws(project, textures, scene_index, host, None)
}

pub fn build_evidence_complete_animation_set_image_draws(
    project: &Project,
    textures: &TextureList,
    scene_index: usize,
    animation_set_index: usize,
    frame: f32,
    host: SrdHostDrawContext,
) -> Result<Vec<EvidenceCompleteSrdDraw>, SrdDrawError> {
    let mut runtime = ProjectRuntime::new(project)
        .map_err(|error| SrdDrawError(format!("failed to construct SRD runtime: {error}")))?;
    runtime
        .apply_animation_set(project, textures, scene_index, animation_set_index, frame)
        .map_err(|error| SrdDrawError(error.to_string()))?;
    let runtime_layers = runtime
        .project_layers
        .get(scene_index)
        .ok_or_else(|| SrdDrawError(format!("scene index {scene_index} is outside the runtime")))?;
    build_evidence_complete_image_draws(project, textures, scene_index, host, Some(runtime_layers))
}

fn build_evidence_complete_image_draws(
    project: &Project,
    textures: &TextureList,
    scene_index: usize,
    host: SrdHostDrawContext,
    runtime_layers: Option<&[ProjectLayerRuntimeState]>,
) -> Result<Vec<EvidenceCompleteSrdDraw>, SrdDrawError> {
    if host.target_render_size.contains(&0) {
        return Err(SrdDrawError(format!(
            "target render size must be non-zero, got {}x{}",
            host.target_render_size[0], host.target_render_size[1]
        )));
    }
    if host.target_screen_size.contains(&0) {
        return Err(SrdDrawError(format!(
            "target screen size must be non-zero, got {}x{}",
            host.target_screen_size[0], host.target_screen_size[1]
        )));
    }
    let scene = project
        .scenes
        .get(scene_index)
        .ok_or_else(|| SrdDrawError(format!("scene index {scene_index} is outside the project")))?;
    let external_inverse = inverse_matrix4x4_game(&host.target_projection_view);
    let srd_projection_view = project
        .camera
        .runtime_matrices(host.target_render_size[0] as f32)
        .projection_view;
    let camera_bridge = mul_matrix4x4_game(&external_inverse, &srd_projection_view);
    let identity = identity_matrix4x4_game();
    let mut packet_current_matrix = identity;
    let mut draws = Vec::new();

    for (layer_index, layer) in scene.layers.iter().enumerate() {
        let runtime_layer = runtime_layers
            .map(|layers| {
                layers.get(layer_index).ok_or_else(|| {
                    SrdDrawError(format!(
                        "SCN[{scene_index}]/LAYR[{layer_index}] is missing from the runtime"
                    ))
                })
            })
            .transpose()?;
        let layer_enabled = runtime_layer
            .map(|runtime_layer| runtime_layer.enabled)
            .unwrap_or(layer.flags & 0x100 != 0);
        if !layer_enabled {
            continue;
        }
        let is_2d = layer.is_2d();
        let transforms = runtime_layer.map_or_else(
            || {
                layer
                    .transforms
                    .iter()
                    .copied()
                    .map(|transform| transform.spatial())
                    .collect::<Vec<_>>()
            },
            |runtime_layer| runtime_layer.cast_transforms.clone(),
        );
        if reject_special_matrix_branches(layer).is_err() {
            continue;
        }
        let world_matrices = layer
            .compose_world_matrices_with_csli_layout(&transforms, host.first_calc_matrix, false)
            .map_err(|error| SrdDrawError(error.to_string()))?;
        let world_colors = compose_initial_world_colors(layer, &transforms, layer_enabled)?;

        for node_index in 0..layer.nodes.len() {
            let Some(image) = layer.image_by_node[node_index]
                .as_ref()
                .filter(|image| !image.creates_text_cast())
            else {
                continue;
            };
            let world_color = world_colors[node_index];
            if !world_color.visible {
                continue;
            }

            let mut fixed_constants = CeylonSrdFixedShaderConstants::initial_for_target(
                host.target_projection_view,
                host.target_screen_size,
            );
            if is_2d {
                fixed_constants.vertex_c0_c3_world = identity;
                fixed_constants.vertex_c4_c7 = identity;
                packet_current_matrix = identity;
            } else {
                fixed_constants.vertex_c4_c7 = packet_current_matrix;
                fixed_constants.vertex_c0_c3_world = camera_bridge;
                packet_current_matrix = camera_bridge;
            }

            let image_state = runtime_layer.map_or_else(
                || {
                    let mut image_state = image.initial_runtime_state();
                    if let Some(ext_param) = layer.ext_param_for_node(node_index) {
                        image_state.render_preset_override = ext_param.render_preset_override;
                    }
                    image_state
                },
                |runtime_layer| runtime_layer.image_states[node_index],
            );
            let preset = select_srd_image_render_preset(
                image.flags,
                image_state.render_preset_override,
                false,
            )
            .ok_or_else(|| {
                SrdDrawError(format!(
                    "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] does not select a proven render preset"
                ))
            })?;
            let slots = image
                .resolve_texture_slots(
                    &image_state,
                    textures,
                    ImageDefinition::INITIAL_COORDINATE_OFFSET_SCALE,
                    [false; 2],
                )
                .map_err(|error| SrdDrawError(error.to_string()))?;
            let mut texture_bindings = [None; 3];
            for (slot_index, destination) in texture_bindings.iter_mut().enumerate().take(2) {
                match slots.slots[slot_index] {
                    Some(SrdTextureBindingSource::TextureList(texture_index)) => {
                        let sampler = slots.channels[slot_index].selected_sampler.ok_or_else(|| {
                            SrdDrawError(format!(
                                "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] texture slot {slot_index} has no proven sampler"
                            ))
                        })?;
                        *destination = Some(EvidenceSrdTextureBinding {
                            texture_index,
                            sampler,
                        });
                    }
                    Some(SrdTextureBindingSource::ExplicitOverride) => continue,
                    None => {}
                }
            }
            if slots
                .slots
                .iter()
                .any(|slot| matches!(slot, Some(SrdTextureBindingSource::ExplicitOverride)))
            {
                continue;
            }

            let mut packet = CeylonDrawPacketPresetState::srd_renderer_initial();
            packet.set_render_preset_id(preset);
            packet.set_srd_quad_is_2d(is_2d);
            apply_srd_image_field_0c_shader_bits(&mut packet, image_state.field_0c as i32);
            let mut renderer_counter = 0u8;
            apply_srd_image_alpha_stencil_packet_fields(
                &mut packet,
                image_state.field_10,
                image_state.field_14,
                image_state.field_18,
                0,
                &mut renderer_counter,
            );
            apply_srd_special_depth_packet_fields(&mut packet, false, image_state.field_1c);

            let shader_key = packet
                .srd_quad_shader_key(slots.texture_present())
                .srd_simple_shader_direct_contributions()
                .map_err(|error| SrdDrawError(format!("unsupported Simple mapping: {error:?}")))?
                .compact_key();
            if embedded_simple_shader_pair(&shader_key).is_none() {
                continue;
            }
            let blend = ceylon_d3d9_blend_preset(i32::from(packet.table_preset_id()));
            if blend.alpha_test_enabled || packet.flags_0c & 0x100 != 0 {
                continue;
            }

            let local_positions = image
                .build_quad_with_geometry(image_state.geometry, is_2d)
                .positions;
            let positions =
                local_positions.map(|point| world_matrices[node_index].transform_point_game(point));
            let quad = image.build_render_quad_from_positions(
                positions,
                image_state.coordinate_state(ImageReferenceChannel::Cref),
                slots.channels[0],
                slots.channels[1],
                world_color.multiply,
                world_color.additive,
            );
            let mut raster = CeylonRasterState::default();
            raster.apply_draw_packet(packet);
            draws.push(EvidenceCompleteSrdDraw {
                scene_index,
                layer_index,
                node_index,
                is_2d,
                shader_key,
                quad,
                packet,
                fixed_constants,
                blend,
                raster,
                depth: CeylonDepthState::from_draw_flags(packet.draw_flags_00),
                texture_bindings,
            });
        }
    }
    Ok(draws)
}

fn reject_special_matrix_branches(layer: &Layer) -> Result<(), SrdDrawError> {
    for (node_index, node) in layer.nodes.iter().enumerate() {
        let flags = node.type_flags.unwrap_or(0);
        if flags & 0x0007_0000 != 0 {
            return Err(SrdDrawError(format!(
                "NODE[{node_index}] uses unimplemented CAST matrix flags {:#x}",
                flags & 0x0007_0000
            )));
        }
    }
    Ok(())
}

fn compose_initial_world_colors(
    layer: &Layer,
    transforms: &[SpatialTransform],
    layer_enabled: bool,
) -> Result<Vec<InitialWorldColorState>, SrdDrawError> {
    let hierarchy = layer
        .build_hierarchy()
        .map_err(|error| SrdDrawError(error.to_string()))?;
    let mut result = vec![
        InitialWorldColorState {
            multiply: [255; 4],
            additive: [0; 4],
            visible: false,
        };
        layer.nodes.len()
    ];
    let layer_world = InitialWorldColorState {
        multiply: [255; 4],
        additive: [0; 4],
        visible: layer_enabled,
    };
    for &root in &hierarchy.roots {
        compose_initial_world_color_node(
            layer,
            transforms,
            &hierarchy.children,
            root,
            layer_world,
            &mut result,
        );
    }
    Ok(result)
}

fn compose_initial_world_color_node(
    layer: &Layer,
    transforms: &[SpatialTransform],
    children: &[Vec<usize>],
    index: usize,
    parent: InitialWorldColorState,
    output: &mut [InitialWorldColorState],
) {
    let flags = layer.nodes[index].type_flags.unwrap_or(0);
    let local = transforms[index];
    let world = InitialWorldColorState {
        multiply: if flags & 0x200 != 0 {
            multiply_color_game(parent.multiply, local.multiply_color)
        } else {
            local.multiply_color
        },
        additive: if flags & 0x0008_0000 != 0 {
            add_color_saturating_game(parent.additive, local.additive_color)
        } else {
            local.additive_color
        },
        visible: local.is_visible() && (flags & 0x400 == 0 || parent.visible),
    };
    output[index] = world;
    for &child in &children[index] {
        compose_initial_world_color_node(layer, transforms, children, child, world, output);
    }
}
