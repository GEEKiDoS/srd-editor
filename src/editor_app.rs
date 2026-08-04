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
    EditorWorkspace, PreviewHostSettings, PreviewScissorSelection, PreviewTargetSelection,
    apply_editor_style,
};
use crate::game_host::{CHUSAN_ADVERTISE_LOGO_PLAYER, CHUSAN_BG_SCENE, CHUSAN_MAIN_SCENE};
use crate::imgui_dx9::ImguiDx9Renderer;
use crate::ruhuna::{RuhunaFont, RuhunaRuntimeFont};
use crate::shader_bytecode::{
    FIRST_FIXTURE_SIMPLE_KEY, FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY, embedded_simple_shader_pair,
};
use crate::srd_draw::{
    EvidenceCompleteFennelDraw, EvidenceCompleteSrdDraw, SrdHostDrawContext,
    build_evidence_complete_animation_set_image_draws,
    build_evidence_complete_initial_fennel_draws, build_evidence_complete_initial_image_draws,
};
use crate::transform::Affine3x4;

const CLEAR_COLOR_ARGB: u32 = 0xff20_2226;

pub fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let advertise_logo_host = parse_advertise_logo_host_argument(&arguments)?;
    let srd_draw_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-draw-smoke");
    let srd_texture_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-texture-smoke");
    let srd_fennel_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-fennel-smoke");
    let dds_device_audit = arguments
        .iter()
        .any(|argument| argument == "--dds-device-audit");
    let smoke_test = srd_draw_smoke
        || srd_texture_smoke
        || srd_fennel_smoke
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
        dds_device_audit,
        advertise_logo_host,
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
    dds_device_audit: bool,
    advertise_logo_host: Option<AdvertiseLogoHostArgument>,
    document_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AdvertiseLogoHostArgument {
    target: PreviewTargetSelection,
    present_width: u32,
    present_height: u32,
    screen_width: u32,
    screen_height: u32,
}

#[derive(Debug, Clone, Copy)]
struct EditorSmokeOptions {
    srd_draw: bool,
    srd_texture: bool,
    srd_fennel: bool,
    dds_device_audit: bool,
    advertise_logo_host: Option<AdvertiseLogoHostArgument>,
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
    fennel_renderer: Option<FennelDx9Renderer>,
    fennel_atlases: BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
    fennel_draws: Vec<EvidenceCompleteFennelDraw>,
    composition_texture_id: Option<TextureId>,
    applied_preview_host: Option<PreviewHostSettings>,
    verify_srd_pixels: bool,
    verify_textured_pixels: bool,
    verify_fennel_pixels: bool,
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
        dds_device_audit: bool,
        advertise_logo_host: Option<AdvertiseLogoHostArgument>,
        document_path: Option<PathBuf>,
    ) -> Self {
        Self {
            window: None,
            fatal_error: None,
            smoke_test,
            srd_draw_smoke,
            srd_texture_smoke,
            srd_fennel_smoke,
            dds_device_audit,
            advertise_logo_host,
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
            dds_device_audit,
            advertise_logo_host,
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
        let require_srd_draw = srd_draw_smoke || srd_texture_smoke;
        let draw_result = if require_srd_draw {
            workspace.document().and_then(|document| {
                let scene = document.project.scenes.first()?;
                let host = if let Some(host) = advertise_logo_host {
                    let target = match host.target {
                        PreviewTargetSelection::MainScene => CHUSAN_MAIN_SCENE,
                        PreviewTargetSelection::BgScene => CHUSAN_BG_SCENE,
                        PreviewTargetSelection::Unselected => unreachable!(),
                    };
                    eprintln!(
                        "SRD smoke host=AdvertiseLogo/{} first_calc=identity present={}x{} screen={}x{}",
                        target.name,
                        host.present_width,
                        host.present_height,
                        host.screen_width,
                        host.screen_height
                    );
                    CHUSAN_ADVERTISE_LOGO_PLAYER
                        .host_context_for_target(
                            target,
                            host.present_width,
                            host.present_height,
                            [host.screen_width, host.screen_height],
                        )
                        .expect("validated AdvertiseLogo host dimensions")
                } else {
                    eprintln!(
                        "SRD smoke host=diagnostic-project-camera first_calc=identity target_width={}",
                        scene.width.max(1.0)
                    );
                    diagnostic_project_camera_smoke_host(
                        &document.project,
                        scene.width.max(1.0),
                        [scene.width.max(1.0) as u32, scene.height.max(1.0) as u32],
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
                };
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
            if advertise_logo_host.is_some() {
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
        let (fennel_renderer, fennel_atlases, fennel_draws) = if srd_fennel_smoke {
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
            let (runtime_fonts, atlases) =
                load_fennel_resources(d3d9.device(), &game_data_root, &document.project.fonts)?;
            let host =
                diagnostic_project_camera_smoke_host(&document.project, scene.width.max(1.0), size);
            let draws = build_evidence_complete_initial_fennel_draws(
                &document.project,
                0,
                host,
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
                draws,
            )
        } else {
            (None, BTreeMap::new(), Vec::new())
        };
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
            fennel_renderer,
            fennel_atlases,
            fennel_draws,
            composition_texture_id,
            applied_preview_host: None,
            verify_srd_pixels: srd_draw_smoke,
            verify_textured_pixels: srd_texture_smoke,
            verify_fennel_pixels: srd_fennel_smoke,
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
        if !self.verify_srd_pixels && !self.verify_textured_pixels && !self.verify_fennel_pixels {
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
            target
                .render_custom_to_composition(CLEAR_COLOR_ARGB, || {
                    render_fennel_draws(renderer, draws, atlases, composition_external)
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
        self.fennel_renderer = None;
        self.fennel_atlases.clear();
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
        let host = CHUSAN_ADVERTISE_LOGO_PLAYER
            .host_context_for_target(
                target,
                present_width,
                present_height,
                [screen_width, screen_height],
            )
            .map_err(|error| error.to_string())?;

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
        let draws = build_evidence_complete_animation_set_image_draws(
            &document.project,
            &document.textures,
            settings.scene_index,
            settings.animation_set_index,
            settings.animation_frame as f32,
            host,
        )
        .map_err(|error| error.to_string())?;
        if draws.is_empty() {
            return Err("No evidence-complete GPU draw for this scene".to_string());
        }
        let required_texture_indices = draws
            .iter()
            .flat_map(|draw| draw.texture_bindings.iter().flatten())
            .map(|binding| binding.texture_index)
            .collect::<Vec<_>>();
        let textures = if required_texture_indices.is_empty() {
            None
        } else {
            let game_data_root = find_game_data_root(&document.path).ok_or_else(|| {
                format!(
                    "Could not locate the game data root above {}",
                    document.path.display()
                )
            })?;
            Some(
                SrdD3d9TextureSet::load_required(
                    self.d3d9.device(),
                    &game_data_root,
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
        self.srd_draws = draws;
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
    SrdHostDrawContext::new(
        Affine3x4::IDENTITY,
        project
            .camera
            .runtime_matrices(target_width)
            .projection_view,
        [target_width as u32, target_screen_size[1]],
        target_screen_size,
    )
}

fn parse_advertise_logo_host_argument(
    arguments: &[String],
) -> Result<Option<AdvertiseLogoHostArgument>, String> {
    let Some(value) = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--advertise-logo-host="))
    else {
        return Ok(None);
    };
    let mut fields = value.split('@');
    let target = fields.next().unwrap_or_default();
    let present_size = fields.next().ok_or_else(|| {
        "--advertise-logo-host must use TARGET@PRESENT_WIDTHxPRESENT_HEIGHT@SCREEN_WIDTHxSCREEN_HEIGHT, for example MainScene@1080x1920@1920x1080".to_string()
    })?;
    let screen_size = fields.next().ok_or_else(|| {
        "--advertise-logo-host requires an explicit screenParam source size after the present size"
            .to_string()
    })?;
    if fields.next().is_some() {
        return Err("--advertise-logo-host contains too many @-separated fields".to_string());
    }
    let target = match target {
        "MainScene" => PreviewTargetSelection::MainScene,
        "BgScene" => PreviewTargetSelection::BgScene,
        _ => {
            return Err(format!(
                "unsupported AdvertiseLogo target {target:?}; expected MainScene or BgScene"
            ));
        }
    };
    let (width, height) = present_size.split_once('x').ok_or_else(|| {
        "--advertise-logo-host present size must use WIDTHxHEIGHT, for example 1080x1920"
            .to_string()
    })?;
    let present_width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid AdvertiseLogo present width {width:?}"))?;
    let present_height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid AdvertiseLogo present height {height:?}"))?;
    if present_width == 0 || present_height == 0 {
        return Err("AdvertiseLogo present dimensions must be non-zero".to_string());
    }
    let (width, height) = screen_size.split_once('x').ok_or_else(|| {
        "--advertise-logo-host screen size must use WIDTHxHEIGHT, for example 1920x1080".to_string()
    })?;
    let screen_width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid AdvertiseLogo screen width {width:?}"))?;
    let screen_height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid AdvertiseLogo screen height {height:?}"))?;
    if screen_width == 0 || screen_height == 0 {
        return Err("AdvertiseLogo screen dimensions must be non-zero".to_string());
    }
    Ok(Some(AdvertiseLogoHostArgument {
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
    fonts: &[crate::text::FontDefinition],
) -> Result<
    (
        BTreeMap<Vec<u8>, RuhunaRuntimeFont>,
        BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
    ),
    String,
> {
    let mut runtime_fonts = BTreeMap::new();
    let mut atlases = BTreeMap::new();
    for font in fonts {
        if runtime_fonts.contains_key(font.name.as_slice())
            || !font
                .name
                .iter()
                .map(u8::to_ascii_lowercase)
                .collect::<Vec<_>>()
                .ends_with(b".rfz")
        {
            continue;
        }
        let name = std::str::from_utf8(&font.name)
            .map_err(|error| format!("RFZ font name is not UTF-8: {error}"))?;
        let path = game_data_root.join("A000/font").join(name);
        let parsed = RuhunaFont::from_rfz(
            &fs::read(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let runtime = parsed
            .build_runtime_font(1, |page| u32::from(page) + 1)
            .map_err(|error| format!("failed to build runtime {}: {error}", path.display()))?;
        let atlas = RuhunaD3d9AtlasSet::from_font(device, &parsed)
            .map_err(|error| format!("failed to upload {}: {error}", path.display()))?;
        runtime_fonts.insert(font.name.clone(), runtime);
        atlases.insert(font.name.clone(), atlas);
    }
    Ok((runtime_fonts, atlases))
}

fn render_fennel_draws(
    renderer: &mut FennelDx9Renderer,
    draws: &[EvidenceCompleteFennelDraw],
    atlases: &BTreeMap<Vec<u8>, RuhunaD3d9AtlasSet>,
    external: SrdDx9ExternalContext,
) -> windows::core::Result<()> {
    for draw in draws {
        let atlas = atlases.get(draw.font_name.as_slice()).ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                format!(
                    "Fennel atlas {:?} is not loaded",
                    String::from_utf8_lossy(&draw.font_name)
                ),
            )
        })?;
        let batches = draw
            .batches
            .iter()
            .map(|batch| {
                let page_index = batch
                    .texture_token
                    .checked_sub(1)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| {
                        windows::core::Error::new(
                            windows::Win32::Foundation::E_INVALIDARG,
                            format!(
                                "Fennel smoke texture token {:#010x} is not the page+1 token assigned by its loader",
                                batch.texture_token
                            ),
                        )
                    })?;
                Ok(EvidenceCompleteFennelBatch {
                    page_index,
                    is_2d: draw.is_2d,
                    fixed_constants: draw.fixed_constants,
                    vertices: &batch.vertices,
                })
            })
            .collect::<windows::core::Result<Vec<_>>>()?;
        renderer.render(&batches, external, atlas)?;
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
                                dds_device_audit: self.dds_device_audit,
                                advertise_logo_host: self.advertise_logo_host,
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
    fn advertise_logo_host_argument_requires_explicit_target_present_and_screen_sizes() {
        let arguments = vec!["--advertise-logo-host=MainScene@1080x1920@1920x1080".to_string()];
        assert_eq!(
            parse_advertise_logo_host_argument(&arguments).unwrap(),
            Some(AdvertiseLogoHostArgument {
                target: PreviewTargetSelection::MainScene,
                present_width: 1080,
                present_height: 1920,
                screen_width: 1920,
                screen_height: 1080,
            })
        );
        assert!(
            parse_advertise_logo_host_argument(&["--advertise-logo-host=MainScene".to_string()])
                .is_err()
        );
        assert!(
            parse_advertise_logo_host_argument(&[
                "--advertise-logo-host=Unknown@1080x1920@1920x1080".to_string()
            ])
            .is_err()
        );
        assert!(
            parse_advertise_logo_host_argument(&[
                "--advertise-logo-host=BgScene@0x1920@1920x1080".to_string()
            ])
            .is_err()
        );
        assert!(
            parse_advertise_logo_host_argument(&[
                "--advertise-logo-host=BgScene@1080x1920@0x1080".to_string()
            ])
            .is_err()
        );
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
