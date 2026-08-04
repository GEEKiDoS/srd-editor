use std::error::Error;
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
use crate::d3d9_srd::{SrdDx9ExternalContext, SrdDx9Renderer};
use crate::editor_workspace::{EditorWorkspace, apply_editor_style};
use crate::imgui_dx9::ImguiDx9Renderer;
use crate::shader_bytecode::{FIRST_FIXTURE_SIMPLE_KEY, embedded_simple_shader_pair};
use crate::srd_draw::{EvidenceCompleteSrdDraw, build_evidence_complete_initial_image_draws};
use crate::transform::Affine3x4;

const CLEAR_COLOR_ARGB: u32 = 0xff20_2226;

pub fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let srd_draw_smoke = arguments
        .iter()
        .any(|argument| argument == "--srd-draw-smoke");
    let smoke_test = srd_draw_smoke
        || arguments
            .iter()
            .any(|argument| argument == "--d3d9ex-smoke" || argument == "--d3d9-smoke");
    let document_path = arguments
        .iter()
        .find(|argument| !argument.starts_with("--"))
        .map(PathBuf::from);
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut application = EditorApplication::new(smoke_test, srd_draw_smoke, document_path);
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
    document_path: Option<PathBuf>,
}

struct EditorWindow {
    window: Window,
    d3d9: D3d9ExDevice,
    imgui: Context,
    platform: WinitPlatform,
    imgui_renderer: ImguiDx9Renderer,
    srd_renderer: Option<SrdDx9Renderer>,
    srd_draws: Vec<EvidenceCompleteSrdDraw>,
    composition_texture_id: Option<TextureId>,
    verify_srd_pixels: bool,
    workspace: EditorWorkspace,
    dpi_factor: f64,
    last_frame: Instant,
}

impl EditorApplication {
    fn new(smoke_test: bool, srd_draw_smoke: bool, document_path: Option<PathBuf>) -> Self {
        Self {
            window: None,
            fatal_error: None,
            smoke_test,
            srd_draw_smoke,
            document_path,
        }
    }
}

impl EditorWindow {
    fn new(
        window: Window,
        d3d9: D3d9ExDevice,
        smoke_test: bool,
        srd_draw_smoke: bool,
        document_path: Option<PathBuf>,
    ) -> Result<Self, String> {
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
        let mut imgui_renderer =
            ImguiDx9Renderer::new(&mut imgui, d3d9.device()).map_err(|error| error.to_string())?;
        let mut workspace = EditorWorkspace::new(build_default_layout, document_path);
        let draw_result = workspace.document().and_then(|document| {
            let scene = document.project.scenes.first()?;
            Some((
                build_evidence_complete_initial_image_draws(
                    &document.project,
                    &document.textures,
                    0,
                    Affine3x4::IDENTITY,
                    scene.width.max(1.0),
                ),
                [scene.width.max(1.0) as u32, scene.height.max(1.0) as u32],
            ))
        });
        let (mut srd_renderer, srd_draws, composition_size) = match draw_result {
            Some((Ok(draws), size)) if !draws.is_empty() => {
                let renderer = SrdDx9Renderer::new(d3d9.device())
                    .map_err(|error| format!("failed to create the SRD D3D9 renderer: {error}"))?;
                (Some(renderer), draws, Some(size))
            }
            Some((Err(error), _)) if srd_draw_smoke => {
                return Err(format!(
                    "failed to build evidence-complete SRD draws: {error}"
                ));
            }
            _ if srd_draw_smoke => {
                return Err("the selected SRD did not produce an evidence-complete draw".into());
            }
            _ => (None, Vec::new(), None),
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
            srd_draws,
            composition_texture_id,
            verify_srd_pixels: srd_draw_smoke,
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
        let ui = self.imgui.frame();
        self.workspace.draw(ui);
        self.platform.prepare_render(ui, &self.window);
        let draw_data = self.imgui.render();

        self.d3d9
            .clear_and_begin_scene(CLEAR_COLOR_ARGB)
            .map_err(|error| error.to_string())?;
        if let Some(renderer) = &mut self.srd_renderer {
            renderer
                .render_to_composition(
                    &self.srd_draws,
                    SrdDx9ExternalContext::smoke_without_scissor(),
                    CLEAR_COLOR_ARGB,
                )
                .map_err(|error| format!("SRD composition draw failed: {error}"))?;
        }
        if self.verify_srd_pixels
            && let Some(renderer) = &mut self.srd_renderer
        {
            renderer
                .render(
                    &self.srd_draws,
                    SrdDx9ExternalContext::smoke_without_scissor(),
                )
                .map_err(|error| format!("SRD D3D9 draw submission failed: {error}"))?;
        }
        if self.verify_srd_pixels {
            self.d3d9.end_scene().map_err(|error| error.to_string())?;
            let renderer = self
                .srd_renderer
                .as_ref()
                .ok_or_else(|| "SRD draw smoke lost the composition renderer".to_string())?;
            let composition_size = renderer
                .composition_size()
                .ok_or_else(|| "SRD draw smoke lost the composition target".to_string())?;
            let composition_pixel = renderer
                .read_composition_pixel(composition_size[0] / 2 + 1, composition_size[1] / 4)
                .map_err(|error| format!("SRD composition readback failed: {error}"))?;
            if composition_pixel[..3] != [0, 0, 0] {
                return Err(format!(
                    "SRD composition expected a black identity-host sample pixel, got raw BGRA/XRGB {composition_pixel:02X?}"
                ));
            }
            let size = self.window.inner_size();
            let pixel = self
                .d3d9
                .read_backbuffer_pixel(size.width / 2 + 1, size.height / 4)
                .map_err(|error| format!("SRD backbuffer readback failed: {error}"))?;
            if pixel[..3] != [0, 0, 0] {
                return Err(format!(
                    "SRD draw smoke expected a black identity-host sample pixel, got raw BGRA/XRGB {pixel:02X?}"
                ));
            }
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
}

fn editor_ini_path() -> Option<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("APPDATA")?).join("SrdEditor");
    std::fs::create_dir_all(&directory).ok()?;
    Some(directory.join("imgui.ini"))
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
                            self.srd_draw_smoke,
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
