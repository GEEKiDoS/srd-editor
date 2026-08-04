use std::fmt;

use crate::camera::{build_look_at_rh_game, build_perspective_fov_rh_game};
use crate::projection::{Matrix4x4, mul_matrix4x4_game};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameHostProfileError(pub String);

impl fmt::Display for GameHostProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GameHostProfileError {}

/// Chusan-owned `air::Scene` configuration recovered from the concrete
/// MainScene/BgScene construction path. Present dimensions remain explicit:
/// the game copies them from the selected runtime present buffer rather than
/// using a scene-local fixed resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChusanAirSceneTargetProfile {
    pub name: &'static str,
    pub registration_order: i32,
    pub registration_starts_enabled: bool,
    pub present_index: u8,
    pub draw_index: u8,
    pub present_mode: u8,
    pub shader_on_demand: bool,
    pub request_color_offscreen: bool,
    pub request_depth_offscreen: bool,
    pub clear: bool,
}

pub const CHUSAN_MAIN_SCENE: ChusanAirSceneTargetProfile = ChusanAirSceneTargetProfile {
    name: "MainScene",
    registration_order: 10_000,
    registration_starts_enabled: true,
    present_index: 0,
    draw_index: 0,
    present_mode: 1,
    shader_on_demand: false,
    request_color_offscreen: true,
    request_depth_offscreen: true,
    clear: false,
};

pub const CHUSAN_BG_SCENE: ChusanAirSceneTargetProfile = ChusanAirSceneTargetProfile {
    name: "BgScene",
    registration_order: 9_900,
    registration_starts_enabled: false,
    present_index: 0,
    draw_index: 16,
    present_mode: 0,
    shader_on_demand: false,
    request_color_offscreen: true,
    request_depth_offscreen: true,
    clear: false,
};

impl ChusanAirSceneTargetProfile {
    /// Rebuilds the target Camera `Projection * View` used by the game after
    /// `air::Camera` attaches to the scene. The attach callback overwrites the
    /// constructor's Aspect=1 with `scene_width / scene_height`.
    pub fn projection_view_for_present_size(
        self,
        width: u32,
        height: u32,
    ) -> Result<Matrix4x4, GameHostProfileError> {
        if width == 0 || height == 0 {
            return Err(GameHostProfileError(format!(
                "{} present size must be non-zero, got {width}x{height}",
                self.name
            )));
        }

        let aspect = (width as f32) / (height as f32);
        let view = build_look_at_rh_game([0.0, 0.0, 30.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let projection = build_perspective_fov_rh_game(45.0, aspect, 1.0, 30_000.0, 0.0, 0.0);
        Ok(mul_matrix4x4_game(&projection, &view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_and_background_scene_settings_match_the_concrete_chusan_constructor() {
        assert_eq!(CHUSAN_MAIN_SCENE.name, "MainScene");
        assert_eq!(CHUSAN_MAIN_SCENE.registration_order, 10_000);
        assert_eq!(CHUSAN_MAIN_SCENE.draw_index, 0);
        assert_eq!(CHUSAN_MAIN_SCENE.present_mode, 1);

        assert_eq!(CHUSAN_BG_SCENE.name, "BgScene");
        assert_eq!(CHUSAN_BG_SCENE.registration_order, 9_900);
        assert_eq!(CHUSAN_BG_SCENE.draw_index, 16);
        assert_eq!(CHUSAN_BG_SCENE.present_mode, 0);

        for (profile, expected_enabled) in [(CHUSAN_MAIN_SCENE, true), (CHUSAN_BG_SCENE, false)] {
            assert_eq!(profile.registration_starts_enabled, expected_enabled);
        }

        for profile in [CHUSAN_MAIN_SCENE, CHUSAN_BG_SCENE] {
            assert_eq!(profile.present_index, 0);
            assert!(!profile.shader_on_demand);
            assert!(profile.request_color_offscreen);
            assert!(profile.request_depth_offscreen);
            assert!(!profile.clear);
        }
    }

    #[test]
    fn attached_air_camera_uses_the_present_aspect_ratio() {
        let square = CHUSAN_MAIN_SCENE
            .projection_view_for_present_size(1080, 1080)
            .unwrap();
        let portrait = CHUSAN_MAIN_SCENE
            .projection_view_for_present_size(1080, 1920)
            .unwrap();

        assert_eq!(square.rows[1], portrait.rows[1]);
        assert_eq!(portrait.rows[0][0], square.rows[0][0] * (1920.0 / 1080.0));
        assert_eq!(square.rows[2], portrait.rows[2]);
        assert_eq!(square.rows[3], portrait.rows[3]);
    }

    #[test]
    fn present_dimensions_are_required_instead_of_assuming_a_game_resolution() {
        assert!(
            CHUSAN_MAIN_SCENE
                .projection_view_for_present_size(0, 1080)
                .is_err()
        );
        assert!(
            CHUSAN_MAIN_SCENE
                .projection_view_for_present_size(1920, 0)
                .is_err()
        );
    }
}
