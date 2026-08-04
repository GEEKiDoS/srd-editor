use crate::ruhuna::RuhunaRuntimeGlyphRecord;

/// Host-independent form of the 116-byte glyph item built by
/// `font::TextBoxObject::setTextByWideString` (`sub_7C90A0`). Pointer fields
/// remain opaque 32-bit tokens so the layout is identical on x86 and x64
/// editor builds.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FennelGlyphLayoutRecord {
    pub kind: i32,
    pub glyph_token: u32,
    pub texture_token: u32,
    pub field_0c: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub field_20: f32,
    pub field_24: f32,
    pub field_28: f32,
    pub field_2c: f32,
    pub field_30: f32,
    pub colors: [u32; 4],
    pub scale_x: f32,
    pub scale_y: f32,
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub uv2: [f32; 2],
    pub uv3: [f32; 2],
    pub field_6c: u32,
    pub field_70: u32,
}

impl Default for FennelGlyphLayoutRecord {
    fn default() -> Self {
        Self {
            kind: 0,
            glyph_token: 0,
            texture_token: 0,
            field_0c: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            field_20: 0.0,
            field_24: 0.0,
            field_28: 0.0,
            field_2c: 0.0,
            field_30: 0.0,
            colors: [0; 4],
            scale_x: 0.0,
            scale_y: 0.0,
            uv0: [0.0; 2],
            uv1: [0.0; 2],
            uv2: [0.0; 2],
            uv3: [0.0; 2],
            field_6c: 0,
            field_70: 0,
        }
    }
}

/// Values supplied by the Fennel token iterator immediately before
/// `sub_7C90A0` converts a 128-byte runtime glyph into a 116-byte layout item.
/// Fields whose semantic names are not yet proven retain their binary offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FennelGlyphPlacementInput {
    pub glyph_token: u32,
    pub field_0c: u32,
    pub x: i32,
    pub y: i32,
    pub field_20: i32,
    pub colors: [u32; 4],
}

impl FennelGlyphLayoutRecord {
    /// Reproduces the normal-glyph block at `0x7C92CA..0x7C940C`.
    ///
    /// `glyph.enabled` is deliberately kept as the binary field name for now;
    /// this routine proves only that it is added on both sides of the glyph box.
    pub fn from_runtime_glyph(
        glyph: &RuhunaRuntimeGlyphRecord,
        input: FennelGlyphPlacementInput,
    ) -> Self {
        let border = (glyph.enabled as i32).wrapping_mul(2);
        let (kind, width, height) = if (glyph.width as i32) <= 0 {
            (
                -2,
                input.field_20.wrapping_add(glyph.advance_x as i32) as f32,
                input.field_20.wrapping_add(glyph.line_height as i32) as f32,
            )
        } else {
            (
                0,
                (glyph.width as i32).wrapping_add(border) as f32,
                (glyph.height as i32).wrapping_add(border) as f32,
            )
        };

        Self {
            kind,
            glyph_token: input.glyph_token,
            texture_token: glyph.texture_token,
            field_0c: input.field_0c,
            x: input.x as f32,
            y: input.y as f32,
            width,
            height,
            field_20: input.field_20 as f32,
            colors: input.colors,
            scale_x: 1.0,
            scale_y: 1.0,
            uv0: glyph.uv0,
            uv1: glyph.uv1,
            uv2: glyph.uv2,
            uv3: glyph.uv3,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> FennelGlyphPlacementInput {
        FennelGlyphPlacementInput {
            glyph_token: 0x1122_3344,
            field_0c: 0x5566_7788,
            x: -12,
            y: 34,
            field_20: 7,
            colors: [1, 2, 3, 4],
        }
    }

    #[test]
    fn layout_record_matches_binary_offsets() {
        assert_eq!(std::mem::size_of::<FennelGlyphLayoutRecord>(), 116);
        assert_eq!(std::mem::offset_of!(FennelGlyphLayoutRecord, kind), 0);
        assert_eq!(
            std::mem::offset_of!(FennelGlyphLayoutRecord, texture_token),
            8
        );
        assert_eq!(std::mem::offset_of!(FennelGlyphLayoutRecord, x), 16);
        assert_eq!(std::mem::offset_of!(FennelGlyphLayoutRecord, colors), 52);
        assert_eq!(std::mem::offset_of!(FennelGlyphLayoutRecord, uv0), 76);
        assert_eq!(std::mem::offset_of!(FennelGlyphLayoutRecord, field_70), 112);
    }

    #[test]
    fn converts_normal_runtime_glyph_exactly() {
        let glyph = RuhunaRuntimeGlyphRecord {
            texture_token: 9,
            enabled: 1,
            width: 13,
            height: 17,
            uv0: [0.1, 0.2],
            uv1: [0.3, 0.4],
            uv2: [0.5, 0.6],
            uv3: [0.7, 0.8],
            ..Default::default()
        };
        let record = FennelGlyphLayoutRecord::from_runtime_glyph(&glyph, input());

        assert_eq!(record.kind, 0);
        assert_eq!(record.glyph_token, 0x1122_3344);
        assert_eq!(record.texture_token, 9);
        assert_eq!(record.field_0c, 0x5566_7788);
        assert_eq!(record.x, -12.0);
        assert_eq!(record.y, 34.0);
        assert_eq!(record.width, 15.0);
        assert_eq!(record.height, 19.0);
        assert_eq!(record.field_20, 7.0);
        assert_eq!(record.colors, [1, 2, 3, 4]);
        assert_eq!(record.scale_x, 1.0);
        assert_eq!(record.scale_y, 1.0);
        assert_eq!(record.uv0, [0.1, 0.2]);
        assert_eq!(record.uv3, [0.7, 0.8]);
        assert_eq!(record.field_6c, 0);
        assert_eq!(record.field_70, 0);
    }

    #[test]
    fn converts_zero_width_runtime_glyph_to_special_kind() {
        let glyph = RuhunaRuntimeGlyphRecord {
            width: 0,
            line_height: 18,
            advance_x: 6,
            ..Default::default()
        };
        let record = FennelGlyphLayoutRecord::from_runtime_glyph(&glyph, input());

        assert_eq!(record.kind, -2);
        assert_eq!(record.width, 13.0);
        assert_eq!(record.height, 25.0);
    }
}
