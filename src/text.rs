use std::fmt;

use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextError(pub String);

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TextError {}

#[derive(Debug, Clone, PartialEq)]
pub struct TextDefinition {
    pub field_78: Option<u32>,
    pub font_index: Option<i32>,
    pub text: Vec<u8>,
    pub field_36: Option<[f32; 2]>,
    pub field_7b: Option<[i16; 4]>,
    pub field_7c: Option<i16>,
    pub field_41: Option<i16>,
}

impl TextDefinition {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, TextError> {
        if !block.is_tag(b"TEXT") {
            return Err(TextError("block is not TEXT".into()));
        }
        Ok(Self {
            field_78: optional_unsigned(file, block, 0x78)?,
            font_index: optional_signed(file, block, 0x79)?,
            text: block
                .last_property(0x7a)
                .map(|property| {
                    property
                        .string_bytes(file)
                        .ok_or_else(|| TextError("TEXT 0x7a is not a string".into()))
                        .map(|bytes| bytes.iter().copied().take(2047).collect())
                })
                .transpose()?
                .unwrap_or_default(),
            field_36: optional_f32_array::<2>(file, block, 0x36)?,
            field_7b: optional_i16_array::<4>(file, block, 0x7b)?,
            field_7c: optional_i16(file, block, 0x7c)?,
            field_41: optional_i16(file, block, 0x41)?,
        })
    }

    /// Returns the exact 3-by-3 Fennel alignment code written to
    /// `TextBoxObject+0x2D0` by `sub_AC6F50 -> sub_AC6BF0`.
    ///
    /// Horizontal flags are `0`, `0x04`, or `0x08`; vertical flags are `0`,
    /// `0x10`, or `0x20`. The game returns zero when either masked group has
    /// an unsupported combination (for example both bits set).
    pub fn fennel_alignment_code(&self) -> Option<u32> {
        self.field_78.map(fennel_alignment_code_from_text_flags)
    }
}

/// Exact `sub_AC6BF0` mapping used by the RFZ/Fennel TextBox path.
pub const fn fennel_alignment_code_from_text_flags(flags: u32) -> u32 {
    match (flags & 0x0C, flags & 0x30) {
        (horizontal @ (0 | 4 | 8), vertical @ (0 | 0x10 | 0x20)) => {
            (vertical / 0x10) * 3 + horizontal / 4
        }
        _ => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontCharacterMapping {
    pub code: u32,
    pub values_4a: [i16; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontDefinition {
    pub name: Vec<u8>,
    pub flags_70: Option<u32>,
    pub field_71: Option<u16>,
    pub characters: Vec<FontCharacterMapping>,
}

impl FontDefinition {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, TextError> {
        if !block.is_tag(b"FONT") {
            return Err(TextError("block is not FONT".into()));
        }
        let name = block
            .last_property(0x03)
            .map(|property| {
                property
                    .string_bytes(file)
                    .ok_or_else(|| TextError("FONT 0x03 is not a string".into()))
                    .map(|bytes| bytes.iter().copied().take(64).collect())
            })
            .transpose()?
            .unwrap_or_default();
        let flags_70 = optional_unsigned(file, block, 0x70)?;
        let capacity = if flags_70.unwrap_or(0) & 7 != 0 {
            0x1_0000
        } else {
            0x100
        };
        let field_71 = optional_unsigned(file, block, 0x71)?
            .map(|value| {
                u16::try_from(value).map_err(|_| TextError("FONT 0x71 exceeds u16".into()))
            })
            .transpose()?;
        let mut characters = Vec::new();
        for child in block.children.iter().filter(|child| child.is_tag(b"CHAR")) {
            let Some(code) = optional_unsigned(file, child, 0x72)? else {
                continue;
            };
            if code >= capacity {
                return Err(TextError(format!(
                    "CHAR code {code} is outside FONT lookup capacity {capacity}"
                )));
            }
            for property in child.properties_with_code(0x4a) {
                let first = signed_at(file, property, 0, "CHAR 0x4a[0]")?;
                let second = signed_at(file, property, 1, "CHAR 0x4a[1]")?;
                characters.push(FontCharacterMapping {
                    code,
                    values_4a: [
                        i16::try_from(first)
                            .map_err(|_| TextError("CHAR 0x4a[0] exceeds i16".into()))?,
                        i16::try_from(second)
                            .map_err(|_| TextError("CHAR 0x4a[1] exceeds i16".into()))?,
                    ],
                });
            }
        }
        Ok(Self {
            name,
            flags_70,
            field_71,
            characters,
        })
    }

    pub fn character(&self, code: u32) -> Option<[i16; 2]> {
        self.characters
            .iter()
            .rev()
            .find(|entry| entry.code == code)
            .map(|entry| entry.values_4a)
    }
}

fn optional_unsigned(file: &SrdFile, block: &Block, code: u8) -> Result<Option<u32>, TextError> {
    block
        .last_property(code)
        .map(|property| {
            property
                .read_unsigned_scalar(file)
                .ok_or_else(|| TextError(format!("invalid property {code:#04x}")))
        })
        .transpose()
}

fn optional_signed(file: &SrdFile, block: &Block, code: u8) -> Result<Option<i32>, TextError> {
    block
        .last_property(code)
        .map(|property| signed_at(file, property, 0, &format!("property {code:#04x}")))
        .transpose()
}

fn optional_i16(file: &SrdFile, block: &Block, code: u8) -> Result<Option<i16>, TextError> {
    optional_signed(file, block, code)?
        .map(|value| {
            i16::try_from(value).map_err(|_| TextError(format!("property {code:#04x} exceeds i16")))
        })
        .transpose()
}

fn optional_f32_array<const N: usize>(
    file: &SrdFile,
    block: &Block,
    code: u8,
) -> Result<Option<[f32; N]>, TextError> {
    block
        .last_property(code)
        .map(|property| {
            let mut output = [0.0; N];
            for (index, destination) in output.iter_mut().enumerate() {
                *destination = property
                    .read_scalar_as_f32_at(file, index)
                    .ok_or_else(|| TextError(format!("invalid property {code:#04x}[{index}]")))?;
            }
            Ok(output)
        })
        .transpose()
}

fn optional_i16_array<const N: usize>(
    file: &SrdFile,
    block: &Block,
    code: u8,
) -> Result<Option<[i16; N]>, TextError> {
    block
        .last_property(code)
        .map(|property| {
            let mut output = [0; N];
            for (index, destination) in output.iter_mut().enumerate() {
                *destination = i16::try_from(signed_at(
                    file,
                    property,
                    index,
                    &format!("property {code:#04x}[{index}]"),
                )?)
                .map_err(|_| TextError(format!("property {code:#04x}[{index}] exceeds i16")))?;
            }
            Ok(output)
        })
        .transpose()
}

fn signed_at(
    file: &SrdFile,
    property: &Property,
    index: usize,
    label: &str,
) -> Result<i32, TextError> {
    property
        .read_signed_scalar_at(file, index)
        .ok_or_else(|| TextError(format!("invalid {label}")))
}

#[cfg(test)]
mod tests {
    use super::fennel_alignment_code_from_text_flags;

    #[test]
    fn fennel_alignment_matches_all_nine_game_codes() {
        for (flags, expected) in [
            (0x00, 0),
            (0x04, 1),
            (0x08, 2),
            (0x10, 3),
            (0x14, 4),
            (0x18, 5),
            (0x20, 6),
            (0x24, 7),
            (0x28, 8),
        ] {
            assert_eq!(fennel_alignment_code_from_text_flags(flags), expected);
        }
    }

    #[test]
    fn fennel_alignment_preserves_the_game_invalid_combination_fallback() {
        assert_eq!(fennel_alignment_code_from_text_flags(0x0C), 0);
        assert_eq!(fennel_alignment_code_from_text_flags(0x30), 0);
        assert_eq!(fennel_alignment_code_from_text_flags(0x3C), 0);
        assert_eq!(fennel_alignment_code_from_text_flags(0xFFFF_FFFF), 0);
    }
}
