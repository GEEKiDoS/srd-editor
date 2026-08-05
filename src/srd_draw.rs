use std::collections::BTreeMap;
use std::fmt;

use crate::attribute::CastAttributeValue;
use crate::fennel::{
    FENNEL_DEFAULT_D_VALUE, FENNEL_DEFAULT_REPEAT_SPACE_COUNT, FennelFontSlotRegistry,
    FennelFontSlotRequest, FennelNormalDrawInput, FennelOwnedTextureBatch, FennelResolvedGlyph,
    FennelSrdDrawPreparation, FennelSrdDrawPreparationInput, FennelStaticTextProperties,
    build_fennel_normal_vertex_batches, build_fennel_plain_record_stream_with_font_slots,
    build_fennel_srd_repeated_text, fennel_font_param_effect_color, fennel_srd_font_style,
    layout_fennel_static_srd_explicit_flags, layout_fennel_static_srd_font_param,
    measure_fennel_srd_text_size_mode0, prepare_fennel_srd_draw, prepare_fennel_srd_runtime_text,
};
use crate::image::{
    ImageDefinition, ImageReferenceChannel, SrdTextureBindingSource,
    premultiply_additive_color_game,
};
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
use crate::ruhuna::RuhunaRuntimeFont;
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

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceCompleteFennelDraw {
    pub scene_index: usize,
    pub layer_index: usize,
    pub node_index: usize,
    pub font_name: Vec<u8>,
    pub is_2d: bool,
    pub fixed_constants: CeylonSrdFixedShaderConstants,
    pub batches: Vec<FennelOwnedTextureBatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FennelTextFontRole {
    Primary,
    Ruby,
    Outline,
    OutlineRuby,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FennelFontResourceRequest {
    pub scene_index: usize,
    pub layer_index: usize,
    pub node_index: usize,
    pub role: FennelTextFontRole,
    pub name: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FennelFontResourceAssignment {
    pub request: FennelFontResourceRequest,
    pub slot: FennelFontSlotRequest,
}

/// Collects the font-resource requests issued by `sub_AAE6C0` while it walks
/// the player's original runtime scene table.
///
/// The binary traverses scenes, layers, and each layer's CAST vector in their
/// construction order. For every SrTextCast it requests the primary font
/// first, then visits the selected CATR records in source order. Only the
/// exact case-sensitive string keys below call the same four-slot TextCast
/// font loader. Empty strings reach `sub_1088590` but do not issue a resource
/// request, so they are omitted here.
///
/// Independently constructed reference layers do not add requests here.
/// `srd_player_impl_load_project` resolves and constructs those copies before
/// this traversal, but `sub_AAE6C0` still walks only the original runtime scene
/// table. Every copied TextCast was rebuilt from a target LAYR already present
/// in that table and resolves its TextBox through the shared renderer resource
/// tree at draw time.
///
/// This is not an assertion that the process-global FontManager was empty
/// before this player loaded. Apply the returned requests to a registry that
/// already contains any host resources whose earlier lifetime is known.
pub fn collect_fennel_font_resource_requests(
    project: &Project,
) -> Result<Vec<FennelFontResourceRequest>, SrdDrawError> {
    let mut requests = Vec::new();
    for (scene_index, scene) in project.scenes.iter().enumerate() {
        for (layer_index, layer) in scene.layers.iter().enumerate() {
            for node_index in 0..layer.nodes.len() {
                let Some(image) = layer.image_by_node.get(node_index).and_then(Option::as_ref)
                else {
                    continue;
                };
                if !image.creates_text_cast() {
                    continue;
                }
                let Some(text) = image.text.as_ref() else {
                    return Err(SrdDrawError(format!(
                        "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] creates an SrTextCast without a TEXT definition"
                    )));
                };
                let font_index = usize::try_from(text.font_index.unwrap_or(-1)).map_err(|_| {
                    SrdDrawError(format!(
                        "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] has an invalid TextCast font index"
                    ))
                })?;
                let font = project.fonts.get(font_index).ok_or_else(|| {
                    SrdDrawError(format!(
                        "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] TextCast font index {font_index} is outside PROJ"
                    ))
                })?;
                push_fennel_font_resource_request(
                    &mut requests,
                    scene_index,
                    layer_index,
                    node_index,
                    FennelTextFontRole::Primary,
                    &font.name,
                );

                let Some(attribute_list_index) = layer
                    .cast_attribute_list_by_node
                    .get(node_index)
                    .and_then(|index| *index)
                else {
                    continue;
                };
                let attribute_list = layer
                    .cast_attribute_lists
                    .get(attribute_list_index)
                    .ok_or_else(|| {
                        SrdDrawError(format!(
                            "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] CATR index {attribute_list_index} is outside the parsed list table"
                        ))
                    })?;
                for attribute in &attribute_list.attributes {
                    let Some(role) = (match attribute.name.as_slice() {
                        b"rubyFont" => Some(FennelTextFontRole::Ruby),
                        b"rfzOutlineFont" => Some(FennelTextFontRole::Outline),
                        b"rfzOutlineRubyFont" => Some(FennelTextFontRole::OutlineRuby),
                        _ => None,
                    }) else {
                        continue;
                    };
                    let CastAttributeValue::String(name) = &attribute.value else {
                        return Err(SrdDrawError(format!(
                            "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] CATR {:?} uses an unported non-string font-resource value",
                            String::from_utf8_lossy(&attribute.name)
                        )));
                    };
                    push_fennel_font_resource_request(
                        &mut requests,
                        scene_index,
                        layer_index,
                        node_index,
                        role,
                        name,
                    );
                }
            }
        }
    }
    Ok(requests)
}

fn push_fennel_font_resource_request(
    requests: &mut Vec<FennelFontResourceRequest>,
    scene_index: usize,
    layer_index: usize,
    node_index: usize,
    role: FennelTextFontRole,
    name: &[u8],
) {
    if !name.is_empty() {
        requests.push(FennelFontResourceRequest {
            scene_index,
            layer_index,
            node_index,
            role,
            name: name.to_vec(),
        });
    }
}

pub fn assign_fennel_font_resource_requests(
    registry: &mut FennelFontSlotRegistry<Vec<u8>>,
    requests: impl IntoIterator<Item = FennelFontResourceRequest>,
) -> Vec<FennelFontResourceAssignment> {
    requests
        .into_iter()
        .map(|request| {
            let slot = registry.request(request.name.clone());
            FennelFontResourceAssignment { request, slot }
        })
        .collect()
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

/// Builds the initial, 2D RFZ TextCast subset whose normal-glyph layout,
/// texture batching, vertex generation, world transform and color inputs are
/// all closed. 3D TextCast, effect/crop records and legacy `.sbfont` stay out
/// of this evidence-complete path.
///
/// `font_registry` is the process-global slot state after this player's
/// requests have been applied. It may already contain earlier host resources;
/// the function uses it for both the TextCast primary slot and explicit
/// `$F[n]` switches. Runtime fonts retain caller-defined opaque texture tokens,
/// allowing the renderer to route batches across different font atlases.
#[derive(Debug, Clone, PartialEq)]
pub struct FennelSrdRuntimeTextInput {
    pub substitutions: [Vec<u8>; 8],
    pub default_d: i32,
    pub repeat_space_count: i32,
    /// Exact current SrTextCast text-state field `+0xF4`. Callers may advance
    /// it with `FennelSrdScrollState::advance`; this API does not infer a game
    /// tick rate from editor animation frames.
    pub field_f4: f32,
}

impl Default for FennelSrdRuntimeTextInput {
    fn default() -> Self {
        // Empty slots are an editor form default, not a claim about an
        // arbitrary game's current SrTextCast host strings. The initial draw
        // wrapper still rejects serialized substitution tokens without an
        // explicit input entry.
        Self {
            substitutions: std::array::from_fn(|_| Vec::new()),
            default_d: FENNEL_DEFAULT_D_VALUE,
            repeat_space_count: FENNEL_DEFAULT_REPEAT_SPACE_COUNT,
            field_f4: 0.0,
        }
    }
}

pub fn build_evidence_complete_initial_fennel_draws(
    project: &Project,
    scene_index: usize,
    host: SrdHostDrawContext,
    font_registry: &FennelFontSlotRegistry<Vec<u8>>,
    runtime_fonts: &BTreeMap<Vec<u8>, RuhunaRuntimeFont>,
    force_color_update: bool,
) -> Result<Vec<EvidenceCompleteFennelDraw>, SrdDrawError> {
    build_evidence_complete_fennel_draws_impl(
        project,
        scene_index,
        host,
        font_registry,
        runtime_fonts,
        force_color_update,
        None,
        None,
    )
}

/// Builds the same RFZ draw subset with explicit per-node SrTextCast host
/// inputs. Keys are `(layer_index, node_index)` within `scene_index`; absent
/// entries retain the strict initial behavior and reject `$[0]..$[7]` rather
/// than inventing substitution values.
pub fn build_evidence_complete_fennel_draws_with_runtime_text(
    project: &Project,
    scene_index: usize,
    host: SrdHostDrawContext,
    font_registry: &FennelFontSlotRegistry<Vec<u8>>,
    runtime_fonts: &BTreeMap<Vec<u8>, RuhunaRuntimeFont>,
    force_color_update: bool,
    runtime_text_inputs: &BTreeMap<(usize, usize), FennelSrdRuntimeTextInput>,
) -> Result<Vec<EvidenceCompleteFennelDraw>, SrdDrawError> {
    build_evidence_complete_fennel_draws_impl(
        project,
        scene_index,
        host,
        font_registry,
        runtime_fonts,
        force_color_update,
        None,
        Some(runtime_text_inputs),
    )
}

/// Applies one ANMS frame through the shared ProjectRuntime, then builds RFZ
/// TextCast draws with the resulting layer enable, CAST transforms/colors and
/// SrImage geometry while keeping text-host substitutions/scroll state
/// explicit.
pub fn build_evidence_complete_animation_set_fennel_draws_with_runtime_text(
    project: &Project,
    textures: &TextureList,
    scene_index: usize,
    animation_set_index: usize,
    frame: f32,
    host: SrdHostDrawContext,
    font_registry: &FennelFontSlotRegistry<Vec<u8>>,
    runtime_fonts: &BTreeMap<Vec<u8>, RuhunaRuntimeFont>,
    force_color_update: bool,
    runtime_text_inputs: &BTreeMap<(usize, usize), FennelSrdRuntimeTextInput>,
) -> Result<Vec<EvidenceCompleteFennelDraw>, SrdDrawError> {
    let mut runtime = ProjectRuntime::new(project)
        .map_err(|error| SrdDrawError(format!("failed to construct SRD runtime: {error}")))?;
    runtime
        .apply_animation_set(project, textures, scene_index, animation_set_index, frame)
        .map_err(|error| SrdDrawError(format!("failed to apply animation set: {error}")))?;
    let runtime_layers = runtime
        .project_layers
        .get(scene_index)
        .ok_or_else(|| SrdDrawError(format!("scene index {scene_index} is outside the runtime")))?;
    build_evidence_complete_fennel_draws_impl(
        project,
        scene_index,
        host,
        font_registry,
        runtime_fonts,
        force_color_update,
        Some(runtime_layers),
        Some(runtime_text_inputs),
    )
}

fn build_evidence_complete_fennel_draws_impl(
    project: &Project,
    scene_index: usize,
    host: SrdHostDrawContext,
    font_registry: &FennelFontSlotRegistry<Vec<u8>>,
    runtime_fonts: &BTreeMap<Vec<u8>, RuhunaRuntimeFont>,
    force_color_update: bool,
    runtime_layers: Option<&[ProjectLayerRuntimeState]>,
    runtime_text_inputs: Option<&BTreeMap<(usize, usize), FennelSrdRuntimeTextInput>>,
) -> Result<Vec<EvidenceCompleteFennelDraw>, SrdDrawError> {
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
    let identity = identity_matrix4x4_game();
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
        if !layer_enabled || !layer.is_2d() || reject_special_matrix_branches(layer).is_err() {
            continue;
        }
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
        let world_matrices = layer
            .compose_world_matrices_with_csli_layout(&transforms, host.first_calc_matrix, false)
            .map_err(|error| SrdDrawError(error.to_string()))?;
        let world_colors = compose_initial_world_colors(layer, &transforms, layer_enabled)?;

        for node_index in 0..layer.nodes.len() {
            let Some(image) = layer.image_by_node[node_index]
                .as_ref()
                .filter(|image| image.creates_text_cast())
            else {
                continue;
            };
            let Some(text) = image.text.as_ref() else {
                continue;
            };
            let world_color = world_colors[node_index];
            if !world_color.visible {
                continue;
            }
            let font_index = usize::try_from(text.font_index.unwrap_or(-1)).map_err(|_| {
                SrdDrawError(format!(
                    "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] has an invalid RFZ font index"
                ))
            })?;
            let font = project.fonts.get(font_index).ok_or_else(|| {
                SrdDrawError(format!(
                    "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] RFZ font index {font_index} is outside PROJ"
                ))
            })?;
            if !font
                .name
                .iter()
                .map(u8::to_ascii_lowercase)
                .collect::<Vec<_>>()
                .ends_with(b".rfz")
            {
                continue;
            }
            if !runtime_fonts.contains_key(font.name.as_slice()) {
                return Err(SrdDrawError(format!(
                    "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] runtime RFZ {:?} is not loaded",
                    String::from_utf8_lossy(&font.name)
                )));
            }
            let primary_slot = font_registry
                .request_for_key(&font.name)
                .filter(|request| request.registered)
                .ok_or_else(|| {
                    SrdDrawError(format!(
                        "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] RFZ {:?} has no registered global Fennel slot",
                        String::from_utf8_lossy(&font.name)
                    ))
                })?
                .font_slot_id;
            let image_state = runtime_layer.map_or_else(
                || image.initial_runtime_state(),
                |runtime_layer| runtime_layer.image_states[node_index],
            );
            let font_param = layer.font_param_for_node(node_index).unwrap_or_default();
            if font_param.vertical {
                return Err(SrdDrawError(format!(
                    "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] uses the unported FontParamData vertical correction"
                )));
            }
            let properties = FennelStaticTextProperties::from_text_definition(
                text,
                image_state.geometry.size[0],
                image_state.geometry.size[1],
            )
            .map_err(|error| SrdDrawError(error.to_string()))?;
            let source_colors = image_state
                .coordinate_state(ImageReferenceChannel::Cref)
                .vertex_colors;
            let primary_rgba = [0usize, 2, 1, 3].map(|source_index| {
                multiply_color_game(source_colors[source_index], world_color.multiply)
            });
            let secondary_rgba = premultiply_additive_color_game(world_color.additive);
            if !force_color_update
                && !secondary_rgba[..3].iter().any(|component| *component != 0)
                && !primary_rgba.iter().any(|color| color[3] != 0)
            {
                // `sub_AC5740` skips the entire TextBox update/draw block when
                // its external +0x11C bit 0x100 is clear and all effective
                // primary alpha / secondary RGB channels are zero.
                continue;
            }
            let primary_colors = primary_rgba.map(pack_fennel_record_color);
            let secondary_color = pack_fennel_record_color(secondary_rgba);
            let font_style = fennel_srd_font_style(font_param);
            let effect_color = fennel_font_param_effect_color(font_param.shadow_color);
            let effect_offset = [font_param.shadow_x as f32, font_param.shadow_y as f32];
            let runtime_text_input =
                runtime_text_inputs.and_then(|inputs| inputs.get(&(layer_index, node_index)));
            if runtime_text_input.is_none() {
                for substitution_index in 0..8u8 {
                    let token = [b'$', b'[', b'0' + substitution_index, b']'];
                    if text
                        .text
                        .windows(token.len())
                        .any(|candidate| candidate == token)
                    {
                        return Err(SrdDrawError(format!(
                            "SCN[{scene_index}]/LAYR[{layer_index}]/NODE[{node_index}] uses runtime substitution {:?}, but this draw has no SrTextCast substitution host input",
                            String::from_utf8_lossy(&token)
                        )));
                    }
                }
            }
            let substitutions: [&[u8]; 8] = runtime_text_input
                .map_or([b"".as_slice(); 8], |input| {
                    std::array::from_fn(|index| input.substitutions[index].as_slice())
                });
            let default_d =
                runtime_text_input.map_or(FENNEL_DEFAULT_D_VALUE, |input| input.default_d);
            let repeat_space_count = runtime_text_input
                .map_or(FENNEL_DEFAULT_REPEAT_SPACE_COUNT, |input| {
                    input.repeat_space_count
                });
            let mut prepared_text =
                prepare_fennel_srd_runtime_text(&text.text, substitutions, default_d, font_param)
                    .map_err(|error| SrdDrawError(error.to_string()))?;
            if let Some(input) = runtime_text_input {
                prepared_text.scroll_state.field_f4 = input.field_f4;
            }
            let scroll = prepared_text.scroll_state.outputs();
            let build_layout = |source: &[u8], explicit_flags: Option<u32>| {
                let mut stream = build_fennel_plain_record_stream_with_font_slots(
                    source,
                    primary_slot,
                    properties.glyph_placement(0, font_style.record_flags, primary_colors),
                    2048,
                    |font_slot_id, code| {
                        let resource_name = font_registry.resource_for_slot(font_slot_id)?;
                        let runtime_font = runtime_fonts.get(resource_name.as_slice())?;
                        Some(FennelResolvedGlyph {
                            glyph_token: fennel_slot_code_token(font_slot_id, code),
                            glyph: *runtime_font.glyph(code)?,
                        })
                    },
                )
                .map_err(|error| SrdDrawError(error.to_string()))?;
                let layout = if let Some(textbox_flags) = explicit_flags {
                    layout_fennel_static_srd_explicit_flags(
                        &mut stream,
                        properties.layout,
                        textbox_flags,
                        |token| {
                            let (font_slot_id, code) = fennel_slot_code_from_token(token);
                            let resource_name = font_registry.resource_for_slot(font_slot_id)?;
                            let runtime_font = runtime_fonts.get(resource_name.as_slice())?;
                            runtime_font.glyph(code).map(Into::into)
                        },
                    )
                } else {
                    layout_fennel_static_srd_font_param(
                        &mut stream,
                        properties.layout,
                        font_param,
                        |token| {
                            let (font_slot_id, code) = fennel_slot_code_from_token(token);
                            let resource_name = font_registry.resource_for_slot(font_slot_id)?;
                            let runtime_font = runtime_fonts.get(resource_name.as_slice())?;
                            runtime_font.glyph(code).map(Into::into)
                        },
                    )
                }
                .map_err(|error| SrdDrawError(error.to_string()))?;
                Ok::<_, SrdDrawError>((stream, layout))
            };
            let measure = |stream: &_| {
                measure_fennel_srd_text_size_mode0(
                    stream,
                    scroll.maximum_glyphs,
                    [properties.layout.scale_x, properties.layout.scale_y],
                    effect_offset,
                    |token| {
                        let (font_slot_id, code) = fennel_slot_code_from_token(token);
                        let resource_name = font_registry.resource_for_slot(font_slot_id)?;
                        let runtime_font = runtime_fonts.get(resource_name.as_slice())?;
                        runtime_font.glyph(code).copied()
                    },
                )
                .map_err(|error| SrdDrawError(error.to_string()))
            };

            let (initial_stream, initial_layout) = build_layout(&prepared_text.text, None)?;
            let initial_text_size = measure(&initial_stream)?;
            let repeated_text = build_fennel_srd_repeated_text(
                &prepared_text.text,
                initial_layout.textbox_flags,
                repeat_space_count,
            )
            .map_err(|error| SrdDrawError(error.to_string()))?;
            let repeated = if let Some(repeated_text) = repeated_text {
                let (stream, layout) =
                    build_layout(&repeated_text, Some(initial_layout.textbox_flags))?;
                let size = measure(&stream)?;
                Some((stream, layout, size))
            } else {
                None
            };
            let preparation = prepare_fennel_srd_draw(FennelSrdDrawPreparationInput {
                textbox_flags: initial_layout.textbox_flags,
                text_size: initial_text_size,
                clip_size: [properties.layout.box_width, properties.layout.box_height],
                font_point_y: u16::from(font_style.point_y),
                scroll,
                repeated_text_size: repeated.as_ref().map(|(_, _, size)| *size),
            })
            .map_err(|error| SrdDrawError(error.to_string()))?;
            let draw_textbox_flags = initial_layout.textbox_flags;
            let (stream, layout, draw_offset) = match preparation {
                FennelSrdDrawPreparation::DrawOffset(draw_offset) => {
                    if let Some((stream, layout, _)) = repeated {
                        (stream, layout, draw_offset)
                    } else {
                        (initial_stream, initial_layout, draw_offset)
                    }
                }
                FennelSrdDrawPreparation::RelayoutWithoutScrollModes { layout_flags } => {
                    let (stream, layout) = build_layout(&prepared_text.text, Some(layout_flags))?;
                    (stream, layout, [0.0; 2])
                }
            };
            let vertex_build = build_fennel_normal_vertex_batches(
                &stream,
                scroll.maximum_glyphs,
                FennelNormalDrawInput {
                    is_2d: true,
                    textbox_position: [
                        -image_state.geometry.origin[0],
                        -image_state.geometry.origin[1],
                        0.0,
                    ],
                    textbox_scale: [properties.layout.scale_x, properties.layout.scale_y],
                    textbox_vertical_offset: layout.layout.textbox_vertical_offset,
                    textbox_transform: affine_to_matrix4x4(world_matrices[node_index]),
                    secondary_color,
                    // `sub_7C04F0` restores the original word after a fit
                    // guard relayout and draws with that original clip state.
                    textbox_flags: draw_textbox_flags,
                    clip_size: [properties.layout.box_width, properties.layout.box_height],
                    draw_offset,
                    effect_colors: [effect_color; 4],
                    effect_offset,
                },
                |token| {
                    let (font_slot_id, code) = fennel_slot_code_from_token(token);
                    let resource_name = font_registry.resource_for_slot(font_slot_id)?;
                    let runtime_font = runtime_fonts.get(resource_name.as_slice())?;
                    runtime_font.glyph(code).copied()
                },
            )
            .map_err(|error| SrdDrawError(error.to_string()))?;
            if vertex_build.processed_glyph_count == 0 {
                continue;
            }
            let mut fixed_constants = CeylonSrdFixedShaderConstants::initial_for_target(
                host.target_projection_view,
                host.target_screen_size,
            );
            fixed_constants.vertex_c0_c3_world = identity;
            fixed_constants.vertex_c4_c7 = identity;
            draws.push(EvidenceCompleteFennelDraw {
                scene_index,
                layer_index,
                node_index,
                font_name: font.name.clone(),
                is_2d: true,
                fixed_constants,
                batches: vertex_build.batches,
            });
        }
    }
    Ok(draws)
}

const fn fennel_slot_code_token(font_slot_id: u16, code: u16) -> u32 {
    (font_slot_id as u32) << 16 | code as u32
}

const fn fennel_slot_code_from_token(token: u32) -> (u16, u16) {
    ((token >> 16) as u16, token as u16)
}

fn affine_to_matrix4x4(matrix: Affine3x4) -> Matrix4x4 {
    Matrix4x4 {
        rows: [
            matrix.rows[0],
            matrix.rows[1],
            matrix.rows[2],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

fn pack_fennel_record_color([red, green, blue, alpha]: [u8; 4]) -> u32 {
    // `sub_AC5740 -> sub_F25DA0` stores record colors as AARRGGBB, whose
    // little-endian bytes are B,G,R,A.
    u32::from_le_bytes([blue, green, red, alpha])
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{CastAttribute, CastAttributeList};
    use crate::camera::CameraDefinition;
    use crate::image::ImageDefinition;
    use crate::scene::{NodeRecord, Scene};
    use crate::text::{FontDefinition, TextDefinition};

    fn text_image(node_index: i32, font_index: i32) -> ImageDefinition {
        ImageDefinition {
            flags: 0x100,
            width: 128.0,
            height: 128.0,
            custom_origin: [0.0; 2],
            origin_mode: 0,
            vertex_colors: [[0xff; 4]; 4],
            cref_index: -1,
            cref_count: 0,
            crefs: Vec::new(),
            field_4c: 0,
            cre1_index: -1,
            cre1_count: 0,
            cre1s: Vec::new(),
            coordinate_offsets: [[0.0; 2]; 2],
            field_a1: 0,
            node_index,
            has_text_child: true,
            text: Some(TextDefinition {
                field_78: None,
                font_index: Some(font_index),
                text: Vec::new(),
                field_36: None,
                field_7b: None,
                field_7c: None,
                field_41: None,
            }),
        }
    }

    fn node() -> NodeRecord {
        NodeRecord {
            name: None,
            type_flags: Some(1),
            parent_csli_cell_index: None,
            first_child_index: -1,
            next_sibling_index: -1,
            field_a0: None,
        }
    }

    fn font(name: &[u8]) -> crate::text::FontDefinition {
        FontDefinition {
            name: name.to_vec(),
            flags_70: None,
            field_71: None,
            characters: Vec::new(),
        }
    }

    #[test]
    fn fennel_font_requests_follow_runtime_cast_then_catr_order() {
        let attribute_list = CastAttributeList {
            node_index: Some(0),
            declared_count: 4,
            attributes: vec![
                CastAttribute {
                    name: b"rubyFont".to_vec(),
                    source_type_code: 2,
                    value: CastAttributeValue::String(b"ruby.rfz".to_vec()),
                },
                CastAttribute {
                    name: b"ignoredFont".to_vec(),
                    source_type_code: 2,
                    value: CastAttributeValue::String(b"ignored.rfz".to_vec()),
                },
                CastAttribute {
                    name: b"rfzOutlineFont".to_vec(),
                    source_type_code: 2,
                    value: CastAttributeValue::String(b"outline.rfz".to_vec()),
                },
                CastAttribute {
                    name: b"rfzOutlineRubyFont".to_vec(),
                    source_type_code: 2,
                    value: CastAttributeValue::String(Vec::new()),
                },
            ],
        };
        let layer = Layer {
            name: b"layer".to_vec(),
            flags: 0,
            animation_count: 0,
            animations: Vec::new(),
            field_23: Vec::new(),
            nodes: vec![node(), node(), node()],
            transforms: Vec::new(),
            image_by_node: vec![
                Some(text_image(0, 1)),
                Some(text_image(1, 0)),
                Some(text_image(2, 1)),
            ],
            number_by_node: vec![None, None, None],
            reference_by_node: vec![None, None, None],
            csli_by_node: vec![None, None, None],
            cast_attribute_lists: vec![attribute_list],
            cast_attribute_list_by_node: vec![Some(0), None, None],
        };
        let project = Project {
            name: b"project".to_vec(),
            declared_scene_count: 1,
            declared_font_count: 2,
            camera: CameraDefinition::default(),
            scenes: vec![Scene {
                name: b"scene".to_vec(),
                declared_layer_count: 1,
                declared_animation_set_count: 0,
                width: 1920.0,
                height: 1080.0,
                layers: vec![layer],
                animation_sets: Vec::new(),
            }],
            fonts: vec![font(b"primary_a.rfz"), font(b"primary_b.rfz")],
        };

        let requests = collect_fennel_font_resource_requests(&project).unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|request| (request.node_index, request.role, request.name.as_slice()))
                .collect::<Vec<_>>(),
            vec![
                (0, FennelTextFontRole::Primary, b"primary_b.rfz".as_slice()),
                (0, FennelTextFontRole::Ruby, b"ruby.rfz".as_slice()),
                (0, FennelTextFontRole::Outline, b"outline.rfz".as_slice()),
                (1, FennelTextFontRole::Primary, b"primary_a.rfz".as_slice()),
                (2, FennelTextFontRole::Primary, b"primary_b.rfz".as_slice()),
            ]
        );

        let mut registry = FennelFontSlotRegistry::default();
        let assignments = assign_fennel_font_resource_requests(&mut registry, requests);
        assert_eq!(
            assignments
                .iter()
                .map(|assignment| assignment.slot.font_slot_id)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 0]
        );
        assert_eq!(
            assignments[4].slot.resource_handle,
            assignments[0].slot.resource_handle
        );
        assert_eq!(assignments[4].slot.lease_count, 2);
    }

    #[test]
    fn fennel_runtime_text_input_manual_defaults_keep_slots_explicit_and_clock_zero() {
        let input = FennelSrdRuntimeTextInput::default();
        assert!(input.substitutions.iter().all(Vec::is_empty));
        assert_eq!(input.default_d, FENNEL_DEFAULT_D_VALUE);
        assert_eq!(input.repeat_space_count, FENNEL_DEFAULT_REPEAT_SPACE_COUNT);
        assert_eq!(input.field_f4.to_bits(), 0.0f32.to_bits());
    }
}
