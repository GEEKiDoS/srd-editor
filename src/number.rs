use std::fmt;

use crate::csli::CrefEntry;
use crate::image::{ImageDefinition, ImageReferenceChannel};
use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberError(pub String);

impl fmt::Display for NumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NumberError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberSpecialGlyphs {
    pub plus: i16,
    pub minus: i16,
    pub comma: i16,
    pub decimal_point: i16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberDefinition {
    pub flags: u32,
    pub width: f32,
    pub height: f32,
    pub custom_origin: [f32; 2],
    pub origin_mode: u8,
    pub vertex_colors: [[u8; 4]; 4],
    pub cref_count: u16,
    pub crefs: Vec<CrefEntry>,
    pub field_4c: u16,
    pub cre1_count: u16,
    pub cre1s: Vec<CrefEntry>,
    pub format_flags: u32,
    pub field_78: u32,
    pub initial_integer: i32,
    pub initial_fraction: f32,
    pub fields_83_8b: [i16; 9],
    pub field_8c: [f32; 2],
    pub fields_8d_94: [i16; 8],
    pub node_index: i32,
}

impl NumberDefinition {
    pub const INITIAL_COORDINATE_OFFSET_SCALE: f32 = 0.0;

    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, NumberError> {
        if !block.is_tag(b"CNUM") {
            return Err(NumberError("block is not CNUM".into()));
        }

        let mut result = Self {
            flags: 0,
            width: 128.0,
            height: 128.0,
            custom_origin: [0.0, 0.0],
            origin_mode: 0,
            vertex_colors: [[0xff; 4]; 4],
            cref_count: 0,
            crefs: Vec::new(),
            field_4c: 0,
            cre1_count: 0,
            cre1s: Vec::new(),
            format_flags: 0,
            field_78: 0,
            initial_integer: 0,
            initial_fraction: 0.0,
            fields_83_8b: [0; 9],
            field_8c: [0.0; 2],
            fields_8d_94: [0; 8],
            node_index: -1,
        };
        let mut color_index = 0usize;
        for property in &block.properties {
            match property.code {
                0x40 => result.width = float_scalar(file, property, "CNUM 0x40")?,
                0x41 => result.height = float_scalar(file, property, "CNUM 0x41")?,
                0x42 => result.custom_origin[0] = float_scalar(file, property, "CNUM 0x42")?,
                0x43 => result.custom_origin[1] = float_scalar(file, property, "CNUM 0x43")?,
                0x44 => {
                    let destination =
                        result.vertex_colors.get_mut(color_index).ok_or_else(|| {
                            NumberError("CNUM has more than four 0x44 properties".into())
                        })?;
                    *destination = reordered_four_bytes(file, property, "CNUM 0x44")?;
                    color_index += 1;
                }
                0x45 => result.cref_count = unsigned_scalar(file, property, "CNUM 0x45")? as u16,
                0x49 => result.flags = unsigned_scalar(file, property, "CNUM 0x49")?,
                0x4b => result.origin_mode = unsigned_scalar(file, property, "CNUM 0x4b")? as u8,
                0x4c => result.field_4c = unsigned_scalar(file, property, "CNUM 0x4c")? as u16,
                0x4d => result.cre1_count = unsigned_scalar(file, property, "CNUM 0x4d")? as u16,
                0x51 => result.node_index = unsigned_scalar(file, property, "CNUM 0x51")? as i32,
                0x78 => result.field_78 = unsigned_scalar(file, property, "CNUM 0x78")?,
                0x80 => result.format_flags = unsigned_scalar(file, property, "CNUM 0x80")?,
                0x81 => result.initial_integer = signed_scalar(file, property, "CNUM 0x81")?,
                0x82 => result.initial_fraction = float_scalar(file, property, "CNUM 0x82")?,
                0x83..=0x8b => {
                    result.fields_83_8b[usize::from(property.code - 0x83)] =
                        signed_scalar(file, property, "CNUM 0x83..0x8b")? as i16
                }
                0x8c => {
                    for (index, value) in result.field_8c.iter_mut().enumerate() {
                        *value = property.read_scalar_as_f32_at(file, index).ok_or_else(|| {
                            NumberError(format!("invalid CNUM 0x8c value {index}"))
                        })?;
                    }
                }
                0x8d..=0x94 => {
                    result.fields_8d_94[usize::from(property.code - 0x8d)] =
                        signed_scalar(file, property, "CNUM 0x8d..0x94")? as i16
                }
                _ => {}
            }
        }

        for child in &block.children {
            if child.is_tag(b"CREF") && result.cref_count != 0 {
                result.crefs = parse_reference_table(file, child, result.cref_count, "CREF")?;
            } else if child.is_tag(b"CRE1") && result.cre1_count != 0 {
                result.cre1s = parse_reference_table(file, child, result.cre1_count, "CRE1")?;
            }
        }
        Ok(result)
    }

    pub fn initial_value(&self) -> f64 {
        f64::from(self.initial_integer) + f64::from(self.initial_fraction)
    }

    pub fn fraction_digit_count(&self) -> i16 {
        self.fields_83_8b[6]
    }

    pub fn fraction_padding_character(&self) -> i16 {
        self.fields_83_8b[5]
    }

    pub fn value_animation_mode(&self) -> i16 {
        self.fields_8d_94[3]
    }

    pub fn special_glyphs(&self) -> NumberSpecialGlyphs {
        NumberSpecialGlyphs {
            plus: self.fields_8d_94[4],
            minus: self.fields_8d_94[5],
            comma: self.fields_8d_94[6],
            decimal_point: self.fields_8d_94[7],
        }
    }

    pub fn glyph_index_for_ascii(&self, byte: u8) -> Option<i16> {
        match byte {
            b'0'..=b'9' => Some(i16::from(byte - b'0')),
            b'+' => Some(self.special_glyphs().plus),
            b'-' => Some(self.special_glyphs().minus),
            b',' => Some(self.special_glyphs().comma),
            b'.' => Some(self.special_glyphs().decimal_point),
            _ => None,
        }
    }

    pub fn image_base(&self) -> ImageDefinition {
        ImageDefinition {
            flags: self.flags,
            width: self.width,
            height: self.height,
            custom_origin: self.custom_origin,
            origin_mode: self.origin_mode,
            vertex_colors: self.vertex_colors,
            cref_index: 0,
            cref_count: self.cref_count,
            crefs: self.crefs.clone(),
            field_4c: self.field_4c,
            cre1_index: 0,
            cre1_count: self.cre1_count,
            cre1s: self.cre1s.clone(),
            coordinate_offsets: [
                [f32::from_bits(self.initial_integer as u32), 0.0],
                [self.initial_fraction, 0.0],
            ],
            field_a1: 0,
            node_index: self.node_index,
            has_text_child: false,
        }
    }

    pub fn initial_reference_index(&self, _channel: ImageReferenceChannel) -> i16 {
        0
    }
}

fn parse_reference_table(
    file: &SrdFile,
    block: &Block,
    declared_count: u16,
    label: &str,
) -> Result<Vec<CrefEntry>, NumberError> {
    let mut entries = vec![
        CrefEntry {
            image_index: 0,
            rectangle_index: 0,
        };
        usize::from(declared_count)
    ];
    for (index, property) in block.properties_with_code(0x4a).enumerate() {
        let destination = entries.get_mut(index).ok_or_else(|| {
            NumberError(format!(
                "CNUM declares {declared_count} {label} records but the child has more"
            ))
        })?;
        destination.image_index = property
            .read_signed_scalar_at(file, 0)
            .ok_or_else(|| NumberError(format!("invalid {label} 0x4a image index")))?
            as i16;
        destination.rectangle_index = property
            .read_signed_scalar_at(file, 1)
            .ok_or_else(|| NumberError(format!("invalid {label} 0x4a rectangle index")))?
            as i16;
    }
    Ok(entries)
}

fn unsigned_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<u32, NumberError> {
    property
        .read_unsigned_scalar(file)
        .ok_or_else(|| NumberError(format!("invalid {label}")))
}

fn signed_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<i32, NumberError> {
    property
        .read_signed_scalar(file)
        .ok_or_else(|| NumberError(format!("invalid {label}")))
}

fn float_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<f32, NumberError> {
    property
        .read_scalar_as_f32(file)
        .ok_or_else(|| NumberError(format!("invalid {label}")))
}

fn reordered_four_bytes(
    file: &SrdFile,
    property: &Property,
    label: &str,
) -> Result<[u8; 4], NumberError> {
    let bytes = property.value_bytes(file);
    if bytes.len() < 4 {
        return Err(NumberError(format!("{label} has fewer than four bytes")));
    }
    Ok([bytes[1], bytes[2], bytes[3], bytes[0]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> NumberDefinition {
        NumberDefinition {
            flags: 0,
            width: 128.0,
            height: 128.0,
            custom_origin: [0.0; 2],
            origin_mode: 0,
            vertex_colors: [[0xff; 4]; 4],
            cref_count: 1,
            crefs: vec![CrefEntry {
                image_index: 0,
                rectangle_index: 0,
            }],
            field_4c: 0,
            cre1_count: 1,
            cre1s: vec![CrefEntry {
                image_index: 0,
                rectangle_index: 0,
            }],
            format_flags: 0,
            field_78: 0,
            initial_integer: -12,
            initial_fraction: -0.5,
            fields_83_8b: [0, 0, 0, 0, 0, b'0' as i16, 3, 0, 0],
            field_8c: [1.0, 1.0],
            fields_8d_94: [0, 0, 0, 6, 10, 11, 12, 13],
            node_index: 0,
        }
    }

    #[test]
    fn value_and_special_glyph_mapping_match_runtime_inputs() {
        let definition = definition();
        assert_eq!(definition.initial_value(), -12.5);
        assert_eq!(definition.fraction_digit_count(), 3);
        assert_eq!(definition.fraction_padding_character(), b'0' as i16);
        assert_eq!(definition.value_animation_mode(), 6);
        assert_eq!(definition.glyph_index_for_ascii(b'7'), Some(7));
        assert_eq!(definition.glyph_index_for_ascii(b'+'), Some(10));
        assert_eq!(definition.glyph_index_for_ascii(b'-'), Some(11));
        assert_eq!(definition.glyph_index_for_ascii(b','), Some(12));
        assert_eq!(definition.glyph_index_for_ascii(b'.'), Some(13));
        assert_eq!(definition.glyph_index_for_ascii(b'x'), None);
    }

    #[test]
    fn number_image_base_uses_both_tables_with_zero_initial_selectors() {
        let definition = definition();
        let base = definition.image_base();
        assert_eq!(base.cref_index, 0);
        assert_eq!(base.cre1_index, 0);
        assert_eq!(base.crefs, definition.crefs);
        assert_eq!(base.cre1s, definition.cre1s);
        assert_eq!(
            base.coordinate_offsets[0][0].to_bits(),
            definition.initial_integer as u32
        );
        assert_eq!(base.coordinate_offsets[1][0], -0.5);
        assert_eq!(NumberDefinition::INITIAL_COORDINATE_OFFSET_SCALE, 0.0);
    }
}
