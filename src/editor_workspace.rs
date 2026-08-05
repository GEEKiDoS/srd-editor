use std::collections::BTreeMap;
use std::ffi::CStr;
use std::path::PathBuf;
use std::ptr;

use imgui::{
    Image, StyleColor, TableColumnFlags, TableColumnSetup, TableFlags, TextureId, Ui, sys,
};

use crate::animation::{KeyData, Track};
use crate::editor_document::{EditorDocument, display_srd_name};
use crate::scene::Layer;
use crate::srd_draw::FennelSrdRuntimeTextInput;

const PROJECT_WINDOW: &CStr = c"Project";
const SCENE_WINDOW: &CStr = c"Scene & Status";
const COMPOSITION_WINDOW: &CStr = c"Composition";
const PROPERTIES_WINDOW: &CStr = c"Properties";
const TIMELINE_WINDOW: &CStr = c"Layers & Timeline";

// Current Chusan cabinet profile: the target camera renders in portrait while
// ShapeEnv2D receives the unrotated 1920x1080 screen source dimensions.
const DEFAULT_PRESENT_WIDTH: i32 = 1080;
const DEFAULT_PRESENT_HEIGHT: i32 = 1920;
const DEFAULT_TARGET_SCREEN_WIDTH: i32 = 1920;
const DEFAULT_TARGET_SCREEN_HEIGHT: i32 = 1080;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewTargetSelection {
    Unselected,
    MainScene,
    BgScene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewPlayerSelection {
    Unselected,
    AdvertiseLogo,
    CommonBackground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewScissorSelection {
    Unselected,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewHostSettings {
    pub scene_index: usize,
    pub animation_set_index: usize,
    pub animation_frame: i32,
    pub player: PreviewPlayerSelection,
    pub target: PreviewTargetSelection,
    pub present_width: i32,
    pub present_height: i32,
    pub screen_width: i32,
    pub screen_height: i32,
    pub scissor: PreviewScissorSelection,
}

pub struct EditorWorkspace {
    build_default_layout: bool,
    frame: i32,
    playing: bool,
    document: Option<EditorDocument>,
    load_error: Option<String>,
    selected_scene: usize,
    selected_animation_set: usize,
    selected_layer: usize,
    selected_node: Option<usize>,
    composition_texture: Option<(TextureId, [u32; 2])>,
    composition_unavailable_reason: Option<String>,
    preview_player: PreviewPlayerSelection,
    preview_target: PreviewTargetSelection,
    preview_present_width: i32,
    preview_present_height: i32,
    preview_screen_width: i32,
    preview_screen_height: i32,
    preview_scissor: PreviewScissorSelection,
    fennel_runtime_text_inputs: BTreeMap<(usize, usize, usize), FennelSrdRuntimeTextInput>,
}

impl EditorWorkspace {
    pub fn new(build_default_layout: bool, document_path: Option<PathBuf>) -> Self {
        let (document, load_error) = match document_path {
            Some(path) => match EditorDocument::load(path) {
                Ok(document) => (Some(document), None),
                Err(error) => (None, Some(error)),
            },
            None => (None, None),
        };
        Self {
            build_default_layout,
            frame: 0,
            playing: false,
            document,
            load_error,
            selected_scene: 0,
            selected_animation_set: 0,
            selected_layer: 0,
            selected_node: None,
            composition_texture: None,
            composition_unavailable_reason: None,
            preview_player: PreviewPlayerSelection::Unselected,
            preview_target: PreviewTargetSelection::Unselected,
            preview_present_width: DEFAULT_PRESENT_WIDTH,
            preview_present_height: DEFAULT_PRESENT_HEIGHT,
            preview_screen_width: DEFAULT_TARGET_SCREEN_WIDTH,
            preview_screen_height: DEFAULT_TARGET_SCREEN_HEIGHT,
            preview_scissor: PreviewScissorSelection::Unselected,
            fennel_runtime_text_inputs: BTreeMap::new(),
        }
    }

    pub fn draw(&mut self, ui: &Ui) {
        self.draw_menu(ui);
        let dock_id = unsafe {
            sys::igDockSpaceOverViewport(
                sys::igGetMainViewport(),
                sys::ImGuiDockNodeFlags_PassthruCentralNode as i32,
                ptr::null(),
            )
        };
        if self.build_default_layout {
            unsafe { build_after_effects_layout(dock_id) };
            self.build_default_layout = false;
        }

        ui.window(PROJECT_WINDOW.to_str().unwrap())
            .build(|| self.draw_project(ui));
        ui.window(SCENE_WINDOW.to_str().unwrap())
            .build(|| self.draw_scene_status(ui));
        ui.window(COMPOSITION_WINDOW.to_str().unwrap())
            .build(|| self.draw_composition(ui));
        ui.window(PROPERTIES_WINDOW.to_str().unwrap())
            .build(|| self.draw_properties(ui));
        ui.window(TIMELINE_WINDOW.to_str().unwrap())
            .build(|| self.draw_timeline(ui));
    }

    pub fn document(&self) -> Option<&EditorDocument> {
        self.document.as_ref()
    }

    pub fn set_composition_texture(&mut self, texture: Option<(TextureId, [u32; 2])>) {
        if texture.is_some() {
            self.composition_unavailable_reason = None;
        }
        self.composition_texture = texture;
    }

    pub fn set_composition_unavailable_reason(&mut self, reason: Option<String>) {
        self.composition_unavailable_reason = reason;
    }

    pub fn preview_host_settings(&self) -> PreviewHostSettings {
        PreviewHostSettings {
            scene_index: self.selected_scene,
            animation_set_index: self.selected_animation_set,
            animation_frame: self.frame,
            player: self.preview_player,
            target: self.preview_target,
            present_width: self.preview_present_width,
            present_height: self.preview_present_height,
            screen_width: self.preview_screen_width,
            screen_height: self.preview_screen_height,
            scissor: self.preview_scissor,
        }
    }

    pub fn selected_scene_fennel_runtime_text_inputs(
        &self,
    ) -> BTreeMap<(usize, usize), FennelSrdRuntimeTextInput> {
        self.fennel_runtime_text_inputs
            .iter()
            .filter_map(|(&(scene_index, layer_index, node_index), input)| {
                (scene_index == self.selected_scene)
                    .then_some(((layer_index, node_index), input.clone()))
            })
            .collect()
    }

    pub fn validate_preview_host_settings(&self) -> Result<PreviewHostSettings, String> {
        if self.document.is_none() {
            return Err("No SRD loaded".to_string());
        }
        let settings = self.preview_host_settings();
        if settings.player == PreviewPlayerSelection::Unselected {
            return Err("Select a Chusan SrPlayer host profile".to_string());
        }
        if settings.target == PreviewTargetSelection::Unselected {
            return Err("Select a Chusan target profile".to_string());
        }
        if settings.present_width <= 0 || settings.present_height <= 0 {
            return Err("Enter the target present width and height".to_string());
        }
        if settings.screen_width <= 0 || settings.screen_height <= 0 {
            return Err("Enter the target screenParam source width and height".to_string());
        }
        if settings.scissor == PreviewScissorSelection::Unselected {
            return Err("Select the external material scissor state".to_string());
        }
        let scene = self
            .document
            .as_ref()
            .and_then(|document| document.project.scenes.get(settings.scene_index))
            .ok_or_else(|| "Select a valid SRD scene".to_string())?;
        if scene
            .animation_sets
            .get(settings.animation_set_index)
            .is_none()
        {
            return Err("Select a scene animation set".to_string());
        }
        Ok(settings)
    }

    fn draw_menu(&mut self, ui: &Ui) {
        ui.main_menu_bar(|| {
            ui.menu("File", || {
                let _ = ui.menu_item("Open SRD...");
                let _ = ui.menu_item("Save");
                ui.separator();
                let _ = ui.menu_item("Exit");
            });
            ui.menu("Edit", || {
                let _ = ui.menu_item("Undo");
                let _ = ui.menu_item("Redo");
            });
            ui.menu("View", || {
                let _ = ui.menu_item("Reset workspace");
            });
            ui.menu("Playback", || {
                if ui.menu_item(if self.playing { "Pause" } else { "Play" }) {
                    self.playing = !self.playing;
                }
                if ui.menu_item("Go to start") {
                    self.frame = self
                        .selected_animation_set()
                        .map_or(0, |set| set.start_frame);
                }
            });
        });
    }

    fn draw_project(&self, ui: &Ui) {
        ui.text_disabled("PROJECT RESOURCES");
        ui.separator();
        if let Some(error) = &self.load_error {
            ui.text_colored([1.0, 0.35, 0.30, 1.0], "Load failed");
            ui.text_wrapped(error);
        } else if let Some(document) = &self.document {
            ui.text(document.path.file_name().map_or_else(
                || document.path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ));
            ui.text_disabled(document.path.display().to_string());
            ui.separator();
            ui.bullet_text(format!("VTBF blocks: {}", document.file.blocks.len()));
            ui.bullet_text(format!("Scenes: {}", document.project.scenes.len()));
            ui.bullet_text(format!("Textures: {}", document.textures.textures.len()));
            for (index, texture) in document.textures.textures.iter().enumerate() {
                ui.bullet_text(format!(
                    "[{index}] {}  {}×{}",
                    display_srd_name(&texture.filename),
                    texture.width,
                    texture.height
                ));
            }
        } else {
            ui.text("No SRD loaded");
            ui.spacing();
            ui.text_disabled("Pass an SRD path on the command line or use File > Open SRD.");
        }
    }

    fn draw_scene_status(&mut self, ui: &Ui) {
        ui.text_disabled("SCENE / LAYER / CAST");
        ui.separator();
        let Some(document) = &self.document else {
            ui.text("No scene loaded");
            return;
        };
        for (scene_index, scene) in document.project.scenes.iter().enumerate() {
            let scene_name = display_srd_name(&scene.name);
            if ui
                .selectable_config(format!("Scene {scene_index}: {scene_name}"))
                .selected(self.selected_scene == scene_index)
                .build()
            {
                self.selected_scene = scene_index;
                self.selected_animation_set = 0;
                self.selected_layer = 0;
                self.selected_node = None;
                self.frame = scene
                    .animation_sets
                    .first()
                    .map_or(0, |set| set.start_frame);
            }
            if self.selected_scene == scene_index {
                for (layer_index, layer) in scene.layers.iter().enumerate() {
                    let layer_name = display_srd_name(&layer.name);
                    let animation_slot = scene
                        .animation_sets
                        .get(self.selected_animation_set)
                        .and_then(|set| set.slots.get(layer_index));
                    let state = animation_slot
                        .map_or("base", |slot| if slot.is_enabled() { "on" } else { "off" });
                    let animation_name = animation_slot
                        .filter(|slot| !slot.animation_name.is_empty())
                        .map(|slot| display_srd_name(&slot.animation_name))
                        .unwrap_or_default();
                    if ui
                        .selectable_config(format!(
                            "    [{state}] Layer {layer_index}: {layer_name} {animation_name}"
                        ))
                        .selected(self.selected_layer == layer_index)
                        .build()
                    {
                        self.selected_layer = layer_index;
                        self.selected_node = None;
                        self.frame = scene
                            .animation_sets
                            .get(self.selected_animation_set)
                            .map_or(0, |set| set.start_frame);
                    }
                }
            }
        }
    }

    fn draw_composition(&self, ui: &Ui) {
        let available = ui.content_region_avail();
        let origin = ui.cursor_screen_pos();
        let size = [available[0].max(1.0), available[1].max(1.0)];
        let draw_list = ui.get_window_draw_list();
        draw_list
            .add_rect(
                origin,
                [origin[0] + size[0], origin[1] + size[1]],
                [0.10, 0.11, 0.13, 1.0],
            )
            .filled(true)
            .build();
        if let Some((texture, [texture_width, texture_height])) = self.composition_texture {
            let scale = (size[0] / texture_width as f32)
                .min(size[1] / texture_height as f32)
                .max(0.0);
            let image_size = [texture_width as f32 * scale, texture_height as f32 * scale];
            let image_origin = [
                origin[0] + (size[0] - image_size[0]) * 0.5,
                origin[1] + (size[1] - image_size[1]) * 0.5,
            ];
            ui.set_cursor_screen_pos(image_origin);
            Image::new(texture, image_size).build(ui);
            ui.set_cursor_screen_pos(origin);
            ui.invisible_button("composition-canvas", size);
        } else {
            let center = [origin[0] + size[0] * 0.5, origin[1] + size[1] * 0.5];
            let message = if self.document.is_some() {
                self.composition_unavailable_reason
                    .as_deref()
                    .unwrap_or("No evidence-complete GPU draw for this scene")
            } else {
                "No SRD loaded"
            };
            let text_size = ui.calc_text_size(message);
            draw_list.add_text(
                [
                    center[0] - text_size[0] * 0.5,
                    center[1] - text_size[1] * 0.5,
                ],
                [0.58, 0.60, 0.64, 1.0],
                message,
            );
            ui.invisible_button("composition-canvas", size);
        }
    }

    fn draw_properties(&mut self, ui: &Ui) {
        ui.text_disabled("PROPERTIES");
        ui.separator();
        ui.text_disabled("GAME HOST PREVIEW");
        if let Some(animation_sets) = self.selected_scene().map(|scene| {
            scene
                .animation_sets
                .iter()
                .enumerate()
                .map(|(index, set)| {
                    (
                        format!("{index}: {}", display_srd_name(&set.name)),
                        set.start_frame,
                        set.runtime_duration,
                    )
                })
                .collect::<Vec<_>>()
        }) {
            let animation_set_names = animation_sets
                .iter()
                .map(|(name, _, _)| name.clone())
                .collect::<Vec<_>>();
            let animation_set_labels = animation_set_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            if !animation_set_labels.is_empty() {
                let mut animation_set_index = self
                    .selected_animation_set
                    .min(animation_set_labels.len() - 1);
                self.selected_animation_set = animation_set_index;
                if ui.combo_simple_string(
                    "Animation set",
                    &mut animation_set_index,
                    &animation_set_labels,
                ) {
                    self.selected_animation_set = animation_set_index;
                    self.frame = animation_sets[animation_set_index].1;
                }
                let start_frame = animation_sets[self.selected_animation_set].1;
                let runtime_duration = animation_sets[self.selected_animation_set].2;
                ui.text(format!(
                    "ANMS start / duration: {} / {}",
                    start_frame, runtime_duration
                ));
            } else {
                ui.text_colored([0.92, 0.68, 0.25, 1.0], "Scene has no ANMS entries");
            }
        }
        let mut player_index = match self.preview_player {
            PreviewPlayerSelection::Unselected => 0,
            PreviewPlayerSelection::AdvertiseLogo => 1,
            PreviewPlayerSelection::CommonBackground => 2,
        };
        if ui.combo_simple_string(
            "SrPlayer host",
            &mut player_index,
            &[
                "Not selected",
                "AdvertiseLogoObject",
                "CommonBackGroundObject",
            ],
        ) {
            self.preview_player = match player_index {
                1 => PreviewPlayerSelection::AdvertiseLogo,
                2 => PreviewPlayerSelection::CommonBackground,
                _ => PreviewPlayerSelection::Unselected,
            };
        }
        let mut target_index = match self.preview_target {
            PreviewTargetSelection::Unselected => 0,
            PreviewTargetSelection::MainScene => 1,
            PreviewTargetSelection::BgScene => 2,
        };
        if ui.combo_simple_string(
            "Target",
            &mut target_index,
            &["Not selected", "MainScene", "BgScene"],
        ) {
            self.preview_target = match target_index {
                1 => PreviewTargetSelection::MainScene,
                2 => PreviewTargetSelection::BgScene,
                _ => PreviewTargetSelection::Unselected,
            };
        }
        ui.input_int("Present width", &mut self.preview_present_width)
            .build();
        ui.input_int("Present height", &mut self.preview_present_height)
            .build();
        ui.input_int("Target screen width", &mut self.preview_screen_width)
            .build();
        ui.input_int("Target screen height", &mut self.preview_screen_height)
            .build();
        ui.text_wrapped(
            "Target screen size drives ShapeEnv2D screenParam and is intentionally independent from the Camera present size.",
        );
        let mut scissor_index = match self.preview_scissor {
            PreviewScissorSelection::Unselected => 0,
            PreviewScissorSelection::Disabled => 1,
        };
        if ui.combo_simple_string(
            "External scissor",
            &mut scissor_index,
            &["Not selected", "Disabled"],
        ) {
            self.preview_scissor = match scissor_index {
                1 => PreviewScissorSelection::Disabled,
                _ => PreviewScissorSelection::Unselected,
            };
        }
        match self.preview_player {
            PreviewPlayerSelection::AdvertiseLogo => ui.text_wrapped(
                "AdvertiseLogo: FirstCalc identity, 2DLayer 100, root key 0xE480 (game binary evidence)",
            ),
            PreviewPlayerSelection::CommonBackground => {
                ui.text_wrapped(
                    "CommonBackGround: FirstCalc identity, 2DLayer 6, root key 0x8680 (game binary evidence)",
                );
                ui.text_wrapped(
                    "This preview currently uses the explicit null-lookup diagnostic branch. Empty TargetScene may still resolve an AFB-registered empty-key target; the selected receiving scene supplies the final Camera only for this diagnostic.",
                );
            }
            PreviewPlayerSelection::Unselected => {
                ui.text_wrapped("Choose a binary-proven SrPlayer host profile.");
            }
        }
        match self.validate_preview_host_settings() {
            Ok(_) => ui.text_colored([0.35, 0.82, 0.48, 1.0], "Host inputs complete"),
            Err(reason) => ui.text_colored([0.92, 0.68, 0.25, 1.0], reason),
        }
        ui.separator();
        if let Some(document) = &self.document {
            let camera = document.project.camera;
            ui.text_disabled("PROJECT CAMERA");
            ui.text(format!("Position: {:?}", camera.position));
            ui.text(format!("Target: {:?}", camera.target));
            ui.text(format!(
                "FovY: {} units / {:.6}°",
                camera.angle_units,
                camera.angle_degrees()
            ));
            ui.text(format!("Near / Far: {} / {}", camera.near, camera.far));
            if let Some(scene) = self.selected_scene() {
                ui.text(format!("Composition: {} × {}", scene.width, scene.height));
            }
            ui.separator();
        }
        let Some((layer, node_index)) = self.selected_layer_and_node() else {
            ui.text_disabled("Select a node for CAST properties");
            return;
        };
        let node = &layer.nodes[node_index];
        ui.text(format!(
            "Node {node_index}: {}",
            node_display_name(node_index, node.name.as_deref())
        ));
        ui.separator();
        ui.text(format!("Cast type: {}", cast_type_name(node.cast_type())));
        ui.text(format!(
            "Type flags: {:#010x}",
            node.type_flags.unwrap_or(0)
        ));
        ui.text(format!("First child: {}", node.first_child_index));
        ui.text(format!("Next sibling: {}", node.next_sibling_index));
        if let Some(transform) = layer.transforms.get(node_index) {
            let spatial = transform.spatial();
            ui.separator();
            ui.text_disabled("TRANSFORM");
            ui.text(format!("Translation: {:?}", spatial.translation));
            ui.text(format!("Rotation: {:?}", spatial.rotation));
            ui.text(format!("Scale: {:?}", spatial.scale));
        }
        let is_text_cast = layer
            .image_by_node
            .get(node_index)
            .and_then(Option::as_ref)
            .is_some_and(|image| image.creates_text_cast());
        if is_text_cast {
            ui.separator();
            ui.text_disabled("FENNEL RUNTIME TEXT HOST");
            let key = (self.selected_scene, self.selected_layer, node_index);
            let mut enabled = self.fennel_runtime_text_inputs.contains_key(&key);
            if ui.checkbox("Manual runtime text input", &mut enabled) {
                if enabled {
                    self.fennel_runtime_text_inputs.entry(key).or_default();
                } else {
                    self.fennel_runtime_text_inputs.remove(&key);
                }
            }
            if let Some(input) = self.fennel_runtime_text_inputs.get_mut(&key) {
                ui.input_float("Text state F4", &mut input.field_f4).build();
                ui.input_int("Default D", &mut input.default_d).build();
                ui.input_int("Repeat spaces", &mut input.repeat_space_count)
                    .build();
                ui.text_wrapped(
                    "F4 is the exact SrTextCast runtime field and is intentionally independent from the animation timeline frame.",
                );
                for substitution_index in 0..8 {
                    let mut value =
                        String::from_utf8_lossy(&input.substitutions[substitution_index])
                            .into_owned();
                    if ui
                        .input_text(format!("$[{substitution_index}]"), &mut value)
                        .build()
                    {
                        input.substitutions[substitution_index] = value.into_bytes();
                    }
                }
                ui.text_colored(
                    [0.92, 0.68, 0.25, 1.0],
                    "Stored only: mixed ImageCast/TextCast Composition ordering is not yet evidence-complete.",
                );
            } else {
                ui.text_disabled(
                    "Enable this only when the host substitution strings or current scroll F4 are known.",
                );
            }
        }
    }

    fn draw_timeline(&mut self, ui: &Ui) {
        if ui.small_button(if self.playing { "Pause" } else { "Play" }) {
            self.playing = !self.playing;
        }
        ui.same_line();
        if ui.small_button("Start") {
            self.frame = self
                .selected_animation_set()
                .map_or(0, |set| set.start_frame);
        }
        ui.same_line();
        let (start_frame, runtime_duration) = self
            .selected_animation_set()
            .map(|set| {
                (
                    set.start_frame,
                    set.runtime_duration.max(set.start_frame + 1),
                )
            })
            .unwrap_or_else(|| {
                let duration = self.selected_layer().map_or(300, layer_duration).max(1);
                (0, duration)
            });
        ui.set_next_item_width(130.0);
        ui.slider_config("Frame", start_frame, runtime_duration)
            .build(&mut self.frame);
        let duration = runtime_duration.max(1);
        ui.separator();

        let columns = [
            TableColumnSetup {
                name: "Layer / State",
                flags: TableColumnFlags::WIDTH_FIXED | TableColumnFlags::NO_HIDE,
                init_width_or_weight: 320.0,
                user_id: Default::default(),
            },
            TableColumnSetup {
                name: "Timeline",
                flags: TableColumnFlags::WIDTH_STRETCH | TableColumnFlags::NO_HIDE,
                init_width_or_weight: 1.0,
                user_id: Default::default(),
            },
        ];
        let flags = TableFlags::BORDERS_INNER
            | TableFlags::BORDERS_OUTER
            | TableFlags::ROW_BG
            | TableFlags::RESIZABLE
            | TableFlags::SCROLL_X
            | TableFlags::SCROLL_Y
            | TableFlags::SIZING_STRETCH_PROP;
        if let Some(_table) =
            ui.begin_table_with_sizing("layer-timeline-table", 2, flags, [0.0, -1.0], 960.0)
        {
            for column in columns {
                ui.table_setup_column_with(column);
            }
            ui.table_setup_scroll_freeze(1, 1);
            ui.table_headers_row();

            let selected_animation_name = self
                .selected_animation_set()
                .and_then(|set| set.slots.get(self.selected_layer))
                .filter(|slot| !slot.animation_name.is_empty())
                .map(|slot| slot.animation_name.clone());
            let Some(layer) = self.selected_layer() else {
                ui.table_next_row();
                ui.table_next_column();
                ui.text_disabled("No layer selected");
                ui.table_next_column();
                ui.text_disabled("No timeline");
                return;
            };
            let mut clicked_node = None;
            for (node_index, node) in layer.nodes.iter().enumerate() {
                ui.table_next_row();
                ui.table_next_column();
                if ui
                    .selectable_config(format!(
                        "{}  [{}]##node-{node_index}",
                        node_display_name(node_index, node.name.as_deref()),
                        cast_type_name(node.cast_type())
                    ))
                    .selected(self.selected_node == Some(node_index))
                    .build()
                {
                    clicked_node = Some(node_index);
                }
                ui.table_next_column();
                draw_node_timeline(
                    ui,
                    layer,
                    node_index,
                    duration,
                    self.frame,
                    selected_animation_name.as_deref(),
                );
            }
            if let Some(node_index) = clicked_node {
                self.selected_node = Some(node_index);
            }
        }
    }

    fn selected_layer(&self) -> Option<&Layer> {
        self.selected_scene()?.layers.get(self.selected_layer)
    }

    fn selected_scene(&self) -> Option<&crate::scene::Scene> {
        self.document
            .as_ref()?
            .project
            .scenes
            .get(self.selected_scene)
    }

    fn selected_animation_set(&self) -> Option<&crate::scene::AnimationSetDefinition> {
        self.selected_scene()?
            .animation_sets
            .get(self.selected_animation_set)
    }

    fn selected_layer_and_node(&self) -> Option<(&Layer, usize)> {
        let node_index = self.selected_node?;
        let layer = self.selected_layer()?;
        (node_index < layer.nodes.len()).then_some((layer, node_index))
    }
}

fn layer_duration(layer: &Layer) -> i32 {
    layer
        .animations
        .first()
        .map_or(300, |animation| animation.duration.max(1))
}

fn node_display_name(index: usize, name: Option<&[u8]>) -> String {
    name.map(display_srd_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Node {index}"))
}

fn cast_type_name(cast_type: Option<u8>) -> &'static str {
    match cast_type {
        Some(1) => "Image",
        Some(2) => "Slice",
        Some(3) => "Reference",
        Some(4) => "Number",
        Some(_) => "Unknown",
        None => "Group",
    }
}

fn draw_node_timeline(
    ui: &Ui,
    layer: &Layer,
    node_index: usize,
    duration: i32,
    frame: i32,
    animation_name: Option<&[u8]>,
) {
    let origin = ui.cursor_screen_pos();
    let size = [
        ui.content_region_avail()[0].max(360.0),
        ui.text_line_height_with_spacing(),
    ];
    let draw_list = ui.get_window_draw_list();
    let center_y = origin[1] + size[1] * 0.5;
    draw_list
        .add_line(
            [origin[0], center_y],
            [origin[0] + size[0], center_y],
            [0.28, 0.30, 0.34, 1.0],
        )
        .build();
    if let Some(animation) = animation_name
        .and_then(|name| layer.find_animation(name))
        .map(|(_, animation)| animation)
    {
        for motion in animation
            .motions
            .iter()
            .filter(|motion| motion.target == node_index as i32)
        {
            for key_frame in motion.tracks.iter().flat_map(track_key_frames) {
                let x = origin[0] + size[0] * key_frame.clamp(0, duration) as f32 / duration as f32;
                draw_list
                    .add_rect(
                        [x - 3.0, center_y - 3.0],
                        [x + 3.0, center_y + 3.0],
                        [0.96, 0.72, 0.20, 1.0],
                    )
                    .filled(true)
                    .build();
            }
        }
    }
    let playhead_x = origin[0] + size[0] * frame.clamp(0, duration) as f32 / duration as f32;
    draw_list
        .add_line(
            [playhead_x, origin[1]],
            [playhead_x, origin[1] + size[1]],
            [0.95, 0.25, 0.22, 1.0],
        )
        .build();
    ui.invisible_button(format!("timeline-row-{node_index}"), size);
}

fn track_key_frames(track: &Track) -> Vec<i32> {
    match &track.keys {
        KeyData::Key8F32(keys) => keys.iter().map(|key| key.frame).collect(),
        KeyData::Key8I32(keys) => keys.iter().map(|key| key.frame).collect(),
        KeyData::Key8Bytes4(keys) => keys.iter().map(|key| key.frame).collect(),
        KeyData::Key20F32(keys) => keys.iter().map(|key| key.frame).collect(),
        KeyData::Key20I32(keys) => keys.iter().map(|key| key.frame).collect(),
        KeyData::Unsupported => Vec::new(),
    }
}

pub fn apply_editor_style(context: &mut imgui::Context) {
    let style = context.style_mut();
    style.window_rounding = 2.0;
    style.child_rounding = 2.0;
    style.frame_rounding = 2.0;
    style.grab_rounding = 2.0;
    style.tab_rounding = 2.0;
    style.window_border_size = 1.0;
    style.frame_border_size = 0.0;
    style.colors[StyleColor::WindowBg as usize] = [0.105, 0.112, 0.125, 1.0];
    style.colors[StyleColor::TitleBg as usize] = [0.075, 0.080, 0.092, 1.0];
    style.colors[StyleColor::TitleBgActive as usize] = [0.12, 0.13, 0.15, 1.0];
    style.colors[StyleColor::Tab as usize] = [0.095, 0.102, 0.115, 1.0];
    style.colors[StyleColor::TabActive as usize] = [0.18, 0.21, 0.25, 1.0];
    style.colors[StyleColor::Header as usize] = [0.16, 0.19, 0.23, 1.0];
    style.colors[StyleColor::HeaderHovered as usize] = [0.22, 0.28, 0.34, 1.0];
    style.colors[StyleColor::HeaderActive as usize] = [0.25, 0.32, 0.39, 1.0];
}

unsafe fn build_after_effects_layout(dock_id: sys::ImGuiID) {
    unsafe {
        let viewport = sys::igGetMainViewport();
        if viewport.is_null() {
            return;
        }
        sys::igDockBuilderRemoveNode(dock_id);
        sys::igDockBuilderAddNode(
            dock_id,
            sys::ImGuiDockNodeFlags_DockSpace | sys::ImGuiDockNodeFlags_PassthruCentralNode as i32,
        );
        sys::igDockBuilderSetNodePos(dock_id, (*viewport).Pos);
        sys::igDockBuilderSetNodeSize(dock_id, (*viewport).Size);

        let mut center = dock_id;
        let mut left = 0;
        let mut right = 0;
        let mut bottom = 0;
        sys::igDockBuilderSplitNode(center, sys::ImGuiDir_Left, 0.20, &mut left, &mut center);
        sys::igDockBuilderSplitNode(center, sys::ImGuiDir_Right, 0.24, &mut right, &mut center);
        sys::igDockBuilderSplitNode(center, sys::ImGuiDir_Down, 0.32, &mut bottom, &mut center);

        sys::igDockBuilderDockWindow(PROJECT_WINDOW.as_ptr(), left);
        sys::igDockBuilderDockWindow(SCENE_WINDOW.as_ptr(), left);
        sys::igDockBuilderDockWindow(PROPERTIES_WINDOW.as_ptr(), right);
        sys::igDockBuilderDockWindow(TIMELINE_WINDOW.as_ptr(), bottom);
        sys::igDockBuilderDockWindow(COMPOSITION_WINDOW.as_ptr(), center);
        sys::igDockBuilderFinish(dock_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_starts_with_current_chusan_host_dimensions() {
        let workspace = EditorWorkspace::new(false, None);
        let settings = workspace.preview_host_settings();
        assert_eq!(settings.present_width, 1080);
        assert_eq!(settings.present_height, 1920);
        assert_eq!(settings.screen_width, 1920);
        assert_eq!(settings.screen_height, 1080);
        assert_eq!(settings.player, PreviewPlayerSelection::Unselected);
    }

    #[test]
    fn workspace_exports_runtime_text_inputs_only_for_the_selected_scene() {
        let mut workspace = EditorWorkspace::new(false, None);
        workspace.fennel_runtime_text_inputs.insert(
            (0, 2, 3),
            FennelSrdRuntimeTextInput {
                field_f4: 9.5,
                ..Default::default()
            },
        );
        workspace.fennel_runtime_text_inputs.insert(
            (1, 4, 5),
            FennelSrdRuntimeTextInput {
                field_f4: 12.0,
                ..Default::default()
            },
        );
        assert_eq!(
            workspace
                .selected_scene_fennel_runtime_text_inputs()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![(2, 3)]
        );
    }
}
