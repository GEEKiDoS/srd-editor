use std::fmt;

use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeError(pub String);

impl fmt::Display for AttributeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AttributeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtParamData {
    pub render_preset_override: i32,
    pub layer_level: u8,
    pub layer: u8,
    pub field_06: u16,
    pub flags: u32,
}

impl Default for ExtParamData {
    fn default() -> Self {
        Self {
            render_preset_override: -1,
            layer_level: 0x80,
            layer: 5,
            field_06: 0,
            flags: 1,
        }
    }
}

impl ExtParamData {
    pub const LAYER_KIND: u32 = 1;
    pub const ENABLE_KIND: u32 = 2;
    pub const ENABLE_LAYER: u32 = 4;
    pub const ENABLE_LEVEL: u32 = 8;

    /// Reproduces the comma-token parser in `sub_AA4890` for values that fit
    /// the 32-bit decimal domain accepted by the local SRD corpus.
    pub fn parse(value: &[u8]) -> Self {
        let mut result = Self::default();
        for token in value
            .split(|byte| *byte == b',')
            .filter(|token| !token.is_empty())
        {
            if token.starts_with(b"blendMode") {
                let value = c_atoi(token.get(10..).unwrap_or_default());
                result.render_preset_override = if value <= 0 {
                    -1
                } else {
                    value.wrapping_add(33)
                };
            } else if token.starts_with(b"layerLevel") {
                result.layer_level =
                    (c_atoi(token.get(11..).unwrap_or_default()) as u8).wrapping_sub(0x80);
            } else if token.starts_with(b"layerKind") {
                set_flag(
                    &mut result.flags,
                    Self::LAYER_KIND,
                    c_atoi(token.get(10..).unwrap_or_default()) == 0,
                );
            } else if token.starts_with(b"layer") {
                result.layer = c_atoi(token.get(6..).unwrap_or_default()) as u8;
            } else if token.starts_with(b"enableLayer") {
                set_flag(
                    &mut result.flags,
                    Self::ENABLE_LAYER,
                    c_atoi(token.get(12..).unwrap_or_default()) != 0,
                );
            } else if token.starts_with(b"enableKind") {
                set_flag(
                    &mut result.flags,
                    Self::ENABLE_KIND,
                    c_atoi(token.get(11..).unwrap_or_default()) != 0,
                );
            } else if token.starts_with(b"enableLevel") {
                set_flag(
                    &mut result.flags,
                    Self::ENABLE_LEVEL,
                    c_atoi(token.get(12..).unwrap_or_default()) != 0,
                );
            }
        }
        result
    }

    /// Applies the three enable-controlled replacements performed by
    /// `srd_update_cast_tree` before it stores the CAST ordering key.
    pub fn compose_layer_key(self, inherited: u32) -> u32 {
        let mut result = inherited;
        if self.flags & Self::ENABLE_KIND != 0 {
            let kind = (self.flags & Self::LAYER_KIND) << 15;
            result = (result & !0x8000) | kind;
        }
        if self.flags & Self::ENABLE_LAYER != 0 {
            let layer = ((i32::from(self.layer as i8) << 8) as u32) & 0x7f00;
            result = (result & !0x7f00) | layer;
        }
        if self.flags & Self::ENABLE_LEVEL != 0 {
            result = (result & !0xff) | u32::from(self.layer_level);
        }
        result
    }
}

/// SrTextCast-specific state parsed from a CATR string record named exactly
/// `FontParamData` by `sub_AD9BF0 -> sub_AB8720`.
///
/// The field order follows the extension object at `SrTextCast+0x2F4`. This is
/// not the globally converted `ExtParamData` runtime type: the text cast walks
/// the original 72-byte CATR records in order and applies every matching
/// string to its already-constructed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontParamData {
    pub vertical: bool,
    pub prohibition: bool,
    pub word_wrap: bool,
    pub monospaced: bool,
    pub display_shadow: bool,
    /// Extension `+0x08`, read as text state `+0x108` and SrTextCast
    /// `+0x2FC`. Both `autoScalingHeight` and `noWrapPutMode` write it in
    /// source-token order.
    pub no_wrap_put_mode: u32,
    pub shadow_x: i32,
    pub shadow_y: i32,
    pub point_x: i32,
    pub point_y: i32,
    pub outline: i32,
    pub italic: i32,
    pub bold: i32,
    pub face_id: i32,
    pub scroll_speed: i32,
    pub scroll_wait: i32,
    pub field_34: i32,
    pub shadow_color: u32,
    pub outline_color: u32,
}

impl Default for FontParamData {
    fn default() -> Self {
        Self {
            vertical: false,
            prohibition: false,
            word_wrap: false,
            monospaced: false,
            display_shadow: false,
            no_wrap_put_mode: 0,
            shadow_x: 0,
            shadow_y: 0,
            point_x: 32,
            point_y: 32,
            outline: 0,
            italic: 0,
            bold: 0,
            face_id: 0,
            scroll_speed: 40,
            scroll_wait: 2,
            field_34: 2,
            shadow_color: 0,
            outline_color: 0,
        }
    }
}

impl FontParamData {
    pub fn parse(value: &[u8]) -> Self {
        let mut result = Self::default();
        result.apply(value);
        result
    }

    /// Applies one comma-separated `FontParamData` string. The binary splits
    /// every token on `#`, requires at least two fields, compares names and
    /// `True` case-sensitively, and ignores any fields after the value.
    pub fn apply(&mut self, value: &[u8]) {
        for token in value.split(|byte| *byte == b',') {
            let mut fields = token.split(|byte| *byte == b'#');
            let Some(name) = fields.next() else {
                continue;
            };
            let Some(value) = fields.next() else {
                continue;
            };
            match name {
                b"vertical" => self.vertical = value == b"True",
                b"prohibition" => self.prohibition = value == b"True",
                b"wordWrap" => self.word_wrap = value == b"True",
                b"monospaced" => self.monospaced = value == b"True",
                b"autoScalingHeight" => self.no_wrap_put_mode = u32::from(value == b"True"),
                b"noWrapPutMode" => self.no_wrap_put_mode = c_atoi(value).clamp(0, 6) as u32,
                b"scrollSpeed" => self.scroll_speed = c_atoi(value),
                b"scrollWait" => self.scroll_wait = c_atoi(value),
                // The misspelling is present in the game string table.
                b"diplayShadow" => self.display_shadow = value == b"True",
                b"shadowX" => self.shadow_x = c_atoi(value),
                b"shadowY" => self.shadow_y = c_atoi(value),
                b"pointX" => self.point_x = c_atoi(value),
                b"pointY" => self.point_y = c_atoi(value),
                b"outline" => self.outline = c_atoi(value),
                b"italic" => self.italic = c_atoi(value),
                b"bold" => self.bold = c_atoi(value),
                b"faceId" => self.face_id = c_atoi(value),
                b"shadowColor" => self.shadow_color = c_atoi(value) as u32,
                b"outlineColor" => self.outline_color = c_atoi(value) as u32,
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CastAttributeValue {
    Signed(i32),
    String(Vec<u8>),
    Float(f32),
    ExtParam {
        source: Vec<u8>,
        parsed: ExtParamData,
    },
    Unsupported {
        type_code: u8,
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CastAttribute {
    pub name: Vec<u8>,
    pub source_type_code: u8,
    pub value: CastAttributeValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CastAttributeList {
    pub node_index: Option<i32>,
    pub declared_count: u32,
    pub attributes: Vec<CastAttribute>,
}

impl CastAttributeList {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, AttributeError> {
        if !block.is_tag(b"CATR") {
            return Err(AttributeError("block is not CATR".into()));
        }

        let mut node_index = None;
        let mut declared_count = 0;
        let mut record_properties = &[][..];
        for (index, property) in block.properties.iter().enumerate() {
            if property.code == 0x51 {
                node_index = property
                    .read_unsigned_scalar(file)
                    .map(|value| value as i32);
            }
            if property.code == 0x0e {
                declared_count = property.read_unsigned_scalar(file).ok_or_else(|| {
                    AttributeError(format!(
                        "CATR at {:#x} has invalid 0x0E count",
                        block.offset
                    ))
                })?;
                record_properties = &block.properties[index + 1..];
                break;
            }
        }

        let capacity = usize::try_from(declared_count)
            .map_err(|_| AttributeError("CATR count does not fit usize".into()))?;
        let mut attributes = Vec::with_capacity(capacity);
        let mut name = Vec::new();
        for property in record_properties {
            match property.code {
                0x03 if attributes.len() < capacity => {
                    name = property
                        .string_bytes(file)
                        .ok_or_else(|| AttributeError("CATR 0x03 name is not a string".into()))?
                        .iter()
                        .copied()
                        .take(64)
                        .collect();
                }
                0x0f if attributes.len() < capacity => {
                    attributes.push(parse_attribute_value(
                        file,
                        property,
                        std::mem::take(&mut name),
                    )?);
                }
                _ => {}
            }
        }

        Ok(Self {
            node_index,
            declared_count,
            attributes,
        })
    }

    pub fn ext_param(&self) -> Option<&ExtParamData> {
        self.attributes
            .iter()
            .find_map(|attribute| match &attribute.value {
                CastAttributeValue::ExtParam { parsed, .. } => Some(parsed),
                _ => None,
            })
    }

    /// Replays the SrTextCast subclass parser over matching string records in
    /// original CATR order. Later records and later tokens overwrite earlier
    /// fields exactly as repeated calls to `sub_AB8720` do.
    pub fn font_param(&self) -> Option<FontParamData> {
        let mut result = FontParamData::default();
        let mut found = false;
        for attribute in &self.attributes {
            if attribute.name != b"FontParamData" || attribute.source_type_code != 2 {
                continue;
            }
            let CastAttributeValue::String(source) = &attribute.value else {
                continue;
            };
            result.apply(source);
            found = true;
        }
        found.then_some(result)
    }
}

fn parse_attribute_value(
    file: &SrdFile,
    property: &Property,
    name: Vec<u8>,
) -> Result<CastAttribute, AttributeError> {
    let source_type_code = property.type_code;
    let value = match source_type_code {
        1 | 8 => CastAttributeValue::Signed(
            property
                .read_signed_scalar(file)
                .ok_or_else(|| AttributeError("invalid signed CATR 0x0F value".into()))?,
        ),
        2 => {
            let source = property
                .string_bytes(file)
                .ok_or_else(|| AttributeError("invalid string CATR 0x0F value".into()))?
                .to_vec();
            if name == b"ExtParamData" {
                CastAttributeValue::ExtParam {
                    parsed: ExtParamData::parse(&source),
                    source,
                }
            } else {
                CastAttributeValue::String(source)
            }
        }
        10 => CastAttributeValue::Float(
            property
                .read_scalar_as_f32(file)
                .ok_or_else(|| AttributeError("invalid float CATR 0x0F value".into()))?,
        ),
        type_code => CastAttributeValue::Unsupported {
            type_code,
            bytes: property.value_bytes(file).to_vec(),
        },
    };
    Ok(CastAttribute {
        name,
        source_type_code,
        value,
    })
}

fn set_flag(flags: &mut u32, bit: u32, enabled: bool) {
    *flags = (*flags & !bit) | if enabled { bit } else { 0 };
}

fn c_atoi(bytes: &[u8]) -> i32 {
    let mut index = 0;
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
    {
        index += 1;
    }
    let negative = match bytes.get(index) {
        Some(b'-') => {
            index += 1;
            true
        }
        Some(b'+') => {
            index += 1;
            false
        }
        _ => false,
    };
    let mut found = false;
    let mut value = 0i64;
    while let Some(byte @ b'0'..=b'9') = bytes.get(index) {
        found = true;
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'));
        index += 1;
    }
    if !found {
        return 0;
    }
    if negative {
        value = -value;
    }
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_param_parser_matches_binary_field_transforms() {
        let parsed = ExtParamData::parse(
            b"blendMode#26,layerLevel#128,layer#-1,layerKind#1,enableLayer#1,enableKind#1,enableLevel#1",
        );
        assert_eq!(parsed.render_preset_override, 59);
        assert_eq!(parsed.layer_level, 0);
        assert_eq!(parsed.layer, 0xff);
        assert_eq!(parsed.field_06, 0);
        assert_eq!(parsed.flags, 0x0e);
    }

    #[test]
    fn ext_param_zero_blend_and_default_enable_state_match_binary() {
        let parsed = ExtParamData::parse(b"blendMode#0,layerLevel#128,layer#-1,layerKind#0");
        assert_eq!(parsed.render_preset_override, -1);
        assert_eq!(parsed.layer_level, 0);
        assert_eq!(parsed.layer, 0xff);
        assert_eq!(parsed.flags, ExtParamData::LAYER_KIND);
    }

    #[test]
    fn layer_key_replaces_only_enabled_fields() {
        let parsed = ExtParamData {
            layer_level: 0x34,
            layer: 0xfe,
            flags: ExtParamData::ENABLE_KIND
                | ExtParamData::LAYER_KIND
                | ExtParamData::ENABLE_LAYER
                | ExtParamData::ENABLE_LEVEL,
            ..ExtParamData::default()
        };
        assert_eq!(parsed.compose_layer_key(0xabcd_5678), 0xabcd_fe34);
        assert_eq!(
            ExtParamData::default().compose_layer_key(0x1234_5678),
            0x1234_5678
        );
    }

    #[test]
    fn font_param_parser_matches_binary_defaults_clamps_and_token_order() {
        assert_eq!(FontParamData::parse(b""), FontParamData::default());

        let parsed = FontParamData::parse(
            b"vertical#True,prohibition#true,wordWrap#True,monospaced#True,autoScalingHeight#True,noWrapPutMode#9,scrollSpeed#-12,scrollWait#+7,diplayShadow#True,shadowX#3,shadowY#4,pointX#5,pointY#6,outline#7,italic#8,bold#9,faceId#10,shadowColor#-1,outlineColor#305419896",
        );
        assert!(parsed.vertical);
        assert!(!parsed.prohibition);
        assert!(parsed.word_wrap);
        assert!(parsed.monospaced);
        assert!(parsed.display_shadow);
        assert_eq!(parsed.no_wrap_put_mode, 6);
        assert_eq!(parsed.scroll_speed, -12);
        assert_eq!(parsed.scroll_wait, 7);
        assert_eq!(parsed.shadow_x, 3);
        assert_eq!(parsed.shadow_y, 4);
        assert_eq!(parsed.point_x, 5);
        assert_eq!(parsed.point_y, 6);
        assert_eq!(parsed.outline, 7);
        assert_eq!(parsed.italic, 8);
        assert_eq!(parsed.bold, 9);
        assert_eq!(parsed.face_id, 10);
        assert_eq!(parsed.shadow_color, u32::MAX);
        assert_eq!(parsed.outline_color, 0x1234_5678);

        assert_eq!(
            FontParamData::parse(b"noWrapPutMode#4,autoScalingHeight#False").no_wrap_put_mode,
            0
        );
        assert_eq!(
            FontParamData::parse(b"autoScalingHeight#False,noWrapPutMode#-2").no_wrap_put_mode,
            0
        );
    }
}
