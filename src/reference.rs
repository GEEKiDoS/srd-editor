use std::fmt;

use crate::animation::{Evaluation, ScalarValue, Track};
use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceError(pub String);

impl fmt::Display for ReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReferenceError {}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceDefinition {
    pub source_name: Vec<u8>,
    pub layer_name: Vec<u8>,
    pub animation_enabled: u32,
    pub animation_name: Vec<u8>,
    pub default_frame: f32,
    pub node_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceAnimationRequest<'a> {
    pub source_name: &'a [u8],
    pub layer_name: &'a [u8],
    pub animation_name: &'a [u8],
    pub frame: f32,
}

impl ReferenceDefinition {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, ReferenceError> {
        if !block.is_tag(b"CRFD") {
            return Err(ReferenceError("block is not CRFD".into()));
        }

        let mut result = Self {
            source_name: Vec::new(),
            layer_name: Vec::new(),
            animation_enabled: 0,
            animation_name: Vec::new(),
            default_frame: 0.0,
            node_index: -1,
        };
        for property in &block.properties {
            match property.code {
                0x51 => result.node_index = unsigned_scalar(file, property, "CRFD 0x51")? as i32,
                0x80 => result.source_name = fixed_string(file, property, "CRFD 0x80")?,
                0x81 => result.layer_name = fixed_string(file, property, "CRFD 0x81")?,
                0x82 => result.animation_enabled = unsigned_scalar(file, property, "CRFD 0x82")?,
                0x83 => result.animation_name = fixed_string(file, property, "CRFD 0x83")?,
                0x84 => {
                    result.default_frame = property
                        .read_scalar_as_f32(file)
                        .ok_or_else(|| ReferenceError("invalid CRFD 0x84".into()))?
                }
                _ => {}
            }
        }
        Ok(result)
    }

    pub fn animation_request(
        &self,
        track: &Track,
        frame: f32,
    ) -> Option<ReferenceAnimationRequest<'_>> {
        if track.target != 23 || self.animation_enabled == 0 {
            return None;
        }

        let mut referenced_frame = f32::from_bits(0xbf80_0000);
        match track.evaluate(frame) {
            Evaluation::Value(ScalarValue::F32(value)) => referenced_frame = value,
            Evaluation::Value(ScalarValue::I32(value)) => {
                referenced_frame = f32::from_bits(value as u32)
            }
            Evaluation::Value(ScalarValue::Bytes4(value)) => {
                referenced_frame = f32::from_bits(u32::from_le_bytes(value))
            }
            Evaluation::Unchanged | Evaluation::Unsupported => {}
        }
        if referenced_frame < 0.0 {
            referenced_frame = self.default_frame;
        }
        Some(ReferenceAnimationRequest {
            source_name: &self.source_name,
            layer_name: &self.layer_name,
            animation_name: &self.animation_name,
            frame: referenced_frame,
        })
    }
}

fn fixed_string(
    file: &SrdFile,
    property: &Property,
    field: &str,
) -> Result<Vec<u8>, ReferenceError> {
    property
        .string_bytes(file)
        .map(|value| value.iter().copied().take(512).collect())
        .ok_or_else(|| ReferenceError(format!("{field} is not a string")))
}

fn unsigned_scalar(
    file: &SrdFile,
    property: &Property,
    field: &str,
) -> Result<u32, ReferenceError> {
    property
        .read_unsigned_scalar(file)
        .ok_or_else(|| ReferenceError(format!("invalid {field}")))
}

#[cfg(test)]
mod tests {
    use crate::animation::{Key20, KeyData};

    use super::*;

    fn definition() -> ReferenceDefinition {
        ReferenceDefinition {
            source_name: b"source".to_vec(),
            layer_name: b"layer".to_vec(),
            animation_enabled: 1,
            animation_name: b"motion".to_vec(),
            default_frame: 6.0,
            node_index: 2,
        }
    }

    #[test]
    fn negative_or_unchanged_channel_23_values_use_the_crfd_default_frame() {
        let track = Track {
            target: 23,
            key_count: 1,
            format: 0x13,
            range_start: 0,
            range_end: 0,
            keys: KeyData::Key20F32(vec![Key20 {
                frame: 0,
                value: -1.0,
                mode: 0,
                slope_in: 0.0,
                slope_out: 0.0,
            }]),
        };
        assert_eq!(
            definition().animation_request(&track, 0.0),
            Some(ReferenceAnimationRequest {
                source_name: b"source",
                layer_name: b"layer",
                animation_name: b"motion",
                frame: 6.0,
            })
        );

        let unchanged = Track {
            format: 0x12,
            keys: KeyData::Unsupported,
            ..track
        };
        assert_eq!(
            definition().animation_request(&unchanged, 0.0),
            Some(ReferenceAnimationRequest {
                source_name: b"source",
                layer_name: b"layer",
                animation_name: b"motion",
                frame: 6.0,
            })
        );
    }

    #[test]
    fn disabled_or_non_reference_tracks_do_not_issue_requests() {
        let track = Track {
            target: 23,
            key_count: 1,
            format: 0x13,
            range_start: 0,
            range_end: 0,
            keys: KeyData::Key20F32(vec![Key20 {
                frame: 0,
                value: 12.0,
                mode: 0,
                slope_in: 0.0,
                slope_out: 0.0,
            }]),
        };
        let mut disabled = definition();
        disabled.animation_enabled = 0;
        assert_eq!(disabled.animation_request(&track, 0.0), None);

        let non_reference = Track {
            target: 22,
            ..track
        };
        assert_eq!(definition().animation_request(&non_reference, 0.0), None);
    }

    #[test]
    fn nan_channel_23_values_are_not_replaced_by_the_default_frame() {
        let nan = f32::from_bits(0x7fc0_1234);
        let track = Track {
            target: 23,
            key_count: 1,
            format: 0x13,
            range_start: 0,
            range_end: 0,
            keys: KeyData::Key20F32(vec![Key20 {
                frame: 0,
                value: nan,
                mode: 0,
                slope_in: 0.0,
                slope_out: 0.0,
            }]),
        };
        let definition = definition();
        let request = definition.animation_request(&track, 0.0).unwrap();
        assert_eq!(request.frame.to_bits(), nan.to_bits());
    }
}
