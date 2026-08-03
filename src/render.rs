#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9PrimitiveType {
    TriangleStrip = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9RenderState {
    AlphaTestEnable = 15,
    SourceBlend = 19,
    DestinationBlend = 20,
    AlphaBlendEnable = 27,
    BlendOperation = 171,
    SeparateAlphaBlendEnable = 206,
    SourceBlendAlpha = 207,
    DestinationBlendAlpha = 208,
    BlendOperationAlpha = 209,
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
    pub flags_58: u32,
    pub flags_60: u32,
}

impl CeylonDrawPacketPresetState {
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
    }
}
