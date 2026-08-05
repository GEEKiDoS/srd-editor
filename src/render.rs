use crate::projection::{Matrix4x4, identity_matrix4x4_game};
use crate::shader::{CeylonShadowParallelParameters, CeylonSimpleShaderBits};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9PrimitiveType {
    TriangleStrip = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9RenderState {
    ZEnable = 7,
    FillMode = 8,
    ZWriteEnable = 14,
    AlphaTestEnable = 15,
    SourceBlend = 19,
    DestinationBlend = 20,
    CullMode = 22,
    ZFunction = 23,
    AlphaReference = 24,
    AlphaFunction = 25,
    AlphaBlendEnable = 27,
    StencilEnable = 52,
    StencilFail = 53,
    StencilZFail = 54,
    StencilPass = 55,
    StencilFunction = 56,
    StencilReference = 57,
    StencilMask = 58,
    StencilWriteMask = 59,
    BlendOperation = 171,
    ColorWriteEnable = 168,
    ScissorTestEnable = 174,
    SlopeScaleDepthBias = 175,
    BlendFactor = 193,
    DepthBias = 195,
    SeparateAlphaBlendEnable = 206,
    SourceBlendAlpha = 207,
    DestinationBlendAlpha = 208,
    BlendOperationAlpha = 209,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9CullMode {
    None = 1,
    Clockwise = 2,
    CounterClockwise = 3,
}

pub const fn d3d9_cull_mode_from_internal(value: u32) -> Option<D3d9CullMode> {
    match value {
        0 => Some(D3d9CullMode::Clockwise),
        1 => Some(D3d9CullMode::CounterClockwise),
        2 | 3 => Some(D3d9CullMode::None),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9FillMode {
    Point = 1,
    Wireframe = 2,
    Solid = 3,
}

pub const fn d3d9_fill_mode_from_internal(value: u32) -> D3d9FillMode {
    match value {
        0 => D3d9FillMode::Point,
        1 => D3d9FillMode::Wireframe,
        _ => D3d9FillMode::Solid,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9ComparisonFunction {
    Never = 1,
    Less = 2,
    Equal = 3,
    LessEqual = 4,
    Greater = 5,
    NotEqual = 6,
    GreaterEqual = 7,
    Always = 8,
}

pub const fn d3d9_comparison_function_from_internal(value: u32) -> Option<D3d9ComparisonFunction> {
    match value {
        0 => Some(D3d9ComparisonFunction::Never),
        1 => Some(D3d9ComparisonFunction::Always),
        2 => Some(D3d9ComparisonFunction::Equal),
        3 => Some(D3d9ComparisonFunction::NotEqual),
        4 => Some(D3d9ComparisonFunction::Less),
        5 => Some(D3d9ComparisonFunction::LessEqual),
        6 => Some(D3d9ComparisonFunction::Greater),
        7 => Some(D3d9ComparisonFunction::GreaterEqual),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9StencilOperation {
    Keep = 1,
    Zero = 2,
    Replace = 3,
    IncrementSaturate = 4,
    DecrementSaturate = 5,
    Invert = 6,
    Increment = 7,
    Decrement = 8,
}

pub const fn d3d9_stencil_operation_from_internal(value: u32) -> Option<D3d9StencilOperation> {
    match value {
        0 => Some(D3d9StencilOperation::Keep),
        1 => Some(D3d9StencilOperation::Zero),
        2 => Some(D3d9StencilOperation::Replace),
        3 => Some(D3d9StencilOperation::IncrementSaturate),
        4 => Some(D3d9StencilOperation::DecrementSaturate),
        5 => Some(D3d9StencilOperation::Invert),
        6 => Some(D3d9StencilOperation::Increment),
        7 => Some(D3d9StencilOperation::Decrement),
        _ => None,
    }
}

/// Raw f32 bits loaded by `d3d9_flush_depth_state` before D3DRS_DEPTHBIAS.
/// Keeping the binary word avoids replacing the game's value with a rounded
/// decimal approximation.
pub const CEYLON_DEPTH_BIAS_SCALE_BITS: u32 = 3_045_472_189;

/// Reproduces `movd` + `cvtdq2ps` + `mulss` and returns the DWORD passed to
/// `IDirect3DDevice9::SetRenderState(D3DRS_DEPTHBIAS, ...)`.
pub fn d3d9_depth_bias_bits(value: i32) -> u32 {
    ((value as f32) * f32::from_bits(CEYLON_DEPTH_BIAS_SCALE_BITS)).to_bits()
}

/// The binary negates slope-scale depth bias with XORPS against a sign-bit
/// mask. Flipping the bit directly also preserves signed zero and NaN payloads.
pub const fn d3d9_slope_scale_depth_bias_bits(value: f32) -> u32 {
    value.to_bits() ^ (-0.0_f32).to_bits()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct D3d9Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CeylonMaterialScissorSource {
    /// Only bit 0x20 has a proven scissor meaning in this source record.
    pub flags_00: u8,
    /// Four LONGs copied from source offset +0x24.
    pub rectangle_24: D3d9Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CeylonScissorStateCommand {
    pub enabled: bool,
    pub rectangle: D3d9Rect,
}

pub const CEYLON_MATERIAL_OVERRIDE_SCISSOR_ENABLE: u32 = 0x0200_0000;
pub const CEYLON_MATERIAL_OVERRIDE_SCISSOR_RECTANGLE: u32 = 0x0400_0000;

/// Reproduces the two independent source selections in
/// `sea_material_sync_render_commands`. The enable and rectangle do not have
/// to come from the same source record.
pub const fn ceylon_select_material_scissor_command(
    base: CeylonMaterialScissorSource,
    override_source: CeylonMaterialScissorSource,
    override_mask: u32,
) -> CeylonScissorStateCommand {
    let enable_source = if override_mask & CEYLON_MATERIAL_OVERRIDE_SCISSOR_ENABLE != 0 {
        override_source
    } else {
        base
    };
    let rectangle_source = if override_mask & CEYLON_MATERIAL_OVERRIDE_SCISSOR_RECTANGLE != 0 {
        override_source
    } else {
        base
    };
    CeylonScissorStateCommand {
        enabled: enable_source.flags_00 & 0x20 != 0,
        rectangle: rectangle_source.rectangle_24,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonRenderScissorState {
    pub enabled: bool,
    pub rectangle: D3d9Rect,
}

impl Default for CeylonRenderScissorState {
    /// Exact fields written by `ceylon_reset_render_state_defaults`. The
    /// disabled rectangle's left value is still preserved even though D3D9
    /// does not consume the rectangle while the test is disabled.
    fn default() -> Self {
        Self {
            enabled: false,
            rectangle: D3d9Rect {
                left: 5,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }
}

impl CeylonRenderScissorState {
    pub fn apply_command(&mut self, command: CeylonScissorStateCommand) {
        self.enabled = command.enabled;
        self.rectangle = command.rectangle;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonRasterState {
    pub cull_mode_internal: u32,
    pub fill_mode_internal: u32,
    pub color_write_mask: u32,
}

impl Default for CeylonRasterState {
    fn default() -> Self {
        Self {
            cull_mode_internal: 1,
            fill_mode_internal: 2,
            color_write_mask: 0x0f,
        }
    }
}

impl CeylonRasterState {
    /// Reproduces the raster fields assembled in
    /// `ceylon_apply_draw_packet_state` after the base RenderState copy.
    pub fn apply_draw_packet(&mut self, packet: CeylonDrawPacketPresetState) {
        if packet.draw_flags_00 & 0x0080_0000 == 0 {
            self.cull_mode_internal = 2;
        }

        let mut color_write_mask = u32::from(packet.draw_flags_00 & 0x2000 != 0);
        if packet.draw_flags_00 & 0x4000 != 0 {
            color_write_mask |= 2;
        }
        if packet.draw_flags_00 & 0x8000 != 0 {
            color_write_mask |= 4;
        }
        if packet.draw_flags_00 & 0x1_0000 != 0 || packet.flags_60 & 0x2000 != 0 {
            color_write_mask |= 8;
        }
        self.color_write_mask = color_write_mask;
    }

    pub fn cull_mode(self) -> Option<D3d9CullMode> {
        d3d9_cull_mode_from_internal(self.cull_mode_internal)
    }

    pub fn fill_mode(self) -> D3d9FillMode {
        d3d9_fill_mode_from_internal(self.fill_mode_internal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9BlendFactor {
    Zero = 1,
    One = 2,
    SourceColor = 3,
    InverseSourceColor = 4,
    SourceAlpha = 5,
    InverseSourceAlpha = 6,
    DestinationAlpha = 7,
    InverseDestinationAlpha = 8,
    DestinationColor = 9,
    InverseDestinationColor = 10,
    SourceAlphaSaturate = 11,
    BlendFactor = 14,
    InverseBlendFactor = 15,
}

pub const fn d3d9_blend_factor_from_internal(value: u32) -> Option<D3d9BlendFactor> {
    match value {
        0 => Some(D3d9BlendFactor::Zero),
        1 => Some(D3d9BlendFactor::One),
        2 => Some(D3d9BlendFactor::SourceColor),
        3 => Some(D3d9BlendFactor::InverseSourceColor),
        4 => Some(D3d9BlendFactor::SourceAlpha),
        5 => Some(D3d9BlendFactor::InverseSourceAlpha),
        6 => Some(D3d9BlendFactor::DestinationColor),
        7 => Some(D3d9BlendFactor::InverseDestinationColor),
        8 => Some(D3d9BlendFactor::DestinationAlpha),
        9 => Some(D3d9BlendFactor::InverseDestinationAlpha),
        10 => Some(D3d9BlendFactor::SourceAlphaSaturate),
        11 => Some(D3d9BlendFactor::BlendFactor),
        12 => Some(D3d9BlendFactor::InverseBlendFactor),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9BlendOperation {
    Add = 1,
    Subtract = 2,
    ReverseSubtract = 3,
    Minimum = 4,
    Maximum = 5,
}

pub const fn d3d9_blend_operation_from_internal(value: u32) -> Option<D3d9BlendOperation> {
    match value {
        0 => Some(D3d9BlendOperation::Add),
        1 => Some(D3d9BlendOperation::Subtract),
        2 => Some(D3d9BlendOperation::ReverseSubtract),
        3 => Some(D3d9BlendOperation::Minimum),
        4 => Some(D3d9BlendOperation::Maximum),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrdD3d9BlendPreset {
    pub alpha_blend_enabled: bool,
    pub source_blend: D3d9BlendFactor,
    pub destination_blend: D3d9BlendFactor,
    pub blend_operation: D3d9BlendOperation,
    pub alpha_test_enabled: bool,
    pub separate_alpha_blend_enabled: bool,
    pub source_blend_alpha: D3d9BlendFactor,
    pub destination_blend_alpha: D3d9BlendFactor,
    pub blend_operation_alpha: D3d9BlendOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlendEquation {
    source_blend: D3d9BlendFactor,
    destination_blend: D3d9BlendFactor,
    blend_operation: D3d9BlendOperation,
}

const fn blend_equation(
    source_blend: D3d9BlendFactor,
    destination_blend: D3d9BlendFactor,
    blend_operation: D3d9BlendOperation,
) -> BlendEquation {
    BlendEquation {
        source_blend,
        destination_blend,
        blend_operation,
    }
}

const fn blend_preset(
    alpha_blend_enabled: bool,
    color: BlendEquation,
    alpha_test_enabled: bool,
    separate_alpha_blend_enabled: bool,
    alpha: BlendEquation,
) -> SrdD3d9BlendPreset {
    SrdD3d9BlendPreset {
        alpha_blend_enabled,
        source_blend: color.source_blend,
        destination_blend: color.destination_blend,
        blend_operation: color.blend_operation,
        alpha_test_enabled,
        separate_alpha_blend_enabled,
        source_blend_alpha: alpha.source_blend,
        destination_blend_alpha: alpha.destination_blend,
        blend_operation_alpha: alpha.blend_operation,
    }
}

const PRESET_0: SrdD3d9BlendPreset = blend_preset(
    false,
    blend_equation(
        D3d9BlendFactor::SourceAlpha,
        D3d9BlendFactor::InverseSourceAlpha,
        D3d9BlendOperation::Add,
    ),
    false,
    false,
    blend_equation(
        D3d9BlendFactor::Zero,
        D3d9BlendFactor::Zero,
        D3d9BlendOperation::Add,
    ),
);
const PRESET_1: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_test_enabled: true,
    ..PRESET_0
};
const PRESET_2: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    alpha_test_enabled: true,
    ..PRESET_0
};
const PRESET_3: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    ..PRESET_0
};
const PRESET_4: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    destination_blend: D3d9BlendFactor::One,
    ..PRESET_0
};
const PRESET_5: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    destination_blend: D3d9BlendFactor::One,
    blend_operation: D3d9BlendOperation::ReverseSubtract,
    ..PRESET_0
};
const PRESET_6: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    source_blend: D3d9BlendFactor::Zero,
    destination_blend: D3d9BlendFactor::SourceColor,
    ..PRESET_0
};
const PRESET_7: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    source_blend: D3d9BlendFactor::InverseDestinationColor,
    destination_blend: D3d9BlendFactor::Zero,
    ..PRESET_0
};
const PRESET_8: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    source_blend: D3d9BlendFactor::DestinationColor,
    destination_blend: D3d9BlendFactor::One,
    ..PRESET_0
};
const PRESET_9: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_test_enabled: true,
    ..PRESET_6
};
const PRESET_10: SrdD3d9BlendPreset = blend_preset(
    true,
    blend_equation(
        D3d9BlendFactor::One,
        D3d9BlendFactor::Zero,
        D3d9BlendOperation::Add,
    ),
    false,
    true,
    blend_equation(
        D3d9BlendFactor::Zero,
        D3d9BlendFactor::Zero,
        D3d9BlendOperation::Add,
    ),
);
const PRESET_11: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: false,
    ..PRESET_10
};
const PRESET_12: SrdD3d9BlendPreset = blend_preset(
    true,
    blend_equation(
        D3d9BlendFactor::SourceAlpha,
        D3d9BlendFactor::InverseSourceAlpha,
        D3d9BlendOperation::Add,
    ),
    true,
    true,
    blend_equation(
        D3d9BlendFactor::Zero,
        D3d9BlendFactor::InverseSourceAlpha,
        D3d9BlendOperation::Add,
    ),
);
const PRESET_13: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_test_enabled: false,
    ..PRESET_12
};
const PRESET_14: SrdD3d9BlendPreset = blend_preset(
    true,
    blend_equation(
        D3d9BlendFactor::SourceAlpha,
        D3d9BlendFactor::One,
        D3d9BlendOperation::Add,
    ),
    false,
    true,
    blend_equation(
        D3d9BlendFactor::Zero,
        D3d9BlendFactor::One,
        D3d9BlendOperation::Add,
    ),
);
const PRESET_18: SrdD3d9BlendPreset = SrdD3d9BlendPreset {
    alpha_blend_enabled: true,
    source_blend: D3d9BlendFactor::DestinationColor,
    destination_blend: D3d9BlendFactor::InverseSourceAlpha,
    ..PRESET_0
};

/// Exact 62-entry table at `byte_18A6908`. Indices 22 through 60 are
/// intentionally identical; the game still preserves them as distinct IDs.
pub const SRD_D3D9_BLEND_PRESETS: [SrdD3d9BlendPreset; 62] = [
    PRESET_0,  // 0
    PRESET_1,  // 1
    PRESET_2,  // 2
    PRESET_3,  // 3
    PRESET_4,  // 4
    PRESET_5,  // 5
    PRESET_6,  // 6
    PRESET_7,  // 7
    PRESET_8,  // 8
    PRESET_9,  // 9
    PRESET_10, // 10
    PRESET_11, // 11
    PRESET_12, // 12
    PRESET_13, // 13
    PRESET_14, // 14
    PRESET_5,  // 15
    PRESET_6,  // 16
    PRESET_7,  // 17
    PRESET_18, // 18
    PRESET_9,  // 19
    PRESET_0,  // 20
    PRESET_1,  // 21
    PRESET_0,  // 22
    PRESET_0,  // 23
    PRESET_0,  // 24
    PRESET_0,  // 25
    PRESET_0,  // 26
    PRESET_0,  // 27
    PRESET_0,  // 28
    PRESET_0,  // 29
    PRESET_0,  // 30
    PRESET_0,  // 31
    PRESET_0,  // 32
    PRESET_0,  // 33
    PRESET_0,  // 34
    PRESET_0,  // 35
    PRESET_0,  // 36
    PRESET_0,  // 37
    PRESET_0,  // 38
    PRESET_0,  // 39
    PRESET_0,  // 40
    PRESET_0,  // 41
    PRESET_0,  // 42
    PRESET_0,  // 43
    PRESET_0,  // 44
    PRESET_0,  // 45
    PRESET_0,  // 46
    PRESET_0,  // 47
    PRESET_0,  // 48
    PRESET_0,  // 49
    PRESET_0,  // 50
    PRESET_0,  // 51
    PRESET_0,  // 52
    PRESET_0,  // 53
    PRESET_0,  // 54
    PRESET_0,  // 55
    PRESET_0,  // 56
    PRESET_0,  // 57
    PRESET_0,  // 58
    PRESET_0,  // 59
    PRESET_0,  // 60
    PRESET_3,  // 61
];

pub fn ceylon_d3d9_blend_preset(preset_id: i32) -> SrdD3d9BlendPreset {
    let table_index = preset_id.clamp(0, 61) as usize;
    SRD_D3D9_BLEND_PRESETS[table_index]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CeylonDrawPacketPresetState {
    pub draw_flags_00: u32,
    pub packed_08: u32,
    pub flags_0c: u32,
    pub field_2c: i32,
    pub flags_58: u32,
    pub flags_60: u32,
    pub flags_64: u32,
}

/// Exact packet `+0x88` value constructed by `ceylon_construct_draw_packet`.
/// The same dword is exposed as `SrRenderer+0x198`; its low word is consumed
/// as the target-pass order value.
pub const SRD_RENDERER_INITIAL_LAYER_KEY: u32 = 0x0000_8580;

/// Reproduces `SrPlayer` property 6 (`2DLayer`) synchronization at
/// `sub_AAD040`: only bits 8..14 are replaced, while kind bit 15 and the
/// initial level byte remain those established by the draw-packet constructor.
pub const fn srd_renderer_layer_key_for_2d_layer(layer_2d: u8) -> u32 {
    (SRD_RENDERER_INITIAL_LAYER_KEY & !0x7f00) | (((layer_2d as u32) << 8) & 0x7f00)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CeylonSrdFixedShaderConstants {
    pub vertex_c0_c3_world: Matrix4x4,
    pub vertex_c4_c7: Matrix4x4,
    pub vertex_c8_fixed_param0: [f32; 4],
    pub vertex_c9_fixed_param1: [f32; 4],
    pub vertex_c10_screen_param: [f32; 4],
    pub vertex_c10_c13_projection_view: Matrix4x4,
    pub pixel_c0_fixed_param0: [f32; 4],
}

impl CeylonSrdFixedShaderConstants {
    /// Builds both mutually exclusive c10 inputs used by the proven SRD quad
    /// shader variants. A 2D draw uploads `screenParam = [width/2, height/2,
    /// 0, 0]`; a 3D draw uploads the target Camera's Projection*View matrix.
    pub fn initial_for_target(
        target_projection_view: Matrix4x4,
        target_screen_size: [u32; 2],
    ) -> Self {
        Self {
            vertex_c0_c3_world: identity_matrix4x4_game(),
            vertex_c4_c7: identity_matrix4x4_game(),
            vertex_c8_fixed_param0: [0.0; 4],
            vertex_c9_fixed_param1: [0.0; 4],
            vertex_c10_screen_param: [
                target_screen_size[0] as f32 * 0.5,
                target_screen_size[1] as f32 * 0.5,
                0.0,
                0.0,
            ],
            vertex_c10_c13_projection_view: target_projection_view,
            pixel_c0_fixed_param0: [0.0, 0.0, 1.0, 0.0],
        }
    }
}

impl CeylonDrawPacketPresetState {
    /// State left by the Ceylon draw-packet constructor followed by
    /// `srd_construct_renderer`'s packet mask. This is separate from Rust's
    /// zero `Default` because callers also use the type for isolated overrides.
    pub const fn srd_renderer_initial() -> Self {
        Self {
            draw_flags_00: 0x0029_e000,
            packed_08: 0,
            flags_0c: 0,
            field_2c: 0,
            flags_58: 0xff,
            flags_60: 0x4000,
            flags_64: 0,
        }
    }

    pub fn encoded_preset_id(self) -> u8 {
        (self.draw_flags_00 & 0x3f) as u8
    }

    pub fn table_preset_id(self) -> u8 {
        self.encoded_preset_id().min(61)
    }

    /// Reproduces `ceylon_set_draw_render_preset_id` and the derived-flag
    /// refresh it invokes. The signed input is intentionally not narrowed
    /// before the binary's low-six-bit packet encoding and `> 32` tests.
    pub fn set_render_preset_id(&mut self, requested_preset_id: i32) {
        let low_six_bits = (requested_preset_id as u8) & 0x3f;
        self.draw_flags_00 = (self.draw_flags_00 & !0x3f) | u32::from(low_six_bits);

        if requested_preset_id > 32 {
            self.flags_60 |= 0x800;
        } else {
            self.flags_60 &= !0x800;
        }

        let preset = ceylon_d3d9_blend_preset(i32::from(self.encoded_preset_id()));
        if preset.alpha_blend_enabled {
            self.flags_60 |= 0x20;
            self.flags_58 &= !0x8;
        } else {
            self.flags_60 &= !0x20;
            self.flags_58 |= 0x8;
        }
        if preset.alpha_test_enabled {
            self.flags_60 |= 0x40;
        } else {
            self.flags_60 &= !0x40;
        }

        if requested_preset_id > 32 {
            self.flags_60 |= 0x20;
        }
    }

    /// Reproduces the bit update in `ceylon_submit_vertex_batch`: the boolean
    /// captured by `srd_begin_quad_draw` becomes packet+0x60 bit 7 before the
    /// ShapeEnv key is generated.
    pub fn set_srd_quad_is_2d(&mut self, is_2d: bool) {
        let encoded = u32::from(is_2d) << 7;
        self.flags_60 ^= (self.flags_60 ^ encoded) & 0x80;
    }

    pub fn srd_quad_shader_key(self, texture_present: [bool; 3]) -> CeylonShaderKey {
        CeylonShaderKeyInput {
            draw_flags_00: self.draw_flags_00,
            field_28: 31,
            field_2c: self.field_2c as u32,
            flags_60: self.flags_60,
            vertex_format_70: 14,
            texture_present,
        }
        .shader_key()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonShaderKeyInput {
    pub draw_flags_00: u32,
    pub field_28: u32,
    pub field_2c: u32,
    pub flags_60: u32,
    pub vertex_format_70: u32,
    pub texture_present: [bool; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeylonShaderKey {
    pub low: u32,
    pub high: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrdSimpleShaderContributionError {
    UnsupportedVertexFormat(u32),
    UnsupportedBaseEnvironment(u32),
}

impl CeylonShaderKey {
    /// Reproduces every Simple-selector feature contributed directly by the
    /// Ceylon ShapeEnv object built from this key for the proven SRD format 14
    /// and Fennel format 13 paths.
    ///
    /// Renderer-global parameter providers are deliberately outside this
    /// method: they are not encoded in the 64-bit ShapeEnv cache key. In
    /// particular, key low bit 6 only gates shadow parameters 43..48 if an
    /// independent global provider supplied parameter 43, and low bit 7 is
    /// not read by `SimpleShaderSelector`.
    pub fn srd_simple_shader_direct_contributions(
        self,
    ) -> Result<CeylonSimpleShaderBits, SrdSimpleShaderContributionError> {
        let vertex_format = (self.low >> 15) & 0x1f;
        let mut bits = CeylonSimpleShaderBits::default();
        match vertex_format {
            13 => bits.apply_fennel_vertex_format_13(),
            14 => bits.apply_srd_vertex_format_14(),
            _ => {
                return Err(SrdSimpleShaderContributionError::UnsupportedVertexFormat(
                    vertex_format,
                ));
            }
        }

        // State slots are allocated from low 13..14. The selector takes the
        // maximum of this count and the number of actually bound textures.
        // The ShapeEnv cache key itself was formed from those same non-null
        // packet slots, so the direct SRD result is exactly this count.
        bits.apply_enabled_texture_count(((self.low >> 13) & 3) as usize);

        // The State alpha-test field starts at zero and this constructor does
        // not modify it, so position 23 remains clear. Alpha blend and the
        // inverted NoUpdateDistance flag are written explicitly by the key.
        bits.set(24, self.low & (1 << 4) != 0);
        bits.set(20, self.low & (1 << 5) == 0);

        match self.low & 7 {
            0 => {}
            1 => bits
                .set_selector_parameter_value(23, 1)
                .expect("parameter 23 is registered"),
            2 => bits
                .set_selector_parameter_value(24, 1)
                .expect("parameter 24 is registered"),
            3 => bits
                .set_selector_parameter_value(24, 2)
                .expect("parameter 24 is registered"),
            4 => bits
                .set_selector_parameter_value(66, 1)
                .expect("parameter 66 is registered"),
            variant => {
                return Err(SrdSimpleShaderContributionError::UnsupportedBaseEnvironment(variant));
            }
        }
        // `ceylon_environment_manager_construct` stores `sea::ShapeEnv2D`
        // only at manager+0x180. `ceylon_create_shape_environment` applies
        // that module exactly when key low bit 3 is set; its virtual apply
        // method raises integer selector parameter 1 to value 1, which the
        // Simple selector maps to position 2 (`SSF_2DTransform`).
        if self.low & (1 << 3) != 0 {
            bits.set_selector_parameter_value(1, 1)
                .expect("parameter 1 is registered");
        }

        bits.apply_shape_environment_variants(
            (self.low >> 20) & 0x3f,
            (self.low >> 26) & 0x0f,
            self.high & 7,
        );
        Ok(bits)
    }

    /// Combines the direct SRD ShapeEnv contributions with an explicitly
    /// supplied renderer-global `LightShadowParallel` parameter context.
    /// This does not assume that such a graph node is active for every SRD
    /// draw; callers must provide the context established by their scene.
    pub fn srd_simple_shader_with_shadow_parallel(
        self,
        parameters: CeylonShadowParallelParameters,
    ) -> Result<CeylonSimpleShaderBits, SrdSimpleShaderContributionError> {
        let mut bits = self.srd_simple_shader_direct_contributions()?;
        bits.set_shadow_parallel_contribution(parameters, self.low & (1 << 6) != 0);
        Ok(bits)
    }
}

impl CeylonShaderKeyInput {
    /// Reproduces the complete 64-bit ShapeEnv cache key built by
    /// `sub_671480`. This identifies the game's shader-module combination;
    /// it does not infer the generated pixel formula.
    pub fn shader_key(self) -> CeylonShaderKey {
        let preset = ceylon_d3d9_blend_preset((self.draw_flags_00 & 0x3f) as i32);
        let mut low = ((self.flags_60 >> 4) & 8) | (self.field_2c & 7);
        low |= (self.draw_flags_00 >> 12) & 0x20;
        low |= 16u32.wrapping_mul(
            u32::from(preset.alpha_blend_enabled) | ((self.draw_flags_00 & 0x3ff) << 16),
        ) & 0xfff0_603f;

        let vertex_format = (self.vertex_format_70 & 0x1f) << 7;
        low |= (self.flags_60 >> 7) & 0x80;
        low |= 8u32.wrapping_mul(
            (self.flags_60 & 8) | 32u32.wrapping_mul((self.field_28 & 0x1f) | vertex_format),
        );

        for present in self.texture_present {
            if present {
                let incremented = (low & 0xe000).wrapping_add(0x2000);
                low ^= (low ^ incremented) & 0x6000;
            }
        }

        CeylonShaderKey {
            low,
            high: (self.draw_flags_00 >> 10) & 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonAlphaStencilState {
    pub alpha_test_enabled: bool,
    pub alpha_reference: u32,
    pub alpha_function_internal: u32,
    pub stencil_enabled: bool,
    pub stencil_function_internal: u32,
    pub stencil_fail_internal: u32,
    pub stencil_z_fail_internal: u32,
    pub stencil_pass_internal: u32,
    pub stencil_reference: u32,
    pub stencil_mask: u32,
    pub stencil_write_mask: u32,
}

impl CeylonAlphaStencilState {
    /// Applies the conditional packet override block in
    /// `ceylon_apply_draw_packet_state` to a caller-supplied base RenderState.
    pub fn apply_draw_packet(&mut self, packet: CeylonDrawPacketPresetState) {
        self.stencil_enabled = packet.flags_0c & 0x100 != 0;
        if packet.flags_0c & 0x100 == 0 {
            return;
        }

        self.stencil_function_internal = packet.packed_08 & 0x0f;
        if self.stencil_function_internal == 1 {
            self.alpha_test_enabled = true;
            self.alpha_reference = 128;
        }
        self.stencil_fail_internal = (packet.packed_08 >> 4) & 0x0f;
        self.stencil_z_fail_internal = (packet.packed_08 >> 8) & 0x0f;
        self.stencil_pass_internal = (packet.packed_08 >> 12) & 0x0f;
        self.stencil_reference = (packet.packed_08 >> 16) & 0xff;
        self.stencil_mask = (packet.packed_08 >> 24) & 0xff;
        self.stencil_write_mask = packet.flags_0c & 0xff;
    }

    pub fn alpha_function(self) -> Option<D3d9ComparisonFunction> {
        d3d9_comparison_function_from_internal(self.alpha_function_internal)
    }

    pub fn stencil_function(self) -> Option<D3d9ComparisonFunction> {
        d3d9_comparison_function_from_internal(self.stencil_function_internal)
    }

    pub fn stencil_fail(self) -> Option<D3d9StencilOperation> {
        d3d9_stencil_operation_from_internal(self.stencil_fail_internal)
    }

    pub fn stencil_z_fail(self) -> Option<D3d9StencilOperation> {
        d3d9_stencil_operation_from_internal(self.stencil_z_fail_internal)
    }

    pub fn stencil_pass(self) -> Option<D3d9StencilOperation> {
        d3d9_stencil_operation_from_internal(self.stencil_pass_internal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonDepthState {
    pub z_enabled: bool,
    pub z_write_enabled: bool,
    pub z_function_internal: u32,
}

impl CeylonDepthState {
    pub fn from_draw_flags(draw_flags_00: u32) -> Self {
        Self {
            z_enabled: draw_flags_00 & 0x40000 != 0,
            z_write_enabled: draw_flags_00 & 0x20000 != 0,
            z_function_internal: (draw_flags_00 >> 19) & 0x0f,
        }
    }

    pub fn z_function(self) -> Option<D3d9ComparisonFunction> {
        d3d9_comparison_function_from_internal(self.z_function_internal)
    }
}

fn set_packet_mask_bytes(packet: &mut CeylonDrawPacketPresetState, mask: u8) {
    packet.packed_08 =
        (packet.packed_08 & 0x0000_ffff) | (u32::from(mask) << 16) | (u32::from(mask) << 24);
    packet.flags_0c = (packet.flags_0c & !0xff) | u32::from(mask);
}

fn set_srd_stencil_packet(
    packet: &mut CeylonDrawPacketPresetState,
    mask: u8,
    comparison_internal: u32,
) {
    packet.packed_08 = comparison_internal;
    packet.flags_0c = 0x100;
    packet.draw_flags_00 |= 0x1e000;
    set_packet_mask_bytes(packet, mask);
}

/// Reproduces the alpha/stencil packet writes in `sub_AC5320` without assigning
/// unproven business names to `SrImage+0x10/+0x14/+0x18`.
pub fn apply_srd_image_alpha_stencil_packet_fields(
    packet: &mut CeylonDrawPacketPresetState,
    image_field_10: i32,
    image_field_14: u32,
    image_field_18: u8,
    renderer_mask_274: u8,
    renderer_counter_198: &mut u8,
) {
    let shift = image_field_14 & 0x1f;
    let image_mask = 1u8.checked_shl(shift).unwrap_or(0);

    match image_field_10 {
        1 | 2 => {
            *renderer_counter_198 = renderer_counter_198.wrapping_sub(1);
            packet.packed_08 = 0x2001;
            packet.flags_0c = 0x100;
            packet.draw_flags_00 &= !0x1e000;
            set_packet_mask_bytes(packet, image_mask);
        }
        3 if image_field_18 == 0 => {
            set_srd_stencil_packet(packet, image_mask | renderer_mask_274, 2);
        }
        4 if image_field_18 == 0 => {
            set_srd_stencil_packet(packet, image_mask | renderer_mask_274, 3);
        }
        _ if renderer_mask_274 != 0 => {
            set_srd_stencil_packet(packet, renderer_mask_274, 2);
        }
        _ => {
            packet.draw_flags_00 |= 0x1e000;
            packet.packed_08 = 0;
            packet.flags_0c = 0;
        }
    }
}

/// Reproduces only the draw-packet fields written by `sub_AC5320`'s renderer
/// special-mode depth branch. The same branch also updates renderer-private
/// ordering fields, which are deliberately not represented here yet.
pub fn apply_srd_special_depth_packet_fields(
    packet: &mut CeylonDrawPacketPresetState,
    renderer_special_mode: bool,
    image_field_1c: i32,
) {
    if !renderer_special_mode {
        return;
    }
    if image_field_1c < 0 {
        packet.draw_flags_00 &= !0x20000;
    } else {
        packet.draw_flags_00 |= 0x20000;
    }
    packet.draw_flags_00 |= 0x40000;
    packet.field_2c = 4;
}

/// Reproduces the `SrImage+0x0C` branch at the start of
/// `srd_begin_quad_draw`. The five-entry stack table is formed by the exact
/// `paddd` constants at `0x18A5130` and `0x190D8B0` before bits 6..9 are
/// replaced in the draw flags.
pub fn apply_srd_image_field_0c_shader_bits(
    packet: &mut CeylonDrawPacketPresetState,
    image_field_0c: i32,
) {
    const TABLE: [u32; 5] = [0, 9, 0x0a00, 0x000b_0000, 0x0c00_0000];
    let index = image_field_0c.clamp(0, 4) as usize;
    let encoded = TABLE[index].wrapping_shl(6) & 0x3c0;
    packet.draw_flags_00 = (packet.draw_flags_00 & !0x3c0) | encoded;
}

pub fn select_srd_image_render_preset(
    image_flags: u32,
    render_preset_override: i32,
    renderer_special_mode: bool,
) -> Option<i32> {
    if render_preset_override >= 0 {
        return Some(render_preset_override);
    }
    match image_flags & 0x0f {
        0 => {
            if renderer_special_mode {
                match image_flags & 0x600 {
                    0x200 => Some(20),
                    0x400 => Some(21),
                    _ => Some(3),
                }
            } else {
                Some(3)
            }
        }
        1 => Some(4),
        2 => Some(5),
        3 => Some(9),
        _ => None,
    }
}

/// Applies the same preset and `SrImage+0x1C` side effects as
/// `srd_select_image_render_preset`. Returns whether the binary calls the
/// draw-packet preset setter; all five game callers discard its machine-level
/// return value.
pub fn apply_srd_image_render_preset(
    image_flags: u32,
    render_preset_override: i32,
    renderer_special_mode: bool,
    image_field_1c: &mut i32,
    global_field_1c: &mut i32,
    draw_packet: &mut CeylonDrawPacketPresetState,
) -> bool {
    let Some(preset_id) =
        select_srd_image_render_preset(image_flags, render_preset_override, renderer_special_mode)
    else {
        return false;
    };

    draw_packet.set_render_preset_id(preset_id);
    if render_preset_override < 0 && image_flags & 0x0f == 0 {
        match (renderer_special_mode, image_flags & 0x600) {
            (true, 0x200 | 0x400) => {
                *global_field_1c = global_field_1c.wrapping_add(1);
                *image_field_1c = *global_field_1c;
            }
            _ => *image_field_1c = -1,
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum D3d9DeclarationType {
    Float2 = 1,
    Float3 = 2,
    Color = 4,
    Unused = 17,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum D3d9DeclarationUsage {
    Position = 0,
    TextureCoordinate = 5,
    Color = 10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct D3d9VertexElement {
    pub stream: u16,
    pub offset: u16,
    pub declaration_type: u8,
    pub method: u8,
    pub usage: u8,
    pub usage_index: u8,
}

pub const SRD_D3D9_VERTEX_DECLARATION: [D3d9VertexElement; 6] = [
    D3d9VertexElement {
        stream: 0,
        offset: 0,
        declaration_type: D3d9DeclarationType::Float3 as u8,
        method: 0,
        usage: D3d9DeclarationUsage::Position as u8,
        usage_index: 0,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 12,
        declaration_type: D3d9DeclarationType::Color as u8,
        method: 0,
        usage: D3d9DeclarationUsage::Color as u8,
        usage_index: 0,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 16,
        declaration_type: D3d9DeclarationType::Color as u8,
        method: 0,
        usage: D3d9DeclarationUsage::Color as u8,
        usage_index: 1,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 20,
        declaration_type: D3d9DeclarationType::Float2 as u8,
        method: 0,
        usage: D3d9DeclarationUsage::TextureCoordinate as u8,
        usage_index: 0,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 28,
        declaration_type: D3d9DeclarationType::Float2 as u8,
        method: 0,
        usage: D3d9DeclarationUsage::TextureCoordinate as u8,
        usage_index: 1,
    },
    D3d9VertexElement {
        stream: 0xff,
        offset: 0,
        declaration_type: D3d9DeclarationType::Unused as u8,
        method: 0,
        usage: 0,
        usage_index: 0,
    },
];

/// Ceylon vertex format 13 registered by `sub_671D30`: the exact first four
/// elements of format 14 followed by the D3D declaration terminator.
pub const FENNEL_D3D9_VERTEX_DECLARATION: [D3d9VertexElement; 5] = [
    SRD_D3D9_VERTEX_DECLARATION[0],
    SRD_D3D9_VERTEX_DECLARATION[1],
    SRD_D3D9_VERTEX_DECLARATION[2],
    SRD_D3D9_VERTEX_DECLARATION[3],
    SRD_D3D9_VERTEX_DECLARATION[5],
];

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct SrdRenderVertex {
    pub position: [f32; 3],
    pub primary_color: [u8; 4],
    pub secondary_color: [u8; 4],
    pub texture_coordinates: [[f32; 2]; 2],
}

impl SrdRenderVertex {
    pub const BINARY_FORMAT_ID: u32 = 14;
    pub const STRIDE: usize = 36;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrdQuadDraw {
    pub primitive_type: D3d9PrimitiveType,
    pub vertices: [SrdRenderVertex; 4],
}

impl SrdQuadDraw {
    pub const PRIMITIVE_COUNT: u32 = 2;

    pub fn new(vertices: [SrdRenderVertex; 4]) -> Self {
        Self {
            primitive_type: D3d9PrimitiveType::TriangleStrip,
            vertices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_fixed_shader_constants_include_both_proven_c10_sources() {
        let projection_view = Matrix4x4 {
            rows: [[2.0; 4]; 4],
        };
        let constants =
            CeylonSrdFixedShaderConstants::initial_for_target(projection_view, [1920, 1080]);
        assert_eq!(constants.vertex_c0_c3_world, identity_matrix4x4_game());
        assert_eq!(constants.vertex_c4_c7, identity_matrix4x4_game());
        assert_eq!(constants.vertex_c8_fixed_param0, [0.0; 4]);
        assert_eq!(constants.vertex_c9_fixed_param1, [0.0; 4]);
        assert_eq!(constants.vertex_c10_screen_param, [960.0, 540.0, 0.0, 0.0]);
        assert_eq!(constants.vertex_c10_c13_projection_view, projection_view);
        assert_eq!(constants.pixel_c0_fixed_param0, [0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn raster_tables_and_initial_srd_packet_match_the_d3d9_backend() {
        assert_eq!(
            (0..=3)
                .map(d3d9_cull_mode_from_internal)
                .collect::<Vec<_>>(),
            vec![
                Some(D3d9CullMode::Clockwise),
                Some(D3d9CullMode::CounterClockwise),
                Some(D3d9CullMode::None),
                Some(D3d9CullMode::None),
            ]
        );
        assert_eq!(d3d9_cull_mode_from_internal(4), None);
        assert_eq!(d3d9_fill_mode_from_internal(0), D3d9FillMode::Point);
        assert_eq!(d3d9_fill_mode_from_internal(1), D3d9FillMode::Wireframe);
        assert_eq!(d3d9_fill_mode_from_internal(2), D3d9FillMode::Solid);

        let mut raster = CeylonRasterState::default();
        raster.apply_draw_packet(CeylonDrawPacketPresetState::srd_renderer_initial());
        assert_eq!(raster.cull_mode(), Some(D3d9CullMode::None));
        assert_eq!(raster.fill_mode(), D3d9FillMode::Solid);
        assert_eq!(raster.color_write_mask, 0x0f);

        let mut preserve_base = CeylonRasterState::default();
        preserve_base.apply_draw_packet(CeylonDrawPacketPresetState {
            draw_flags_00: 0x0080_0000,
            flags_60: 0,
            ..CeylonDrawPacketPresetState::default()
        });
        assert_eq!(
            preserve_base.cull_mode(),
            Some(D3d9CullMode::CounterClockwise)
        );
        assert_eq!(preserve_base.color_write_mask, 0);
    }

    #[test]
    fn shader_key_maps_every_binary_packet_field_and_texture_count() {
        let base = CeylonShaderKeyInput {
            draw_flags_00: 0,
            field_28: 31,
            field_2c: 5,
            flags_60: 0x4088,
            vertex_format_70: 14,
            texture_present: [false; 3],
        };
        let key = base.shader_key();
        assert_eq!(key.low & 7, 5);
        assert_eq!((key.low >> 3) & 1, 1);
        assert_eq!((key.low >> 6) & 1, 1);
        assert_eq!((key.low >> 7) & 1, 1);
        assert_eq!((key.low >> 8) & 0x1f, 31);
        assert_eq!((key.low >> 13) & 3, 0);
        assert_eq!((key.low >> 15) & 0x1f, 14);
        assert_eq!(key.high, 0);

        for texture_count in 0..=3 {
            let mut input = base;
            input.texture_present = std::array::from_fn(|index| index < texture_count);
            assert_eq!((input.shader_key().low >> 13) & 3, texture_count as u32);
        }
    }

    #[test]
    fn srd_shape_environment_key_maps_all_direct_simple_features() {
        let low = 3 | (1 << 3) | (1 << 4) | (2 << 13) | (14 << 15) | (0x23 << 20) | (9 << 26);
        let bits = CeylonShaderKey { low, high: 5 }
            .srd_simple_shader_direct_contributions()
            .unwrap();

        assert!(!bits.contains(9));
        assert!(bits.contains(10));
        assert!(!bits.contains(11));
        assert!(bits.contains(12));
        assert!(bits.contains(2));
        assert!(bits.contains(20));
        assert!(!bits.contains(23));
        assert!(bits.contains(24));
        assert!(bits.contains(36));
        assert!(bits.contains(37));
        assert!(!bits.contains(38));
        assert!(!bits.contains(39));
        assert!(bits.contains(40));
        for bit in 0..6 {
            assert_eq!(bits.contains(41 + bit), 0x23 & (1 << bit) != 0);
        }
        assert!(bits.contains(47));
        assert!(!bits.contains(48));
        assert!(!bits.contains(49));
        assert!(bits.contains(50));
        assert!(bits.contains(51));
        assert!(!bits.contains(52));
        assert!(bits.contains(53));
    }

    #[test]
    fn srd_direct_simple_mapping_rejects_unproven_key_variants() {
        assert_eq!(
            CeylonShaderKey {
                low: 12 << 15,
                high: 0,
            }
            .srd_simple_shader_direct_contributions(),
            Err(SrdSimpleShaderContributionError::UnsupportedVertexFormat(
                12
            ))
        );
        for variant in 5..=7 {
            assert_eq!(
                CeylonShaderKey {
                    low: variant | (14 << 15),
                    high: 0,
                }
                .srd_simple_shader_direct_contributions(),
                Err(SrdSimpleShaderContributionError::UnsupportedBaseEnvironment(variant))
            );
        }
    }

    #[test]
    fn shape_env_2d_is_the_optional_low_bit_3_module_not_a_base_variant() {
        let base_depth_write = CeylonShaderKey {
            low: 4 | (14 << 15),
            high: 0,
        }
        .srd_simple_shader_direct_contributions()
        .unwrap();
        assert!(base_depth_write.contains(69));
        assert!(!base_depth_write.contains(2));

        let optional_2d = CeylonShaderKey {
            low: (1 << 3) | (14 << 15),
            high: 0,
        }
        .srd_simple_shader_direct_contributions()
        .unwrap();
        assert!(optional_2d.contains(2));
        assert!(!optional_2d.contains(69));
    }

    #[test]
    fn srd_shadow_context_is_global_but_shape_key_low_bit_6_gates_details() {
        let parameters = CeylonShadowParallelParameters::from_light_shadow_parallel(2, 4, true, 2);
        let ungated = CeylonShaderKey {
            low: 14 << 15,
            high: 0,
        }
        .srd_simple_shader_with_shadow_parallel(parameters)
        .unwrap();
        assert!(ungated.contains(54));
        assert!((55..=68).all(|position| !ungated.contains(position)));

        let gated = CeylonShaderKey {
            low: (1 << 6) | (14 << 15),
            high: 0,
        }
        .srd_simple_shader_with_shadow_parallel(parameters)
        .unwrap();
        assert!(gated.contains(54));
        assert!(!gated.contains(55));
        assert!(gated.contains(56));
        assert!(gated.contains(57));
        assert!(!gated.contains(58));
        assert!(gated.contains(59));
        assert!(!gated.contains(60));
        assert!(!gated.contains(61));
        assert!(gated.contains(62));
        assert!(gated.contains(63));
        assert!(!gated.contains(64));
        assert!(!gated.contains(65));
        assert!((66..=68).all(|position| !gated.contains(position)));
    }

    #[test]
    fn srd_renderer_packet_defaults_feed_format_14_shader_keys() {
        let mut packet = CeylonDrawPacketPresetState::srd_renderer_initial();
        assert_eq!(packet.draw_flags_00, 0x0029_e000);
        assert_eq!(packet.field_2c, 0);
        assert_eq!(packet.flags_58, 0xff);
        assert_eq!(packet.flags_60, 0x4000);
        assert_eq!(packet.flags_64, 0);

        packet.set_render_preset_id(3);
        let key = packet.srd_quad_shader_key([true, true, false]);
        assert_eq!((key.low >> 8) & 0x1f, 31);
        assert_eq!((key.low >> 13) & 3, 2);
        assert_eq!((key.low >> 15) & 0x1f, 14);
        assert_eq!((key.low >> 20) & 0x3f, 3);

        let bits_3d = key.srd_simple_shader_direct_contributions().unwrap();
        let key_3d = bits_3d.compact_key();
        assert!(!bits_3d.contains(2));
        packet.set_srd_quad_is_2d(true);
        assert_eq!(packet.flags_60 & 0x80, 0x80);
        let key_2d = packet.srd_quad_shader_key([true, true, false]);
        assert_eq!(key_2d.low ^ key.low, 1 << 3);
        let bits_2d = key_2d.srd_simple_shader_direct_contributions().unwrap();
        assert_ne!(bits_2d.compact_key(), key_3d);
        assert!(bits_2d.contains(2));
        packet.set_srd_quad_is_2d(false);
        assert_eq!(packet.flags_60 & 0x80, 0);
        assert_eq!(packet.srd_quad_shader_key([true, true, false]), key);
    }

    #[test]
    fn srimage_field_0c_replaces_only_draw_flag_bits_six_through_nine() {
        for (value, expected) in [(-1, 0), (0, 0), (1, 0x240), (2, 0), (3, 0), (4, 0), (5, 0)] {
            let mut packet = CeylonDrawPacketPresetState {
                draw_flags_00: u32::MAX,
                ..CeylonDrawPacketPresetState::default()
            };
            apply_srd_image_field_0c_shader_bits(&mut packet, value);
            assert_eq!(packet.draw_flags_00 & 0x3c0, expected);
            assert_eq!(packet.draw_flags_00 & !0x3c0, !0x3c0);
        }
    }

    #[test]
    fn shader_key_uses_encoded_preset_record_and_high_draw_flag_bits() {
        let mut input = CeylonShaderKeyInput {
            draw_flags_00: 0x143 | (6 << 10) | (1 << 17),
            field_28: 0,
            field_2c: 0,
            flags_60: 0,
            vertex_format_70: 0,
            texture_present: [false; 3],
        };
        let key = input.shader_key();
        assert_eq!(key.high, 6);
        assert_eq!((key.low >> 4) & 1, 1);
        assert_eq!((key.low >> 5) & 1, 1);
        assert_eq!((key.low >> 20) & 0x3ff, input.draw_flags_00 & 0x3ff);

        input.draw_flags_00 = 0;
        let key = input.shader_key();
        assert_eq!(key.high, 0);
        assert_eq!((key.low >> 4) & 1, 0);
        assert_eq!((key.low >> 5) & 1, 0);
    }

    #[test]
    fn blend_factor_and_operation_mappings_match_the_d3d9_backend_tables() {
        let factors = [
            D3d9BlendFactor::Zero,
            D3d9BlendFactor::One,
            D3d9BlendFactor::SourceColor,
            D3d9BlendFactor::InverseSourceColor,
            D3d9BlendFactor::SourceAlpha,
            D3d9BlendFactor::InverseSourceAlpha,
            D3d9BlendFactor::DestinationColor,
            D3d9BlendFactor::InverseDestinationColor,
            D3d9BlendFactor::DestinationAlpha,
            D3d9BlendFactor::InverseDestinationAlpha,
            D3d9BlendFactor::SourceAlphaSaturate,
            D3d9BlendFactor::BlendFactor,
            D3d9BlendFactor::InverseBlendFactor,
        ];
        for (internal, expected) in factors.into_iter().enumerate() {
            assert_eq!(
                d3d9_blend_factor_from_internal(internal as u32),
                Some(expected)
            );
        }
        assert_eq!(d3d9_blend_factor_from_internal(13), None);

        let operations = [
            D3d9BlendOperation::Add,
            D3d9BlendOperation::Subtract,
            D3d9BlendOperation::ReverseSubtract,
            D3d9BlendOperation::Minimum,
            D3d9BlendOperation::Maximum,
        ];
        for (internal, expected) in operations.into_iter().enumerate() {
            assert_eq!(
                d3d9_blend_operation_from_internal(internal as u32),
                Some(expected)
            );
        }
        assert_eq!(d3d9_blend_operation_from_internal(5), None);
    }

    #[test]
    fn render_preset_table_preserves_all_binary_records_and_clamps_like_the_accessor() {
        fn signature(preset: SrdD3d9BlendPreset) -> [u32; 9] {
            [
                u32::from(preset.alpha_blend_enabled),
                preset.source_blend as u32,
                preset.destination_blend as u32,
                preset.blend_operation as u32,
                u32::from(preset.alpha_test_enabled),
                u32::from(preset.separate_alpha_blend_enabled),
                preset.source_blend_alpha as u32,
                preset.destination_blend_alpha as u32,
                preset.blend_operation_alpha as u32,
            ]
        }

        assert_eq!(SRD_D3D9_BLEND_PRESETS.len(), 62);
        let expected_0_through_21 = [
            [0, 5, 6, 1, 0, 0, 1, 1, 1],
            [0, 5, 6, 1, 1, 0, 1, 1, 1],
            [1, 5, 6, 1, 1, 0, 1, 1, 1],
            [1, 5, 6, 1, 0, 0, 1, 1, 1],
            [1, 5, 2, 1, 0, 0, 1, 1, 1],
            [1, 5, 2, 3, 0, 0, 1, 1, 1],
            [1, 1, 3, 1, 0, 0, 1, 1, 1],
            [1, 10, 1, 1, 0, 0, 1, 1, 1],
            [1, 9, 2, 1, 0, 0, 1, 1, 1],
            [1, 1, 3, 1, 1, 0, 1, 1, 1],
            [1, 2, 1, 1, 0, 1, 1, 1, 1],
            [0, 2, 1, 1, 0, 1, 1, 1, 1],
            [1, 5, 6, 1, 1, 1, 1, 6, 1],
            [1, 5, 6, 1, 0, 1, 1, 6, 1],
            [1, 5, 2, 1, 0, 1, 1, 2, 1],
            [1, 5, 2, 3, 0, 0, 1, 1, 1],
            [1, 1, 3, 1, 0, 0, 1, 1, 1],
            [1, 10, 1, 1, 0, 0, 1, 1, 1],
            [1, 9, 6, 1, 0, 0, 1, 1, 1],
            [1, 1, 3, 1, 1, 0, 1, 1, 1],
            [0, 5, 6, 1, 0, 0, 1, 1, 1],
            [0, 5, 6, 1, 1, 0, 1, 1, 1],
        ];
        assert_eq!(
            SRD_D3D9_BLEND_PRESETS[..22]
                .iter()
                .copied()
                .map(signature)
                .collect::<Vec<_>>(),
            expected_0_through_21
        );
        assert_eq!(ceylon_d3d9_blend_preset(-7), PRESET_0);
        assert_eq!(ceylon_d3d9_blend_preset(0), PRESET_0);
        assert_eq!(ceylon_d3d9_blend_preset(3), PRESET_3);
        assert_eq!(ceylon_d3d9_blend_preset(4), PRESET_4);
        assert_eq!(ceylon_d3d9_blend_preset(5), PRESET_5);
        assert_eq!(ceylon_d3d9_blend_preset(9), PRESET_9);
        assert_eq!(ceylon_d3d9_blend_preset(12), PRESET_12);
        assert_eq!(ceylon_d3d9_blend_preset(14), PRESET_14);
        assert_eq!(ceylon_d3d9_blend_preset(18), PRESET_18);
        assert_eq!(ceylon_d3d9_blend_preset(20), PRESET_0);
        assert_eq!(ceylon_d3d9_blend_preset(21), PRESET_1);
        assert!(
            SRD_D3D9_BLEND_PRESETS[22..=60]
                .iter()
                .all(|preset| *preset == PRESET_0)
        );
        assert_eq!(ceylon_d3d9_blend_preset(61), PRESET_3);
        assert_eq!(ceylon_d3d9_blend_preset(62), PRESET_3);
        assert_eq!(ceylon_d3d9_blend_preset(i32::MAX), PRESET_3);

        let preset_12 = ceylon_d3d9_blend_preset(12);
        let preset_14 = ceylon_d3d9_blend_preset(14);
        assert!(preset_12.separate_alpha_blend_enabled);
        assert_eq!(
            preset_12.destination_blend_alpha,
            D3d9BlendFactor::InverseSourceAlpha
        );
        assert_eq!(preset_14.destination_blend_alpha, D3d9BlendFactor::One);
    }

    #[test]
    fn draw_packet_preset_setter_keeps_the_signed_request_until_binary_encoding() {
        let mut packet = CeylonDrawPacketPresetState {
            draw_flags_00: 0xa5a5_5a80,
            flags_58: 0xffff_ffff,
            flags_60: 0xffff_f7ff,
            ..CeylonDrawPacketPresetState::default()
        };
        packet.set_render_preset_id(3);
        assert_eq!(packet.draw_flags_00 & 0x3f, 3);
        assert_eq!(packet.draw_flags_00 & !0x3f, 0xa5a5_5a80);
        assert_eq!(packet.flags_60 & 0x800, 0);
        assert_eq!(packet.flags_60 & 0x20, 0x20);
        assert_eq!(packet.flags_60 & 0x40, 0);
        assert_eq!(packet.flags_58 & 0x8, 0);

        packet.set_render_preset_id(21);
        assert_eq!(packet.encoded_preset_id(), 21);
        assert_eq!(packet.table_preset_id(), 21);
        assert_eq!(packet.flags_60 & 0x20, 0);
        assert_eq!(packet.flags_60 & 0x40, 0x40);
        assert_eq!(packet.flags_58 & 0x8, 0x8);

        packet.set_render_preset_id(300);
        assert_eq!(packet.encoded_preset_id(), 44);
        assert_eq!(packet.table_preset_id(), 44);
        assert_eq!(packet.flags_60 & 0x800, 0x800);
        assert_eq!(packet.flags_60 & 0x20, 0x20);
        assert_eq!(packet.flags_60 & 0x40, 0);
        assert_eq!(packet.flags_58 & 0x8, 0x8);

        packet.set_render_preset_id(62);
        assert_eq!(packet.encoded_preset_id(), 62);
        assert_eq!(packet.table_preset_id(), 61);
        assert_eq!(packet.flags_60 & 0x20, 0x20);
        assert_eq!(packet.flags_58 & 0x8, 0);

        packet.set_render_preset_id(64);
        assert_eq!(packet.encoded_preset_id(), 0);
        assert_eq!(packet.table_preset_id(), 0);
        assert_eq!(packet.flags_60 & 0x800, 0x800);
        assert_eq!(packet.flags_60 & 0x20, 0x20);
        assert_eq!(packet.flags_58 & 0x8, 0x8);

        packet.set_render_preset_id(-1);
        assert_eq!(packet.encoded_preset_id(), 63);
        assert_eq!(packet.table_preset_id(), 61);
        assert_eq!(packet.flags_60 & 0x800, 0);
        assert_eq!(packet.flags_60 & 0x20, 0x20);
        assert_eq!(packet.flags_58 & 0x8, 0);
    }

    #[test]
    fn comparison_and_stencil_operation_tables_match_the_d3d9_backend() {
        let comparisons = [
            D3d9ComparisonFunction::Never,
            D3d9ComparisonFunction::Always,
            D3d9ComparisonFunction::Equal,
            D3d9ComparisonFunction::NotEqual,
            D3d9ComparisonFunction::Less,
            D3d9ComparisonFunction::LessEqual,
            D3d9ComparisonFunction::Greater,
            D3d9ComparisonFunction::GreaterEqual,
        ];
        for (internal, expected) in comparisons.into_iter().enumerate() {
            assert_eq!(
                d3d9_comparison_function_from_internal(internal as u32),
                Some(expected)
            );
        }
        assert_eq!(d3d9_comparison_function_from_internal(8), None);

        let stencil_operations = [
            D3d9StencilOperation::Keep,
            D3d9StencilOperation::Zero,
            D3d9StencilOperation::Replace,
            D3d9StencilOperation::IncrementSaturate,
            D3d9StencilOperation::DecrementSaturate,
            D3d9StencilOperation::Invert,
            D3d9StencilOperation::Increment,
            D3d9StencilOperation::Decrement,
        ];
        for (internal, expected) in stencil_operations.into_iter().enumerate() {
            assert_eq!(
                d3d9_stencil_operation_from_internal(internal as u32),
                Some(expected)
            );
        }
        assert_eq!(d3d9_stencil_operation_from_internal(8), None);
    }

    #[test]
    fn depth_bias_encoding_preserves_the_backend_instruction_chain() {
        assert_eq!(d3d9_depth_bias_bits(1), CEYLON_DEPTH_BIAS_SCALE_BITS);
        assert_eq!(d3d9_depth_bias_bits(0), (-0.0_f32).to_bits());

        assert_eq!(d3d9_slope_scale_depth_bias_bits(0.0), (-0.0_f32).to_bits());
        assert_eq!(d3d9_slope_scale_depth_bias_bits(-0.0), 0.0_f32.to_bits());
        let nan = f32::from_bits(2_143_290_709);
        assert_eq!(
            d3d9_slope_scale_depth_bias_bits(nan),
            nan.to_bits() ^ (-0.0_f32).to_bits()
        );
    }

    #[test]
    fn material_scissor_sources_are_selected_independently() {
        let base = CeylonMaterialScissorSource {
            flags_00: 0x20,
            rectangle_24: D3d9Rect {
                left: 1,
                top: 2,
                right: 30,
                bottom: 40,
            },
        };
        let override_source = CeylonMaterialScissorSource {
            flags_00: 0,
            rectangle_24: D3d9Rect {
                left: -5,
                top: -6,
                right: 70,
                bottom: 80,
            },
        };

        assert_eq!(
            ceylon_select_material_scissor_command(base, override_source, 0),
            CeylonScissorStateCommand {
                enabled: true,
                rectangle: base.rectangle_24,
            }
        );
        assert_eq!(
            ceylon_select_material_scissor_command(
                base,
                override_source,
                CEYLON_MATERIAL_OVERRIDE_SCISSOR_ENABLE,
            ),
            CeylonScissorStateCommand {
                enabled: false,
                rectangle: base.rectangle_24,
            }
        );
        assert_eq!(
            ceylon_select_material_scissor_command(
                base,
                override_source,
                CEYLON_MATERIAL_OVERRIDE_SCISSOR_RECTANGLE,
            ),
            CeylonScissorStateCommand {
                enabled: true,
                rectangle: override_source.rectangle_24,
            }
        );
        assert_eq!(
            ceylon_select_material_scissor_command(
                base,
                override_source,
                CEYLON_MATERIAL_OVERRIDE_SCISSOR_ENABLE
                    | CEYLON_MATERIAL_OVERRIDE_SCISSOR_RECTANGLE,
            ),
            CeylonScissorStateCommand {
                enabled: false,
                rectangle: override_source.rectangle_24,
            }
        );
    }

    #[test]
    fn scissor_command_replaces_the_exact_render_state_fields() {
        assert_eq!(
            CeylonScissorStateCommand::default(),
            CeylonScissorStateCommand {
                enabled: false,
                rectangle: D3d9Rect::default(),
            }
        );

        let mut state = CeylonRenderScissorState::default();
        assert!(!state.enabled);
        assert_eq!(state.rectangle.left, 5);

        let command = CeylonScissorStateCommand {
            enabled: true,
            rectangle: D3d9Rect {
                left: -10,
                top: 20,
                right: 300,
                bottom: 400,
            },
        };
        state.apply_command(command);
        assert_eq!(state.enabled, command.enabled);
        assert_eq!(state.rectangle, command.rectangle);
        assert_eq!(D3d9RenderState::ScissorTestEnable as u32, 174);
    }

    #[test]
    fn srimage_raw_fields_build_the_exact_alpha_and_stencil_packet_words() {
        let mut packet = CeylonDrawPacketPresetState {
            draw_flags_00: 0xffff_ffff,
            ..CeylonDrawPacketPresetState::default()
        };
        let mut counter = 0;
        apply_srd_image_alpha_stencil_packet_fields(&mut packet, 1, 3, 0, 0, &mut counter);
        assert_eq!(counter, 0xff);
        assert_eq!(packet.packed_08, 0x0808_2001);
        assert_eq!(packet.flags_0c, 0x108);
        assert_eq!(packet.draw_flags_00 & 0x1e000, 0);

        let mut state = CeylonAlphaStencilState {
            alpha_test_enabled: false,
            alpha_reference: 7,
            alpha_function_internal: 1,
            stencil_enabled: false,
            stencil_function_internal: 7,
            stencil_fail_internal: 7,
            stencil_z_fail_internal: 7,
            stencil_pass_internal: 7,
            stencil_reference: 0,
            stencil_mask: 0,
            stencil_write_mask: 0,
        };
        state.apply_draw_packet(packet);
        assert!(state.alpha_test_enabled);
        assert_eq!(state.alpha_reference, 128);
        assert_eq!(state.alpha_function(), Some(D3d9ComparisonFunction::Always));
        assert!(state.stencil_enabled);
        assert_eq!(
            state.stencil_function(),
            Some(D3d9ComparisonFunction::Always)
        );
        assert_eq!(state.stencil_fail(), Some(D3d9StencilOperation::Keep));
        assert_eq!(state.stencil_z_fail(), Some(D3d9StencilOperation::Keep));
        assert_eq!(state.stencil_pass(), Some(D3d9StencilOperation::Replace));
        assert_eq!(state.stencil_reference, 8);
        assert_eq!(state.stencil_mask, 8);
        assert_eq!(state.stencil_write_mask, 8);

        packet.draw_flags_00 = 0;
        apply_srd_image_alpha_stencil_packet_fields(&mut packet, 3, 2, 0, 1, &mut counter);
        assert_eq!(packet.packed_08, 0x0505_0002);
        assert_eq!(packet.flags_0c, 0x105);
        assert_eq!(packet.draw_flags_00 & 0x1e000, 0x1e000);

        apply_srd_image_alpha_stencil_packet_fields(&mut packet, 4, 2, 0, 1, &mut counter);
        assert_eq!(packet.packed_08 & 0xffff, 3);

        apply_srd_image_alpha_stencil_packet_fields(&mut packet, 4, 2, 1, 1, &mut counter);
        assert_eq!(packet.packed_08, 0x0101_0002);
        assert_eq!(packet.flags_0c, 0x101);

        apply_srd_image_alpha_stencil_packet_fields(&mut packet, 0, 0, 0, 0, &mut counter);
        assert_eq!(packet.packed_08, 0);
        assert_eq!(packet.flags_0c, 0);

        apply_srd_image_alpha_stencil_packet_fields(&mut packet, 1, 8, 0, 0, &mut counter);
        assert_eq!(packet.packed_08, 0x2001);
        assert_eq!(packet.flags_0c, 0x100);
    }

    #[test]
    fn draw_flags_decode_and_special_mode_update_the_binary_depth_state() {
        let initial = CeylonDepthState::from_draw_flags(0x00af_e000);
        assert!(initial.z_enabled);
        assert!(initial.z_write_enabled);
        assert_eq!(initial.z_function_internal, 5);
        assert_eq!(
            initial.z_function(),
            Some(D3d9ComparisonFunction::LessEqual)
        );

        let mut packet = CeylonDrawPacketPresetState {
            draw_flags_00: 0x00af_e003,
            field_2c: -9,
            ..CeylonDrawPacketPresetState::default()
        };
        apply_srd_special_depth_packet_fields(&mut packet, false, -1);
        assert_eq!(packet.draw_flags_00, 0x00af_e003);
        assert_eq!(packet.field_2c, -9);

        apply_srd_special_depth_packet_fields(&mut packet, true, -1);
        let disabled_write = CeylonDepthState::from_draw_flags(packet.draw_flags_00);
        assert!(disabled_write.z_enabled);
        assert!(!disabled_write.z_write_enabled);
        assert_eq!(disabled_write.z_function_internal, 5);
        assert_eq!(packet.field_2c, 4);

        apply_srd_special_depth_packet_fields(&mut packet, true, 0);
        let enabled_write = CeylonDepthState::from_draw_flags(packet.draw_flags_00);
        assert!(enabled_write.z_enabled);
        assert!(enabled_write.z_write_enabled);
        assert_eq!(enabled_write.z_function_internal, 5);
    }

    #[test]
    fn image_preset_selection_and_sequence_side_effects_match_all_binary_branches() {
        assert_eq!(select_srd_image_render_preset(0, 300, false), Some(300));
        assert_eq!(select_srd_image_render_preset(0, -1, false), Some(3));
        assert_eq!(select_srd_image_render_preset(1, -1, true), Some(4));
        assert_eq!(select_srd_image_render_preset(2, -1, true), Some(5));
        assert_eq!(select_srd_image_render_preset(3, -1, true), Some(9));
        assert_eq!(select_srd_image_render_preset(4, -1, true), None);
        assert_eq!(select_srd_image_render_preset(0x200, -1, true), Some(20));
        assert_eq!(select_srd_image_render_preset(0x400, -1, true), Some(21));
        assert_eq!(select_srd_image_render_preset(0x600, -1, true), Some(3));

        let mut packet = CeylonDrawPacketPresetState::default();
        let mut image_field = 17;
        let mut global_field = i32::MAX;
        assert!(apply_srd_image_render_preset(
            0x200,
            -1,
            true,
            &mut image_field,
            &mut global_field,
            &mut packet,
        ));
        assert_eq!(packet.encoded_preset_id(), 20);
        assert_eq!(global_field, i32::MIN);
        assert_eq!(image_field, i32::MIN);

        assert!(apply_srd_image_render_preset(
            0x400,
            -1,
            true,
            &mut image_field,
            &mut global_field,
            &mut packet,
        ));
        assert_eq!(packet.encoded_preset_id(), 21);
        assert_eq!(global_field, i32::MIN + 1);
        assert_eq!(image_field, i32::MIN + 1);

        assert!(apply_srd_image_render_preset(
            0x600,
            -1,
            true,
            &mut image_field,
            &mut global_field,
            &mut packet,
        ));
        assert_eq!(packet.encoded_preset_id(), 3);
        assert_eq!(image_field, -1);
        assert_eq!(global_field, i32::MIN + 1);

        image_field = 25;
        assert!(apply_srd_image_render_preset(
            1,
            -1,
            true,
            &mut image_field,
            &mut global_field,
            &mut packet,
        ));
        assert_eq!(packet.encoded_preset_id(), 4);
        assert_eq!(image_field, 25);

        assert!(apply_srd_image_render_preset(
            0,
            300,
            true,
            &mut image_field,
            &mut global_field,
            &mut packet,
        ));
        assert_eq!(packet.encoded_preset_id(), 44);
        assert_eq!(image_field, 25);

        let unchanged = (packet, image_field, global_field);
        assert!(!apply_srd_image_render_preset(
            4,
            -1,
            true,
            &mut image_field,
            &mut global_field,
            &mut packet,
        ));
        assert_eq!((packet, image_field, global_field), unchanged);
    }

    #[test]
    fn vertex_layout_matches_the_binary_format_14_writes() {
        assert_eq!(std::mem::size_of::<SrdRenderVertex>(), 36);
        assert_eq!(std::mem::offset_of!(SrdRenderVertex, position), 0);
        assert_eq!(std::mem::offset_of!(SrdRenderVertex, primary_color), 12);
        assert_eq!(std::mem::offset_of!(SrdRenderVertex, secondary_color), 16);
        assert_eq!(
            std::mem::offset_of!(SrdRenderVertex, texture_coordinates),
            20
        );
        assert_eq!(D3d9PrimitiveType::TriangleStrip as u32, 5);
        assert_eq!(SrdQuadDraw::PRIMITIVE_COUNT, 2);
        assert_eq!(std::mem::size_of::<D3d9VertexElement>(), 8);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[0].offset, 0);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[1].offset, 12);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[2].offset, 16);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[3].offset, 20);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[4].offset, 28);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[5].stream, 0xff);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[5].declaration_type, 17);
        assert_eq!(FENNEL_D3D9_VERTEX_DECLARATION.len(), 5);
        assert_eq!(FENNEL_D3D9_VERTEX_DECLARATION[0].offset, 0);
        assert_eq!(FENNEL_D3D9_VERTEX_DECLARATION[1].offset, 12);
        assert_eq!(FENNEL_D3D9_VERTEX_DECLARATION[2].offset, 16);
        assert_eq!(FENNEL_D3D9_VERTEX_DECLARATION[3].offset, 20);
        assert_eq!(FENNEL_D3D9_VERTEX_DECLARATION[4].stream, 0xff);
        assert_eq!(std::mem::size_of::<crate::fennel::FennelRenderVertex>(), 28);
    }
}
