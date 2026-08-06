use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

use imgui::{ConfigFlags, Context, FontConfig, FontSource, TextureId};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::d3d9_backend::{D3d9ExDevice, D3d9ExDeviceStatus, D3d9ExFrameStatus};
use crate::d3d9_fennel::{EvidenceCompleteFennelBatch, FennelDx9Renderer};
use crate::d3d9_srd::{SrdDx9ExternalContext, SrdDx9Renderer};
use crate::d3d9_texture::{RuhunaD3d9AtlasSet, SrdD3d9TextureSet, audit_dds_device_uploads};
use crate::editor_workspace::{
    EditorWorkspace, PreviewHostSettings, PreviewPlayerSelection, PreviewScissorSelection,
    PreviewTargetSelection, apply_editor_style,
};
use crate::fennel::FennelFontSlotRegistry;
use crate::game_host::{
    CHUSAN_ADVERTISE_LOGO_PLAYER, CHUSAN_BG_SCENE, CHUSAN_COMMON_BACKGROUND_PLAYER,
    CHUSAN_MAIN_SCENE,
};
use crate::imgui_dx9::ImguiDx9Renderer;
use crate::reference_runtime::ReferenceLayerParent;
use crate::ruhuna::{RuhunaFont, RuhunaRuntimeFont};
use crate::scene::ReferenceTarget;
use crate::shader_bytecode::{
    FIRST_FIXTURE_SIMPLE_KEY, FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY, embedded_simple_shader_pair,
};
use crate::srd_draw::{
    EvidenceCompleteFennelDraw, EvidenceCompleteRuntimeCastDraw, EvidenceCompleteSrdDraw,
    EvidenceMergedRuntimeTargetCommand, EvidenceRuntimeTargetCommandSource,
    FennelFontResourceAssignment, FennelRuntimeTextCastKey, SrdHostDrawContext,
    SrdRendererProjectTargetContext, assign_fennel_font_resource_requests,
    build_evidence_complete_animation_set_image_draws,
    build_evidence_complete_animation_set_runtime_cast_draws,
    build_evidence_complete_initial_fennel_draws, build_evidence_complete_initial_image_draws,
    build_evidence_filtered_merged_runtime_target_submission,
    build_evidence_merged_runtime_fennel_list, build_evidence_merged_runtime_srd_strip,
    collect_fennel_font_resource_requests,
};
use crate::transform::Affine3x4;

const CLEAR_COLOR_ARGB: u32 = 0xff20_2226;

pub fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let chusan_player_host = parse_chusan_player_host_argument(&arguments)?;
    let srd_draw_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-draw-smoke");
    let srd_texture_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-texture-smoke");
    let srd_fennel_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-fennel-smoke");
    let srd_runtime_smoke = parse_runtime_smoke_argument(&arguments)?;
    let dds_device_audit = arguments
        .iter()
        .any(|argument| argument == "--dds-device-audit");
    let smoke_test = srd_draw_smoke
        || srd_texture_smoke
        || srd_fennel_smoke
        || srd_runtime_smoke.is_some()
        || dds_device_audit
        || arguments
            .iter()
            .any(|argument| argument == "--d3d9ex-smoke" || argument == "--d3d9-smoke");
    let document_path = arguments
        .iter()
        .find(|argument| !argument.starts_with("--"))
        .map(PathBuf::from);
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut application = EditorApplication::new(
        smoke_test,
        srd_draw_smoke,
        srd_texture_smoke,
        srd_fennel_smoke,
        srd_runtime_smoke,
        dds_device_audit,
        chusan_player_host,
        document_path,
    );
    event_loop.run_app(&mut application)?;
    if let Some(error) = application.fatal_error {
        return Err(io::Error::other(error).into());
    }
    Ok(())
}

struct EditorApplication {
    window: Option<EditorWindow>,
    fatal_error: Option<String>,
    smoke_test: bool,
    srd_draw_smoke: bool,
    srd_texture_smoke: bool,
    srd_fennel_smoke: bool,
    srd_runtime_smoke: Option<RuntimeSmokeSelection>,
    dds_device_audit: bool,
    chusan_player_host: Option<ChusanPlayerHostArgument>,
    document_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChusanPlayerHostArgument {
    common_background: bool,
    target: PreviewTargetSelection,
    present_width: u32,
    present_height: u32,
    screen_width: u32,
    screen_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSmokeSelection {
    scene_index: usize,
    animation_set_index: usize,
    frame: i32,
}

#[derive(Debug, Clone, Copy)]
struct EditorSmokeOptions {
    srd_draw: bool,
    srd_texture: bool,
    srd_fennel: bool,
    srd_runtime: Option<RuntimeSmokeSelection>,
    dds_device_audit: bool,
    chusan_player_host: Option<ChusanPlayerHostArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FennelAtlasRoute {
    font_name: Vec<u8>,
    page_index: usize,
}

struct EditorWindow {
    window: Window,
    d3d9: D3d9ExDevice,
    imgui: Context,
    platform: WinitPlatform,
    imgui_renderer: ImguiDx9Renderer,
    srd_renderer: Option<SrdDx9Renderer>,
    srd_textures: Option<SrdD3d9TextureSet>,
    srd_draws: Vec<EvidenceCompleteSrdDraw>,
    runtime_draws: Vec<EvidenceCompleteRuntimeCastDraw>,
    runtime_submission: Vec<EvidenceMergedRuntimeTargetCommand>,
    fennel_renderer: Option<FennelDx9Renderer>,
    fennel_atlases: BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
    fennel_atlas_routes: BTreeMap<u32, FennelAtlasRoute>,
    fennel_draws: Vec<EvidenceCompleteFennelDraw>,
    composition_texture_id: Option<TextureId>,
    applied_preview_host: Option<PreviewHostSettings>,
    verify_srd_pixels: bool,
    verify_textured_pixels: bool,
    verify_fennel_pixels: bool,
    verify_runtime_pixels: bool,
    verify_hidpi: bool,
    workspace: EditorWorkspace,
    dpi_factor: f64,
    last_frame: Instant,
}

impl EditorApplication {
    fn new(
        smoke_test: bool,
        srd_draw_smoke: bool,
        srd_texture_smoke: bool,
        srd_fennel_smoke: bool,
        srd_runtime_smoke: Option<RuntimeSmokeSelection>,
        dds_device_audit: bool,
        chusan_player_host: Option<ChusanPlayerHostArgument>,
        document_path: Option<PathBuf>,
    ) -> Self {
        Self {
            window: None,
            fatal_error: None,
            smoke_test,
            srd_draw_smoke,
            srd_texture_smoke,
            srd_fennel_smoke,
            srd_runtime_smoke,
            dds_device_audit,
            chusan_player_host,
            document_path,
        }
    }
}

impl EditorWindow {
    fn new(
        window: Window,
        d3d9: D3d9ExDevice,
        smoke_test: bool,
        smoke: EditorSmokeOptions,
        document_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let EditorSmokeOptions {
            srd_draw: srd_draw_smoke,
            srd_texture: srd_texture_smoke,
            srd_fennel: srd_fennel_smoke,
            srd_runtime: srd_runtime_smoke,
            dds_device_audit,
            chusan_player_host,
        } = smoke;
        let mut imgui = Context::create();
        imgui.io_mut().config_flags |= ConfigFlags::DOCKING_ENABLE;
        let ini_path = if smoke_test { None } else { editor_ini_path() };
        let build_default_layout = ini_path.as_ref().is_none_or(|path| !path.exists());
        imgui.set_ini_filename(ini_path);
        apply_editor_style(&mut imgui);

        let mut platform = WinitPlatform::new(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);
        let dpi_factor = platform.hidpi_factor();
        configure_imgui_fonts(&mut imgui, dpi_factor);
        let shader_pair = embedded_simple_shader_pair(&FIRST_FIXTURE_SIMPLE_KEY)
            .ok_or_else(|| "embedded first-fixture shader pair is missing".to_string())?;
        d3d9.validate_shader_pair(&shader_pair)
            .map_err(|error| format!("D3D9 failed to create embedded SRD shaders: {error}"))?;
        if dds_device_audit {
            let root = document_path.as_ref().ok_or_else(|| {
                "--dds-device-audit requires a DDS file or directory path".to_string()
            })?;
            let audit = audit_dds_device_uploads(d3d9.device(), root)
                .map_err(|error| format!("D3D9 DDS device audit failed: {error}"))?;
            eprintln!(
                "D3D9 DDS device audit files={} game_native={} independent_decode={} mip_levels={}",
                audit.file_count,
                audit.game_native_count,
                audit.independent_decode_count,
                audit.mip_level_count,
            );
        }
        let mut imgui_renderer =
            ImguiDx9Renderer::new(&mut imgui, d3d9.device()).map_err(|error| error.to_string())?;
        let workspace_path = (!dds_device_audit).then_some(document_path).flatten();
        let mut workspace = EditorWorkspace::new(build_default_layout, workspace_path);
        if let Some(runtime) = srd_runtime_smoke {
            let host = chusan_player_host.ok_or_else(|| {
                "--srd-runtime-smoke requires --advertise-logo-host or --common-background-host"
                    .to_string()
            })?;
            let player = if host.common_background {
                PreviewPlayerSelection::CommonBackground
            } else {
                PreviewPlayerSelection::AdvertiseLogo
            };
            workspace.configure_preview_for_runtime_smoke(
                runtime.scene_index,
                runtime.animation_set_index,
                runtime.frame,
                player,
                host.target,
                [
                    i32::try_from(host.present_width)
                        .map_err(|_| "runtime smoke present width exceeds i32".to_string())?,
                    i32::try_from(host.present_height)
                        .map_err(|_| "runtime smoke present height exceeds i32".to_string())?,
                ],
                [
                    i32::try_from(host.screen_width)
                        .map_err(|_| "runtime smoke screen width exceeds i32".to_string())?,
                    i32::try_from(host.screen_height)
                        .map_err(|_| "runtime smoke screen height exceeds i32".to_string())?,
                ],
            );
        }
        let require_srd_draw = srd_draw_smoke || srd_texture_smoke;
        let draw_result = if require_srd_draw {
            workspace.document().and_then(|document| {
                let scene = document.project.scenes.first()?;
                let (host, target_filter) = if let Some(host) = chusan_player_host {
                    let target = match host.target {
                        PreviewTargetSelection::MainScene => CHUSAN_MAIN_SCENE,
                        PreviewTargetSelection::BgScene => CHUSAN_BG_SCENE,
                        PreviewTargetSelection::Unselected => unreachable!(),
                    };
                    let (player_name, context, filter) = if host.common_background {
                        (
                            "CommonBackGround",
                            CHUSAN_COMMON_BACKGROUND_PLAYER.host_context_for_target(
                                target,
                                host.present_width,
                                host.present_height,
                                [host.screen_width, host.screen_height],
                            ),
                            CHUSAN_COMMON_BACKGROUND_PLAYER.initial_srd_target_filter(target),
                        )
                    } else {
                        (
                            "AdvertiseLogo",
                            CHUSAN_ADVERTISE_LOGO_PLAYER.host_context_for_target(
                                target,
                                host.present_width,
                                host.present_height,
                                [host.screen_width, host.screen_height],
                            ),
                            CHUSAN_ADVERTISE_LOGO_PLAYER.initial_srd_target_filter(target),
                        )
                    };
                    eprintln!(
                        "SRD smoke host={player_name}/{} first_calc=identity present={}x{} screen={}x{}",
                        target.name,
                        host.present_width,
                        host.present_height,
                        host.screen_width,
                        host.screen_height
                    );
                    (
                        context.expect("validated Chusan SrPlayer host dimensions"),
                        Some(filter),
                    )
                } else {
                    eprintln!(
                        "SRD smoke host=diagnostic-project-camera first_calc=identity target_width={}",
                        scene.width.max(1.0)
                    );
                    (
                        diagnostic_project_camera_smoke_host(
                            &document.project,
                            scene.width.max(1.0),
                            [scene.width.max(1.0) as u32, scene.height.max(1.0) as u32],
                        ),
                        None,
                    )
                };
                let draws = if srd_texture_smoke {
                    build_evidence_complete_initial_image_draws(
                        &document.project,
                        &document.textures,
                        0,
                        host,
                    )
                } else {
                    let animation_set = scene.animation_sets.first().ok_or_else(|| {
                        crate::srd_draw::SrdDrawError(
                            "SRD draw smoke scene has no ANMS entry".to_string(),
                        )
                    });
                    animation_set.and_then(|animation_set| {
                        build_evidence_complete_animation_set_image_draws(
                            &document.project,
                            &document.textures,
                            0,
                            0,
                            animation_set.runtime_duration as f32,
                            host,
                        )
                    })
                }
                .map(|mut draws| {
                    if let Some(filter) = target_filter {
                        draws.retain(|draw| filter.accepts(draw.packet));
                    }
                    draws
                });
                Some((
                    draws,
                    [scene.width.max(1.0) as u32, scene.height.max(1.0) as u32],
                ))
            })
        } else {
            if workspace.document().is_some() {
                workspace.set_composition_unavailable_reason(Some(
                    "Host target/camera profile not selected".to_string(),
                ));
            }
            None
        };
        let (mut srd_renderer, mut srd_draws, mut composition_size) = match draw_result {
            Some((Ok(draws), size)) if !draws.is_empty() => {
                let renderer = SrdDx9Renderer::new(d3d9.device())
                    .map_err(|error| format!("failed to create the SRD D3D9 renderer: {error}"))?;
                (Some(renderer), draws, Some(size))
            }
            Some((Err(error), _)) if require_srd_draw => {
                return Err(format!(
                    "failed to build evidence-complete SRD draws: {error}"
                ));
            }
            _ if require_srd_draw => {
                return Err("the selected SRD did not produce an evidence-complete draw".into());
            }
            _ => (None, Vec::new(), None),
        };
        if srd_texture_smoke {
            srd_draws.retain(|draw| {
                (draw.layer_index, draw.node_index) == (4, 4)
                    && draw.shader_key == FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY
            });
            if srd_draws.len() != 1 {
                return Err(format!(
                    "textured SRD smoke expected exactly layer 4/node 4 with shader key EAEBABBAABGAAAAAAA, found {} draws",
                    srd_draws.len()
                ));
            }
            if chusan_player_host.is_some() {
                eprintln!(
                    "AdvertiseLogo textured smoke vertices={:?}",
                    srd_draws[0].quad.vertices.map(|vertex| vertex.position)
                );
            }
        }
        let required_texture_indices = srd_draws
            .iter()
            .flat_map(|draw| draw.texture_bindings.iter().flatten())
            .map(|binding| binding.texture_index)
            .collect::<Vec<_>>();
        let srd_textures = if required_texture_indices.is_empty() {
            None
        } else {
            let document = workspace
                .document()
                .ok_or_else(|| "SRD texture loading requires a document".to_string())?;
            let game_data_root = find_game_data_root(&document.path).ok_or_else(|| {
                format!(
                    "could not locate the game data root above {}",
                    document.path.display()
                )
            })?;
            Some(
                SrdD3d9TextureSet::load_required(
                    d3d9.device(),
                    &game_data_root,
                    &document.textures,
                    required_texture_indices,
                )
                .map_err(|error| format!("failed to load SRD textures: {error}"))?,
            )
        };
        let fennel_resources = if srd_fennel_smoke {
            let document = workspace
                .document()
                .ok_or_else(|| "--srd-fennel-smoke requires an SRD document".to_string())?;
            let scene = document
                .project
                .scenes
                .first()
                .ok_or_else(|| "Fennel smoke SRD has no scene".to_string())?;
            let size = checked_scene_composition_size(scene.width, scene.height)?;
            let game_data_root = find_game_data_root(&document.path).ok_or_else(|| {
                format!(
                    "could not locate the game data root above {}",
                    document.path.display()
                )
            })?;
            let mut font_registry = FennelFontSlotRegistry::default();
            let assignments = assign_fennel_font_resource_requests(
                &mut font_registry,
                collect_fennel_font_resource_requests(&document.project)
                    .map_err(|error| error.to_string())?,
            );
            let (runtime_fonts, atlases, atlas_routes) =
                load_fennel_resources(d3d9.device(), &game_data_root, &assignments)?;
            let host =
                diagnostic_project_camera_smoke_host(&document.project, scene.width.max(1.0), size);
            let draws = build_evidence_complete_initial_fennel_draws(
                &document.project,
                0,
                host,
                &font_registry,
                &runtime_fonts,
                false,
            )
            .map_err(|error| format!("failed to build evidence-complete Fennel draws: {error}"))?;
            if draws.is_empty() {
                return Err("the selected SRD did not produce a 2D RFZ Fennel draw".into());
            }
            let total_vertices = draws
                .iter()
                .flat_map(|draw| &draw.batches)
                .map(|batch| batch.vertices.len())
                .sum::<usize>();
            let mut min_position = [f32::INFINITY; 3];
            let mut max_position = [f32::NEG_INFINITY; 3];
            let mut nonzero_primary_alpha = 0usize;
            let mut texture_tokens = Vec::new();
            for batch in draws.iter().flat_map(|draw| &draw.batches) {
                texture_tokens.push(batch.texture_token);
                for vertex in &batch.vertices {
                    for axis in 0..3 {
                        min_position[axis] = min_position[axis].min(vertex.position[axis]);
                        max_position[axis] = max_position[axis].max(vertex.position[axis]);
                    }
                    nonzero_primary_alpha += usize::from(vertex.primary_color_bgra[3] != 0);
                }
            }
            eprintln!(
                "Fennel smoke draws={} vertices={} fonts={} bbox={:?}..{:?} nonzero_primary_alpha={} texture_tokens={:?}",
                draws.len(),
                total_vertices,
                atlases.len(),
                min_position,
                max_position,
                nonzero_primary_alpha,
                texture_tokens,
            );
            if srd_renderer.is_none() {
                srd_renderer = Some(SrdDx9Renderer::new(d3d9.device()).map_err(|error| {
                    format!("failed to create the Composition target renderer: {error}")
                })?);
            }
            composition_size = Some(size);
            (
                Some(
                    FennelDx9Renderer::new(d3d9.device())
                        .map_err(|error| format!("failed to create Fennel renderer: {error}"))?,
                ),
                atlases,
                atlas_routes,
                draws,
            )
        } else {
            (None, BTreeMap::new(), BTreeMap::new(), Vec::new())
        };
        let (fennel_renderer, fennel_atlases, fennel_atlas_routes, fennel_draws) = fennel_resources;
        let composition_texture_id =
            if let (Some(renderer), Some(size)) = (srd_renderer.as_mut(), composition_size) {
                renderer
                    .configure_composition_target(size[0], size[1])
                    .map_err(|error| format!("failed to create SRD composition target: {error}"))?;
                let texture_id = imgui_renderer.textures_mut().insert(
                    renderer
                        .composition_texture()
                        .map_err(|error| error.to_string())?,
                );
                workspace.set_composition_texture(Some((texture_id, size)));
                Some(texture_id)
            } else {
                None
            };

        Ok(Self {
            window,
            d3d9,
            imgui,
            platform,
            imgui_renderer,
            srd_renderer,
            srd_textures,
            srd_draws,
            runtime_draws: Vec::new(),
            runtime_submission: Vec::new(),
            fennel_renderer,
            fennel_atlases,
            fennel_atlas_routes,
            fennel_draws,
            composition_texture_id,
            applied_preview_host: None,
            verify_srd_pixels: srd_draw_smoke,
            verify_textured_pixels: srd_texture_smoke,
            verify_fennel_pixels: srd_fennel_smoke,
            verify_runtime_pixels: srd_runtime_smoke.is_some(),
            verify_hidpi: smoke_test,
            workspace,
            dpi_factor,
            last_frame: Instant::now(),
        })
    }

    fn handle_scale_factor_change(&mut self) {
        let dpi_factor = self.platform.hidpi_factor();
        if (dpi_factor - self.dpi_factor).abs() <= f64::EPSILON {
            return;
        }
        self.imgui_renderer
            .invalidate_device_objects(&mut self.imgui);
        configure_imgui_fonts(&mut self.imgui, dpi_factor);
        self.dpi_factor = dpi_factor;
        self.d3d9.resize(self.window.inner_size());
    }

    fn render_frame(&mut self) -> Result<D3d9ExFrameStatus, String> {
        match self.d3d9.status().map_err(|error| error.to_string())? {
            D3d9ExDeviceStatus::Ready => {}
            D3d9ExDeviceStatus::NeedsReset => {
                if let Some(texture_id) = self.composition_texture_id.take() {
                    self.imgui_renderer.textures_mut().remove(texture_id);
                    self.workspace.set_composition_texture(None);
                }
                self.imgui_renderer
                    .invalidate_device_objects(&mut self.imgui);
                if let Some(renderer) = &mut self.srd_renderer {
                    renderer.invalidate_device_objects();
                }
                if let Some(textures) = &mut self.srd_textures {
                    textures.invalidate_device_objects();
                }
                if let Some(renderer) = &mut self.fennel_renderer {
                    renderer.invalidate_device_objects();
                }
                for atlas in self.fennel_atlases.values_mut() {
                    atlas.invalidate_device_objects();
                }
                self.d3d9.reset().map_err(|error| error.to_string())?;
                self.imgui_renderer
                    .create_device_objects(&mut self.imgui)
                    .map_err(|error| error.to_string())?;
                if let Some(renderer) = &mut self.srd_renderer {
                    renderer
                        .create_device_objects()
                        .map_err(|error| error.to_string())?;
                    let size = renderer.composition_size().ok_or_else(|| {
                        "SRD composition target was not recreated after ResetEx".to_string()
                    })?;
                    let texture_id = self.imgui_renderer.textures_mut().insert(
                        renderer
                            .composition_texture()
                            .map_err(|error| error.to_string())?,
                    );
                    self.workspace
                        .set_composition_texture(Some((texture_id, size)));
                    self.composition_texture_id = Some(texture_id);
                }
                if let Some(textures) = &mut self.srd_textures {
                    textures
                        .create_device_objects(self.d3d9.device())
                        .map_err(|error| error.to_string())?;
                }
                if let Some(renderer) = &mut self.fennel_renderer {
                    renderer
                        .create_device_objects()
                        .map_err(|error| error.to_string())?;
                }
                for atlas in self.fennel_atlases.values_mut() {
                    atlas
                        .create_device_objects(self.d3d9.device())
                        .map_err(|error| error.to_string())?;
                }
            }
            D3d9ExDeviceStatus::DeviceLost => return Ok(D3d9ExFrameStatus::DeviceLost),
            D3d9ExDeviceStatus::Minimized => return Ok(D3d9ExFrameStatus::Minimized),
        }

        let now = Instant::now();
        self.imgui.io_mut().update_delta_time(now - self.last_frame);
        self.last_frame = now;
        self.platform
            .prepare_frame(self.imgui.io_mut(), &self.window)
            .map_err(|error| error.to_string())?;
        if self.verify_runtime_pixels
            || (!self.verify_srd_pixels
                && !self.verify_textured_pixels
                && !self.verify_fennel_pixels)
        {
            self.sync_preview_host();
        }
        if self.verify_hidpi {
            validate_hidpi_frame_contract(&self.window, &self.platform, &self.imgui)?;
        }
        let ui = self.imgui.frame();
        self.workspace.draw(ui);
        self.platform.prepare_render(ui, &self.window);
        let draw_data = self.imgui.render();

        self.d3d9
            .clear_and_begin_scene(CLEAR_COLOR_ARGB)
            .map_err(|error| error.to_string())?;
        let composition_external =
            if self.verify_srd_pixels || self.verify_textured_pixels || self.verify_fennel_pixels {
                SrdDx9ExternalContext::smoke_without_scissor()
            } else {
                SrdDx9ExternalContext::without_scissor()
            };
        if !self.runtime_submission.is_empty() {
            let runtime_draws = &self.runtime_draws;
            let runtime_submission = &self.runtime_submission;
            let srd_textures = self.srd_textures.as_ref();
            let fennel_renderer = self.fennel_renderer.as_mut();
            let fennel_atlases = &self.fennel_atlases;
            let fennel_atlas_routes = &self.fennel_atlas_routes;
            let renderer = self
                .srd_renderer
                .as_mut()
                .ok_or_else(|| "Runtime target stream lost the Composition renderer".to_string())?;
            renderer
                .render_runtime_to_composition(CLEAR_COLOR_ARGB, |renderer| {
                    render_runtime_target_submission(
                        renderer,
                        fennel_renderer,
                        runtime_draws,
                        runtime_submission,
                        srd_textures,
                        fennel_atlases,
                        fennel_atlas_routes,
                        composition_external,
                        true,
                        true,
                    )
                })
                .map_err(|error| format!("Runtime target composition draw failed: {error}"))?;
        }
        if self.verify_runtime_pixels {
            let fennel_sources = self
                .runtime_submission
                .iter()
                .flat_map(|command| &command.sources)
                .filter(|source| {
                    matches!(
                        source,
                        EvidenceRuntimeTargetCommandSource::FennelBatch { .. }
                    )
                })
                .count();
            let number_sources = self
                .runtime_submission
                .iter()
                .flat_map(|command| &command.sources)
                .filter(|source| {
                    matches!(
                        source,
                        EvidenceRuntimeTargetCommandSource::NumberGlyph { .. }
                    )
                })
                .count();
            if fennel_sources == 0 && number_sources == 0 {
                return Err(
                    "runtime smoke target stream contains neither Fennel nor NumberGlyph sources"
                        .to_string(),
                );
            }
            self.d3d9.end_scene().map_err(|error| error.to_string())?;
            let complete = self
                .srd_renderer
                .as_ref()
                .ok_or_else(|| "runtime smoke lost the Composition renderer".to_string())?
                .read_composition_bgra()
                .map_err(|error| format!("runtime smoke readback failed: {error}"))?;
            if fennel_sources != 0 {
                self.d3d9.begin_scene().map_err(|error| error.to_string())?;
                {
                    let runtime_draws = &self.runtime_draws;
                    let runtime_submission = &self.runtime_submission;
                    let srd_textures = self.srd_textures.as_ref();
                    let fennel_renderer = self.fennel_renderer.as_mut();
                    let fennel_atlases = &self.fennel_atlases;
                    let fennel_atlas_routes = &self.fennel_atlas_routes;
                    let renderer = self.srd_renderer.as_mut().ok_or_else(|| {
                        "runtime smoke lost the Composition renderer during Fennel comparison"
                            .to_string()
                    })?;
                    renderer
                        .render_runtime_to_composition(CLEAR_COLOR_ARGB, |renderer| {
                            render_runtime_target_submission(
                                renderer,
                                fennel_renderer,
                                runtime_draws,
                                runtime_submission,
                                srd_textures,
                                fennel_atlases,
                                fennel_atlas_routes,
                                composition_external,
                                false,
                                true,
                            )
                        })
                        .map_err(|error| {
                            format!("runtime smoke no-Fennel comparison draw failed: {error}")
                        })?;
                }
                self.d3d9.end_scene().map_err(|error| error.to_string())?;
                let comparison = self
                    .srd_renderer
                    .as_ref()
                    .ok_or_else(|| "runtime smoke lost its Fennel comparison target".to_string())?
                    .read_composition_bgra()
                    .map_err(|error| {
                        format!("runtime smoke Fennel comparison readback failed: {error}")
                    })?;
                let changed_pixels = runtime_rgb_difference(&complete.bgra, &comparison.bgra);
                if changed_pixels == 0 {
                    return Err(
                        "runtime smoke Fennel batches changed no Composition RGB pixels"
                            .to_string(),
                    );
                }
                eprintln!(
                    "runtime target Fennel sources={fennel_sources} changed_pixels={changed_pixels}"
                );
            }
            if number_sources != 0 {
                self.d3d9.begin_scene().map_err(|error| error.to_string())?;
                {
                    let runtime_draws = &self.runtime_draws;
                    let runtime_submission = &self.runtime_submission;
                    let srd_textures = self.srd_textures.as_ref();
                    let fennel_renderer = self.fennel_renderer.as_mut();
                    let fennel_atlases = &self.fennel_atlases;
                    let fennel_atlas_routes = &self.fennel_atlas_routes;
                    let renderer = self.srd_renderer.as_mut().ok_or_else(|| {
                        "runtime smoke lost the Composition renderer during Number comparison"
                            .to_string()
                    })?;
                    renderer
                        .render_runtime_to_composition(CLEAR_COLOR_ARGB, |renderer| {
                            render_runtime_target_submission(
                                renderer,
                                fennel_renderer,
                                runtime_draws,
                                runtime_submission,
                                srd_textures,
                                fennel_atlases,
                                fennel_atlas_routes,
                                composition_external,
                                true,
                                false,
                            )
                        })
                        .map_err(|error| {
                            format!("runtime smoke no-Number comparison draw failed: {error}")
                        })?;
                }
                self.d3d9.end_scene().map_err(|error| error.to_string())?;
                let comparison = self
                    .srd_renderer
                    .as_ref()
                    .ok_or_else(|| "runtime smoke lost its Number comparison target".to_string())?
                    .read_composition_bgra()
                    .map_err(|error| {
                        format!("runtime smoke Number comparison readback failed: {error}")
                    })?;
                let changed_pixels = runtime_rgb_difference(&complete.bgra, &comparison.bgra);
                if changed_pixels == 0 {
                    return Err(
                        "runtime smoke NumberGlyph draws changed no Composition RGB pixels"
                            .to_string(),
                    );
                }
                eprintln!(
                    "runtime target NumberGlyph sources={number_sources} changed_pixels={changed_pixels}"
                );
            }
            self.d3d9.begin_scene().map_err(|error| error.to_string())?;
        }
        if !self.srd_draws.is_empty()
            && let Some(renderer) = &mut self.srd_renderer
        {
            renderer
                .render_to_composition(
                    &self.srd_draws,
                    composition_external,
                    self.srd_textures.as_ref(),
                    CLEAR_COLOR_ARGB,
                )
                .map_err(|error| format!("SRD composition draw failed: {error}"))?;
        }
        if self.verify_fennel_pixels {
            let target = self
                .srd_renderer
                .as_ref()
                .ok_or_else(|| "Fennel smoke lost the Composition target".to_string())?;
            let renderer = self
                .fennel_renderer
                .as_mut()
                .ok_or_else(|| "Fennel smoke lost the font renderer".to_string())?;
            let draws = &self.fennel_draws;
            let atlases = &self.fennel_atlases;
            let atlas_routes = &self.fennel_atlas_routes;
            target
                .render_custom_to_composition(CLEAR_COLOR_ARGB, || {
                    render_fennel_draws(
                        renderer,
                        draws,
                        atlases,
                        atlas_routes,
                        composition_external,
                    )
                })
                .map_err(|error| format!("Fennel composition draw failed: {error}"))?;
        }
        if self.verify_srd_pixels
            && let Some(renderer) = &mut self.srd_renderer
        {
            renderer
                .render(
                    &self.srd_draws,
                    SrdDx9ExternalContext::smoke_without_scissor(),
                    self.srd_textures.as_ref(),
                )
                .map_err(|error| format!("SRD D3D9 draw submission failed: {error}"))?;
        }
        if self.verify_srd_pixels {
            self.d3d9.end_scene().map_err(|error| error.to_string())?;
            let renderer = self
                .srd_renderer
                .as_ref()
                .ok_or_else(|| "SRD draw smoke lost the composition renderer".to_string())?;
            let readback = renderer
                .read_composition_bgra()
                .map_err(|error| format!("SRD composition readback failed: {error}"))?;
            let diagnostic = analyze_composition_readback(
                readback.width,
                readback.height,
                &readback.bgra,
                [0x26, 0x22, 0x20],
            )?;
            if diagnostic.changed_pixels == 0 {
                return Err(
                    "SRD draw did not change any Composition pixel from the clear color".into(),
                );
            }
            eprintln!(
                "SRD composition pixels={} white_pixels={} bbox=({}, {})..({}, {}) fnv1a64={:016X}",
                diagnostic.changed_pixels,
                diagnostic.white_pixels,
                diagnostic.min_x,
                diagnostic.min_y,
                diagnostic.max_x,
                diagnostic.max_y,
                diagnostic.fnv1a64,
            );
            let size = self.window.inner_size();
            let pixel = self
                .d3d9
                .read_backbuffer_pixel(size.width / 2 + 1, size.height / 4)
                .map_err(|error| format!("SRD backbuffer readback failed: {error}"))?;
            if pixel[..3] == [0x26, 0x22, 0x20] {
                return Err(format!(
                    "SRD direct draw did not change the diagnostic backbuffer sample from clear; raw BGRA/XRGB {pixel:02X?}"
                ));
            }
            self.d3d9.begin_scene().map_err(|error| error.to_string())?;
        }
        if self.verify_textured_pixels {
            self.d3d9.end_scene().map_err(|error| error.to_string())?;
            let readback = self
                .srd_renderer
                .as_ref()
                .ok_or_else(|| "textured SRD smoke lost the composition renderer".to_string())?
                .read_composition_bgra()
                .map_err(|error| format!("textured SRD composition readback failed: {error}"))?;
            let diagnostic = analyze_composition_readback(
                readback.width,
                readback.height,
                &readback.bgra,
                [0x26, 0x22, 0x20],
            )?;
            if diagnostic.changed_pixels == 0 {
                return Err(
                    "textured SRD draw did not change any Composition pixel from the clear color"
                        .into(),
                );
            }
            eprintln!(
                "textured SRD composition pixels={} white_pixels={} bbox=({}, {})..({}, {}) fnv1a64={:016X}",
                diagnostic.changed_pixels,
                diagnostic.white_pixels,
                diagnostic.min_x,
                diagnostic.min_y,
                diagnostic.max_x,
                diagnostic.max_y,
                diagnostic.fnv1a64,
            );
            self.d3d9.begin_scene().map_err(|error| error.to_string())?;
        }
        if self.verify_fennel_pixels {
            self.d3d9.end_scene().map_err(|error| error.to_string())?;
            let readback = self
                .srd_renderer
                .as_ref()
                .ok_or_else(|| "Fennel smoke lost the Composition target".to_string())?
                .read_composition_bgra()
                .map_err(|error| format!("Fennel composition readback failed: {error}"))?;
            let diagnostic = analyze_composition_readback(
                readback.width,
                readback.height,
                &readback.bgra,
                [0x26, 0x22, 0x20],
            )?;
            if diagnostic.changed_pixels == 0 {
                return Err("Fennel draw did not change any Composition pixel".into());
            }
            eprintln!(
                "Fennel composition pixels={} white_pixels={} bbox=({}, {})..({}, {}) fnv1a64={:016X}",
                diagnostic.changed_pixels,
                diagnostic.white_pixels,
                diagnostic.min_x,
                diagnostic.min_y,
                diagnostic.max_x,
                diagnostic.max_y,
                diagnostic.fnv1a64,
            );
            self.d3d9.begin_scene().map_err(|error| error.to_string())?;
        }
        let render_result = self
            .imgui_renderer
            .render(draw_data)
            .map_err(|error| error.to_string());
        let present_result = self
            .d3d9
            .end_scene_and_present()
            .map_err(|error| error.to_string());
        render_result?;
        present_result
    }

    fn sync_preview_host(&mut self) {
        let settings = self.workspace.preview_host_settings();
        if self.applied_preview_host == Some(settings) {
            return;
        }
        self.applied_preview_host = Some(settings);
        self.clear_preview_resources();

        let settings = match self.workspace.validate_preview_host_settings() {
            Ok(settings) => settings,
            Err(reason) => {
                self.workspace
                    .set_composition_unavailable_reason(Some(reason));
                return;
            }
        };
        if settings.scissor != PreviewScissorSelection::Disabled {
            self.workspace.set_composition_unavailable_reason(Some(
                "The selected external scissor mode is not implemented".to_string(),
            ));
            return;
        }

        if let Err(error) = self.rebuild_preview_resources(settings) {
            self.clear_preview_resources();
            self.workspace
                .set_composition_unavailable_reason(Some(error));
        }
    }

    fn clear_preview_resources(&mut self) {
        if let Some(texture_id) = self.composition_texture_id.take() {
            self.imgui_renderer.textures_mut().remove(texture_id);
        }
        self.workspace.set_composition_texture(None);
        self.srd_renderer = None;
        self.srd_textures = None;
        self.srd_draws.clear();
        self.runtime_draws.clear();
        self.runtime_submission.clear();
        self.fennel_renderer = None;
        self.fennel_atlases.clear();
        self.fennel_atlas_routes.clear();
        self.fennel_draws.clear();
    }

    fn rebuild_preview_resources(&mut self, settings: PreviewHostSettings) -> Result<(), String> {
        let target = match settings.target {
            PreviewTargetSelection::MainScene => CHUSAN_MAIN_SCENE,
            PreviewTargetSelection::BgScene => CHUSAN_BG_SCENE,
            PreviewTargetSelection::Unselected => {
                return Err("Select a Chusan target profile".to_string());
            }
        };
        let present_width = u32::try_from(settings.present_width)
            .map_err(|_| "Present width must be positive".to_string())?;
        let present_height = u32::try_from(settings.present_height)
            .map_err(|_| "Present height must be positive".to_string())?;
        let screen_width = u32::try_from(settings.screen_width)
            .map_err(|_| "Target screen width must be positive".to_string())?;
        let screen_height = u32::try_from(settings.screen_height)
            .map_err(|_| "Target screen height must be positive".to_string())?;
        let (host, target_filter) = match settings.player {
            PreviewPlayerSelection::AdvertiseLogo => (
                CHUSAN_ADVERTISE_LOGO_PLAYER.host_context_for_target(
                    target,
                    present_width,
                    present_height,
                    [screen_width, screen_height],
                ),
                CHUSAN_ADVERTISE_LOGO_PLAYER.initial_srd_target_filter(target),
            ),
            PreviewPlayerSelection::CommonBackground => (
                CHUSAN_COMMON_BACKGROUND_PLAYER.host_context_for_target(
                    target,
                    present_width,
                    present_height,
                    [screen_width, screen_height],
                ),
                CHUSAN_COMMON_BACKGROUND_PLAYER.initial_srd_target_filter(target),
            ),
            PreviewPlayerSelection::Unselected => {
                return Err("Select a Chusan SrPlayer host profile".to_string());
            }
        };
        let host = host.map_err(|error| error.to_string())?;

        let selected_runtime_text_inputs =
            self.workspace.selected_scene_fennel_runtime_text_inputs();
        let document = self
            .workspace
            .document()
            .ok_or_else(|| "No SRD loaded".to_string())?;
        let scene = document
            .project
            .scenes
            .get(settings.scene_index)
            .ok_or_else(|| format!("Scene {} is no longer available", settings.scene_index))?;
        let composition_size = checked_scene_composition_size(scene.width, scene.height)?;
        scene
            .animation_sets
            .get(settings.animation_set_index)
            .ok_or_else(|| {
                format!(
                    "Animation set {} is no longer available",
                    settings.animation_set_index
                )
            })?;
        let mut font_registry = FennelFontSlotRegistry::default();
        let assignments = assign_fennel_font_resource_requests(
            &mut font_registry,
            collect_fennel_font_resource_requests(&document.project)
                .map_err(|error| error.to_string())?,
        );
        let game_data_root = if assignments.is_empty() {
            None
        } else {
            Some(find_game_data_root(&document.path).ok_or_else(|| {
                format!(
                    "Could not locate the game data root above {} for RFZ resources",
                    document.path.display()
                )
            })?)
        };
        let (runtime_fonts, fennel_atlases, fennel_atlas_routes) = if assignments.is_empty() {
            (BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
        } else {
            load_fennel_resources(
                self.d3d9.device(),
                game_data_root
                    .as_ref()
                    .expect("non-empty assignments require a game data root"),
                &assignments,
            )?
        };
        let runtime_text_inputs = selected_runtime_text_inputs
            .into_iter()
            .map(|((layer_index, node_index), input)| {
                (
                    FennelRuntimeTextCastKey {
                        owner: ReferenceLayerParent::ProjectLayer(ReferenceTarget {
                            scene_index: settings.scene_index,
                            layer_index,
                        }),
                        node_index,
                    },
                    input,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let runtime_draws = build_evidence_complete_animation_set_runtime_cast_draws(
            &document.project,
            &document.textures,
            settings.scene_index,
            settings.animation_set_index,
            settings.animation_frame as f32,
            host,
            &font_registry,
            &runtime_fonts,
            false,
            &runtime_text_inputs,
        )
        .map_err(|error| error.to_string())?;
        let target_profile = target
            .scene_pass_profile()
            .map_err(|error| error.to_string())?;
        let runtime_submission = build_evidence_filtered_merged_runtime_target_submission(
            &runtime_draws,
            &target_profile,
            target_filter,
        )
        .map_err(|error| error.to_string())?;
        if runtime_submission.is_empty() {
            return Err(format!(
                "No target-admitted evidence-complete GPU draw for {}",
                target.name
            ));
        }
        let required_texture_indices = runtime_draws
            .iter()
            .filter_map(|draw| match draw {
                EvidenceCompleteRuntimeCastDraw::Image(draw) => Some(draw),
                EvidenceCompleteRuntimeCastDraw::SliceCell(cell) => Some(&cell.draw),
                EvidenceCompleteRuntimeCastDraw::NumberGlyph(glyph) => Some(&glyph.draw),
                EvidenceCompleteRuntimeCastDraw::Fennel(_) => None,
            })
            .flat_map(|draw| draw.texture_bindings.iter().flatten())
            .map(|binding| binding.texture_index)
            .collect::<Vec<_>>();
        let textures = if required_texture_indices.is_empty() {
            None
        } else {
            let texture_root = find_game_data_root(&document.path).ok_or_else(|| {
                format!(
                    "Could not locate the game data root above {}",
                    document.path.display()
                )
            })?;
            Some(
                SrdD3d9TextureSet::load_required(
                    self.d3d9.device(),
                    &texture_root,
                    &document.textures,
                    required_texture_indices,
                )
                .map_err(|error| format!("Failed to load SRD textures: {error}"))?,
            )
        };

        let mut renderer = SrdDx9Renderer::new(self.d3d9.device())
            .map_err(|error| format!("Failed to create the SRD D3D9 renderer: {error}"))?;
        renderer
            .configure_composition_target(composition_size[0], composition_size[1])
            .map_err(|error| format!("Failed to create SRD Composition target: {error}"))?;
        let texture_id = self.imgui_renderer.textures_mut().insert(
            renderer
                .composition_texture()
                .map_err(|error| error.to_string())?,
        );
        self.workspace
            .set_composition_texture(Some((texture_id, composition_size)));
        self.composition_texture_id = Some(texture_id);
        self.srd_renderer = Some(renderer);
        self.srd_textures = textures;
        self.srd_draws.clear();
        self.runtime_draws = runtime_draws;
        self.runtime_submission = runtime_submission;
        self.fennel_renderer = self
            .runtime_draws
            .iter()
            .any(|draw| matches!(draw, EvidenceCompleteRuntimeCastDraw::Fennel(_)))
            .then(|| FennelDx9Renderer::new(self.d3d9.device()))
            .transpose()
            .map_err(|error| format!("Failed to create Fennel renderer: {error}"))?;
        self.fennel_atlases = fennel_atlases;
        self.fennel_atlas_routes = fennel_atlas_routes;
        self.fennel_draws.clear();
        Ok(())
    }
}

fn checked_scene_composition_size(width: f32, height: f32) -> Result<[u32; 2], String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(format!(
            "Scene Composition size must be finite and positive, got {width}x{height}"
        ));
    }
    if width.fract() != 0.0 || height.fract() != 0.0 {
        return Err(format!(
            "Fractional Scene Composition sizes are not yet evidence-complete: {width}x{height}"
        ));
    }
    if width > u32::MAX as f32 || height > u32::MAX as f32 {
        return Err(format!(
            "Scene Composition size exceeds D3D9 dimensions: {width}x{height}"
        ));
    }
    Ok([width as u32, height as u32])
}

/// Preserves the existing GPU smoke as an explicit diagnostic host. This is
/// intentionally restricted to smoke flags: the SRD CAM is not a game target
/// Camera and must never become the editor's implicit preview default.
fn diagnostic_project_camera_smoke_host(
    project: &crate::scene::Project,
    target_width: f32,
    target_screen_size: [u32; 2],
) -> SrdHostDrawContext {
    let projection_view = project
        .camera
        .runtime_matrices(target_width)
        .projection_view;
    let render_size = [target_width as u32, target_screen_size[1]];
    SrdHostDrawContext::new(
        Affine3x4::IDENTITY,
        crate::render::SRD_RENDERER_INITIAL_LAYER_KEY,
        Some(SrdRendererProjectTargetContext::new(
            projection_view,
            render_size,
        )),
        projection_view,
        target_screen_size,
    )
}

fn parse_runtime_smoke_argument(
    arguments: &[String],
) -> Result<Option<RuntimeSmokeSelection>, String> {
    let Some(value) = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--srd-runtime-smoke="))
    else {
        return Ok(None);
    };
    let mut fields = value.split(',');
    let scene_index = fields
        .next()
        .ok_or_else(|| "runtime smoke is missing the scene index".to_string())?
        .parse::<usize>()
        .map_err(|error| format!("invalid runtime smoke scene index: {error}"))?;
    let animation_set_index = fields
        .next()
        .ok_or_else(|| "runtime smoke is missing the animation-set index".to_string())?
        .parse::<usize>()
        .map_err(|error| format!("invalid runtime smoke animation-set index: {error}"))?;
    let frame = fields
        .next()
        .ok_or_else(|| "runtime smoke is missing the frame".to_string())?
        .parse::<i32>()
        .map_err(|error| format!("invalid runtime smoke frame: {error}"))?;
    if fields.next().is_some() {
        return Err(
            "--srd-runtime-smoke must use SCENE_INDEX,ANIMATION_SET_INDEX,FRAME".to_string(),
        );
    }
    Ok(Some(RuntimeSmokeSelection {
        scene_index,
        animation_set_index,
        frame,
    }))
}

fn parse_chusan_player_host_argument(
    arguments: &[String],
) -> Result<Option<ChusanPlayerHostArgument>, String> {
    let advertise = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--advertise-logo-host="));
    let common = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--common-background-host="));
    let (common_background, option_name, value) = match (advertise, common) {
        (None, None) => return Ok(None),
        (Some(_), Some(_)) => {
            return Err(
                "choose only one of --advertise-logo-host or --common-background-host".to_string(),
            );
        }
        (Some(value), None) => (false, "--advertise-logo-host", value),
        (None, Some(value)) => (true, "--common-background-host", value),
    };
    let mut fields = value.split('@');
    let target = fields.next().unwrap_or_default();
    let present_size = fields.next().ok_or_else(|| {
        format!("{option_name} must use TARGET@PRESENT_WIDTHxPRESENT_HEIGHT@SCREEN_WIDTHxSCREEN_HEIGHT, for example MainScene@1080x1920@1920x1080")
    })?;
    let screen_size = fields.next().ok_or_else(|| {
        format!("{option_name} requires an explicit screenParam source size after the present size")
    })?;
    if fields.next().is_some() {
        return Err(format!(
            "{option_name} contains too many @-separated fields"
        ));
    }
    let target = match target {
        "MainScene" => PreviewTargetSelection::MainScene,
        "BgScene" => PreviewTargetSelection::BgScene,
        _ => {
            return Err(format!(
                "unsupported Chusan target {target:?}; expected MainScene or BgScene"
            ));
        }
    };
    let (width, height) = present_size.split_once('x').ok_or_else(|| {
        format!("{option_name} present size must use WIDTHxHEIGHT, for example 1080x1920")
    })?;
    let present_width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid Chusan host present width {width:?}"))?;
    let present_height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid Chusan host present height {height:?}"))?;
    if present_width == 0 || present_height == 0 {
        return Err("Chusan host present dimensions must be non-zero".to_string());
    }
    let (width, height) = screen_size.split_once('x').ok_or_else(|| {
        format!("{option_name} screen size must use WIDTHxHEIGHT, for example 1920x1080")
    })?;
    let screen_width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid Chusan host screen width {width:?}"))?;
    let screen_height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid Chusan host screen height {height:?}"))?;
    if screen_width == 0 || screen_height == 0 {
        return Err("Chusan host screen dimensions must be non-zero".to_string());
    }
    Ok(Some(ChusanPlayerHostArgument {
        common_background,
        target,
        present_width,
        present_height,
        screen_width,
        screen_height,
    }))
}

fn editor_ini_path() -> Option<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("APPDATA")?).join("SrdEditor");
    std::fs::create_dir_all(&directory).ok()?;
    Some(directory.join("imgui.ini"))
}

fn find_game_data_root(path: &std::path::Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("data"))
        })
        .map(std::path::Path::to_path_buf)
}

fn load_fennel_resources(
    device: &windows::Win32::Graphics::Direct3D9::IDirect3DDevice9,
    game_data_root: &std::path::Path,
    assignments: &[FennelFontResourceAssignment],
) -> Result<
    (
        BTreeMap<Vec<u8>, RuhunaRuntimeFont>,
        BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
        BTreeMap<u32, FennelAtlasRoute>,
    ),
    String,
> {
    let mut runtime_fonts = BTreeMap::new();
    let mut atlases = BTreeMap::new();
    let mut atlas_routes = BTreeMap::new();
    let mut next_texture_token = 1u32;
    for assignment in assignments {
        if !assignment.slot.first_request
            || !assignment.slot.registered
            || runtime_fonts.contains_key(assignment.request.name.as_slice())
            || !assignment
                .request
                .name
                .iter()
                .map(u8::to_ascii_lowercase)
                .collect::<Vec<_>>()
                .ends_with(b".rfz")
        {
            continue;
        }
        let name = std::str::from_utf8(&assignment.request.name)
            .map_err(|error| format!("RFZ font name is not UTF-8: {error}"))?;
        let path = game_data_root.join("A000/font").join(name);
        let parsed = RuhunaFont::from_rfz(
            &fs::read(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let atlas = RuhunaD3d9AtlasSet::from_font(device, &parsed)
            .map_err(|error| format!("failed to upload {}: {error}", path.display()))?;
        let mut page_tokens = Vec::with_capacity(atlas.page_count());
        for page_index in 0..atlas.page_count() {
            let texture_token = next_texture_token;
            next_texture_token = next_texture_token.checked_add(1).ok_or_else(|| {
                "editor Fennel texture-token space exhausted while loading atlases".to_string()
            })?;
            atlas_routes.insert(
                texture_token,
                FennelAtlasRoute {
                    font_name: assignment.request.name.clone(),
                    page_index,
                },
            );
            page_tokens.push(texture_token);
        }
        let owner_token = u32::try_from(assignment.slot.resource_handle).map_err(|_| {
            format!(
                "Fennel resource handle {} does not fit the runtime glyph token",
                assignment.slot.resource_handle
            )
        })?;
        let runtime = parsed
            .build_runtime_font(owner_token, |page| {
                page_tokens.get(usize::from(page)).copied().unwrap_or(0)
            })
            .map_err(|error| format!("failed to build runtime {}: {error}", path.display()))?;
        runtime_fonts.insert(assignment.request.name.clone(), runtime);
        atlases.insert(assignment.request.name.clone(), atlas);
    }
    Ok((runtime_fonts, atlases, atlas_routes))
}

#[allow(clippy::too_many_arguments)]
fn render_runtime_target_submission(
    srd_renderer: &mut SrdDx9Renderer,
    mut fennel_renderer: Option<&mut FennelDx9Renderer>,
    draws: &[EvidenceCompleteRuntimeCastDraw],
    submission: &[EvidenceMergedRuntimeTargetCommand],
    textures: Option<&SrdD3d9TextureSet>,
    atlases: &BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
    atlas_routes: &BTreeMap<u32, FennelAtlasRoute>,
    external: SrdDx9ExternalContext,
    render_fennel: bool,
    render_number: bool,
) -> windows::core::Result<()> {
    for command in submission {
        if command.vertex_format == 14 && command.primitive_type == 4 {
            if let Some(strip) = build_evidence_merged_runtime_srd_strip(draws, command, |source| {
                render_number
                    || !matches!(
                        source,
                        EvidenceRuntimeTargetCommandSource::NumberGlyph { .. }
                    )
            })
            .map_err(|error| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    error.to_string(),
                )
            })? {
                srd_renderer.render_triangle_strip(
                    strip.state,
                    &strip.vertices,
                    external,
                    textures,
                )?;
            }
            continue;
        }
        if command.vertex_format == 13 && command.primitive_type == 3 {
            if !render_fennel {
                continue;
            }
            let list =
                build_evidence_merged_runtime_fennel_list(draws, command).map_err(|error| {
                    windows::core::Error::new(
                        windows::Win32::Foundation::E_INVALIDARG,
                        error.to_string(),
                    )
                })?;
            let renderer = fennel_renderer.as_deref_mut().ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    "Merged Fennel target command has no D3D9 renderer",
                )
            })?;
            let route = atlas_routes.get(&list.texture_token).ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    format!(
                        "Fennel texture token {:#010x} has no atlas route",
                        list.texture_token
                    ),
                )
            })?;
            let atlas = atlases.get(route.font_name.as_slice()).ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    format!(
                        "Fennel atlas {:?} is not loaded",
                        String::from_utf8_lossy(&route.font_name)
                    ),
                )
            })?;
            let batch = EvidenceCompleteFennelBatch {
                page_index: route.page_index,
                is_2d: list.state.is_2d,
                fixed_constants: list.state.fixed_constants,
                vertices: &list.vertices,
            };
            renderer.render(std::slice::from_ref(&batch), external, atlas)?;
            continue;
        }
        for source in &command.sources {
            match *source {
                EvidenceRuntimeTargetCommandSource::Image { runtime_draw_index } => {
                    let Some(EvidenceCompleteRuntimeCastDraw::Image(draw)) =
                        draws.get(runtime_draw_index)
                    else {
                        return Err(windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            "Runtime target Image source does not match its draw",
                        ));
                    };
                    srd_renderer.render(std::slice::from_ref(draw), external, textures)?;
                }
                EvidenceRuntimeTargetCommandSource::SliceCell { runtime_draw_index } => {
                    let Some(EvidenceCompleteRuntimeCastDraw::SliceCell(cell)) =
                        draws.get(runtime_draw_index)
                    else {
                        return Err(windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            "Runtime target SliceCell source does not match its draw",
                        ));
                    };
                    srd_renderer.render(std::slice::from_ref(&cell.draw), external, textures)?;
                }
                EvidenceRuntimeTargetCommandSource::NumberGlyph { runtime_draw_index } => {
                    if !render_number {
                        continue;
                    }
                    let Some(EvidenceCompleteRuntimeCastDraw::NumberGlyph(glyph)) =
                        draws.get(runtime_draw_index)
                    else {
                        return Err(windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            "Runtime target NumberGlyph source does not match its draw",
                        ));
                    };
                    srd_renderer.render(std::slice::from_ref(&glyph.draw), external, textures)?;
                }
                EvidenceRuntimeTargetCommandSource::FennelBatch {
                    runtime_draw_index,
                    batch_index,
                } => {
                    if !render_fennel {
                        continue;
                    }
                    let Some(EvidenceCompleteRuntimeCastDraw::Fennel(draw)) =
                        draws.get(runtime_draw_index)
                    else {
                        return Err(windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            "Runtime target Fennel source does not match its draw",
                        ));
                    };
                    let batch = draw.batches.get(batch_index).ok_or_else(|| {
                        windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            "Runtime target Fennel batch index is outside its draw",
                        )
                    })?;
                    let renderer = fennel_renderer.as_deref_mut().ok_or_else(|| {
                        windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            "Runtime target Fennel source has no D3D9 renderer",
                        )
                    })?;
                    let route = atlas_routes.get(&batch.texture_token).ok_or_else(|| {
                        windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            format!(
                                "Fennel texture token {:#010x} has no atlas route",
                                batch.texture_token
                            ),
                        )
                    })?;
                    let atlas = atlases.get(route.font_name.as_slice()).ok_or_else(|| {
                        windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            format!(
                                "Fennel atlas {:?} is not loaded",
                                String::from_utf8_lossy(&route.font_name)
                            ),
                        )
                    })?;
                    let routed_batch = EvidenceCompleteFennelBatch {
                        page_index: route.page_index,
                        is_2d: draw.is_2d,
                        fixed_constants: draw.fixed_constants,
                        vertices: &batch.vertices,
                    };
                    renderer.render(std::slice::from_ref(&routed_batch), external, atlas)?;
                }
            }
        }
    }
    Ok(())
}

fn render_fennel_draws(
    renderer: &mut FennelDx9Renderer,
    draws: &[EvidenceCompleteFennelDraw],
    atlases: &BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
    atlas_routes: &BTreeMap<u32, FennelAtlasRoute>,
    external: SrdDx9ExternalContext,
) -> windows::core::Result<()> {
    for draw in draws {
        for batch in &draw.batches {
            let route = atlas_routes.get(&batch.texture_token).ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    format!(
                        "Fennel texture token {:#010x} has no atlas route",
                        batch.texture_token
                    ),
                )
            })?;
            let atlas = atlases.get(route.font_name.as_slice()).ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_INVALIDARG,
                    format!(
                        "Fennel atlas {:?} is not loaded",
                        String::from_utf8_lossy(&route.font_name)
                    ),
                )
            })?;
            let routed_batch = EvidenceCompleteFennelBatch {
                page_index: route.page_index,
                is_2d: draw.is_2d,
                fixed_constants: draw.fixed_constants,
                vertices: &batch.vertices,
            };
            renderer.render(std::slice::from_ref(&routed_batch), external, atlas)?;
        }
    }
    Ok(())
}

fn configure_imgui_fonts(imgui: &mut Context, dpi_factor: f64) {
    let dpi_factor = dpi_factor.max(0.5) as f32;
    let fonts = imgui.fonts();
    fonts.clear();
    fonts.add_font(&[FontSource::DefaultFontData {
        config: Some(FontConfig {
            size_pixels: 13.0 * dpi_factor,
            pixel_snap_h: true,
            ..FontConfig::default()
        }),
    }]);
    imgui.io_mut().font_global_scale = 1.0 / dpi_factor;
}

fn validate_hidpi_frame_contract(
    window: &Window,
    platform: &WinitPlatform,
    imgui: &Context,
) -> Result<(), String> {
    let dpi_factor = platform.hidpi_factor();
    if (window.scale_factor() - dpi_factor).abs() > f64::EPSILON {
        return Err(format!(
            "HiDPI platform factor {dpi_factor} differs from window factor {}",
            window.scale_factor()
        ));
    }
    let io = imgui.io();
    let framebuffer_scale = f64::from(io.display_framebuffer_scale[0]);
    if (framebuffer_scale - dpi_factor).abs() > 0.0001
        || (f64::from(io.display_framebuffer_scale[1]) - dpi_factor).abs() > 0.0001
    {
        return Err(format!(
            "ImGui framebuffer scale {:?} differs from window DPI factor {dpi_factor}",
            io.display_framebuffer_scale
        ));
    }
    let physical = window.inner_size();
    let expected_width = f64::from(io.display_size[0]) * framebuffer_scale;
    let expected_height = f64::from(io.display_size[1]) * framebuffer_scale;
    if (expected_width - f64::from(physical.width)).abs() > 1.0
        || (expected_height - f64::from(physical.height)).abs() > 1.0
    {
        return Err(format!(
            "HiDPI logical size {:?} times scale {framebuffer_scale} does not match physical backbuffer {}x{}",
            io.display_size, physical.width, physical.height
        ));
    }
    Ok(())
}

struct CompositionReadbackDiagnostic {
    changed_pixels: usize,
    white_pixels: usize,
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    fnv1a64: u64,
}

fn runtime_rgb_difference(left: &[u8], right: &[u8]) -> usize {
    left.chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(left, right)| left[..3] != right[..3])
        .count()
}

fn analyze_composition_readback(
    width: u32,
    height: u32,
    bgra: &[u8],
    clear_bgr: [u8; 3],
) -> Result<CompositionReadbackDiagnostic, String> {
    let expected_len = width as usize * height as usize * 4;
    if bgra.len() != expected_len {
        return Err(format!(
            "Composition readback has {} bytes, expected {expected_len} for {width}x{height}",
            bgra.len()
        ));
    }
    let mut result = CompositionReadbackDiagnostic {
        changed_pixels: 0,
        white_pixels: 0,
        min_x: width,
        min_y: height,
        max_x: 0,
        max_y: 0,
        fnv1a64: 0xcbf2_9ce4_8422_2325,
    };
    for (index, pixel) in bgra.chunks_exact(4).enumerate() {
        for byte in pixel {
            result.fnv1a64 ^= u64::from(*byte);
            result.fnv1a64 = result.fnv1a64.wrapping_mul(0x0000_0100_0000_01b3);
        }
        if pixel[..3] == [0xff, 0xff, 0xff] {
            result.white_pixels += 1;
        }
        if pixel[..3] == clear_bgr {
            continue;
        }
        let x = index as u32 % width;
        let y = index as u32 / width;
        result.changed_pixels += 1;
        result.min_x = result.min_x.min(x);
        result.min_y = result.min_y.min(y);
        result.max_x = result.max_x.max(x);
        result.max_y = result.max_y.max(y);
    }
    Ok(result)
}

impl ApplicationHandler for EditorApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.fatal_error.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("SRD Editor")
            .with_inner_size(LogicalSize::new(1600.0, 1000.0))
            .with_min_inner_size(LogicalSize::new(960.0, 600.0))
            .with_visible(!self.smoke_test);
        let result = event_loop
            .create_window(attributes)
            .map_err(|error| error.to_string())
            .and_then(|window| {
                D3d9ExDevice::new(&window)
                    .map_err(|error| error.to_string())
                    .and_then(|d3d9| {
                        EditorWindow::new(
                            window,
                            d3d9,
                            self.smoke_test,
                            EditorSmokeOptions {
                                srd_draw: self.srd_draw_smoke,
                                srd_texture: self.srd_texture_smoke,
                                srd_fennel: self.srd_fennel_smoke,
                                srd_runtime: self.srd_runtime_smoke,
                                dds_device_audit: self.dds_device_audit,
                                chusan_player_host: self.chusan_player_host,
                            },
                            self.document_path.clone(),
                        )
                    })
            });
        match result {
            Ok(mut window) => {
                if self.smoke_test {
                    let first_frame = window.render_frame();
                    let current_size = window.window.inner_size();
                    window.d3d9.resize(current_size);
                    let reset_frame = window.render_frame();
                    match first_frame.and(reset_frame) {
                        Ok(D3d9ExFrameStatus::Presented) => {}
                        Ok(status) => {
                            self.fatal_error = Some(format!(
                                "D3D9Ex/ImGui smoke frame did not present: {status:?}"
                            ));
                        }
                        Err(error) => {
                            self.fatal_error =
                                Some(format!("D3D9Ex/ImGui smoke frame failed: {error}"));
                        }
                    }
                    self.window = Some(window);
                    event_loop.exit();
                } else {
                    window.window.request_redraw();
                    self.window = Some(window);
                }
            }
            Err(error) => {
                self.fatal_error = Some(error.clone());
                eprintln!("failed to initialize SRD editor: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        window_event: WindowEvent,
    ) {
        let Some(editor) = self.window.as_mut() else {
            return;
        };
        if editor.window.id() != window_id {
            return;
        }

        let event = Event::<()>::WindowEvent {
            window_id,
            event: window_event,
        };
        editor
            .platform
            .handle_event(editor.imgui.io_mut(), &editor.window, &event);
        let Event::WindowEvent { event, .. } = event else {
            unreachable!();
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                editor.d3d9.resize(size);
                editor.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                editor.handle_scale_factor_change();
                editor.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                editor.window.pre_present_notify();
                match editor.render_frame() {
                    Ok(D3d9ExFrameStatus::Presented) => {
                        if self.smoke_test {
                            event_loop.exit();
                        }
                    }
                    Ok(D3d9ExFrameStatus::DeviceLost) => editor.window.request_redraw(),
                    Ok(D3d9ExFrameStatus::Minimized) => {}
                    Err(error) => {
                        let message = format!("D3D9Ex/ImGui frame failed: {error}");
                        eprintln!("{message}");
                        self.fatal_error = Some(message);
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(editor) = &self.window {
            editor.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_diagnostic_ignores_alpha_in_the_clear_comparison() {
        let pixels = [
            0x26, 0x22, 0x20, 0x00, 0x26, 0x22, 0x20, 0xff, 0x27, 0x22, 0x20, 0xff, 0x26, 0x22,
            0x21, 0xff,
        ];
        let result = analyze_composition_readback(2, 2, &pixels, [0x26, 0x22, 0x20]).unwrap();
        assert_eq!(result.changed_pixels, 2);
        assert_eq!([result.min_x, result.min_y], [0, 1]);
        assert_eq!([result.max_x, result.max_y], [1, 1]);
    }

    #[test]
    fn runtime_rgb_difference_compares_only_color_channels() {
        let left = [1, 2, 3, 0, 4, 5, 6, 7];
        let right = [1, 2, 3, 255, 4, 5, 7, 7];
        assert_eq!(runtime_rgb_difference(&left, &right), 1);
    }

    #[test]
    fn chusan_player_host_argument_requires_explicit_target_present_and_screen_sizes() {
        let arguments = vec!["--advertise-logo-host=MainScene@1080x1920@1920x1080".to_string()];
        assert_eq!(
            parse_chusan_player_host_argument(&arguments).unwrap(),
            Some(ChusanPlayerHostArgument {
                common_background: false,
                target: PreviewTargetSelection::MainScene,
                present_width: 1080,
                present_height: 1920,
                screen_width: 1920,
                screen_height: 1080,
            })
        );
        let common = vec!["--common-background-host=MainScene@1080x1920@1920x1080".to_string()];
        assert_eq!(
            parse_chusan_player_host_argument(&common)
                .unwrap()
                .unwrap()
                .common_background,
            true
        );
        assert!(
            parse_chusan_player_host_argument(&["--advertise-logo-host=MainScene".to_string()])
                .is_err()
        );
        assert!(
            parse_chusan_player_host_argument(&[
                "--advertise-logo-host=Unknown@1080x1920@1920x1080".to_string()
            ])
            .is_err()
        );
        assert!(
            parse_chusan_player_host_argument(&[
                "--advertise-logo-host=BgScene@0x1920@1920x1080".to_string()
            ])
            .is_err()
        );
        assert!(
            parse_chusan_player_host_argument(&[
                "--advertise-logo-host=BgScene@1080x1920@0x1080".to_string()
            ])
            .is_err()
        );
    }

    #[test]
    fn runtime_smoke_argument_requires_exact_scene_animation_and_frame_tuple() {
        let arguments = vec!["--srd-runtime-smoke=2,3,-24".to_string()];
        assert_eq!(
            parse_runtime_smoke_argument(&arguments).unwrap(),
            Some(RuntimeSmokeSelection {
                scene_index: 2,
                animation_set_index: 3,
                frame: -24,
            })
        );
        assert!(parse_runtime_smoke_argument(&[]).unwrap().is_none());
        assert!(parse_runtime_smoke_argument(&["--srd-runtime-smoke=0,1".to_string()]).is_err());
        assert!(
            parse_runtime_smoke_argument(&["--srd-runtime-smoke=0,1,2,3".to_string()]).is_err()
        );
        assert!(parse_runtime_smoke_argument(&["--srd-runtime-smoke=x,1,2".to_string()]).is_err());
    }

    #[test]
    fn composition_size_rejects_unproven_fractional_conversion() {
        assert_eq!(
            checked_scene_composition_size(1080.0, 1920.0).unwrap(),
            [1080, 1920]
        );
        assert!(checked_scene_composition_size(1080.5, 1920.0).is_err());
        assert!(checked_scene_composition_size(f32::NAN, 1920.0).is_err());
        assert!(checked_scene_composition_size(0.0, 1920.0).is_err());
    }
}
