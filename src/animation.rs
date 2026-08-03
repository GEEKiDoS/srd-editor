use std::fmt;

use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub target: u16,
    pub key_count: u16,
    pub format: u32,
    pub range_start: i32,
    pub range_end: i32,
    pub keys: KeyData,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyData {
    Key8F32(Vec<Key8<f32>>),
    Key8I32(Vec<Key8<i32>>),
    Key8Bytes4(Vec<Key8<[u8; 4]>>),
    Key20F32(Vec<Key20<f32>>),
    Key20I32(Vec<Key20<i32>>),
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key8<T> {
    pub frame: i32,
    pub value: T,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key20<T> {
    pub frame: i32,
    pub value: T,
    pub mode: u32,
    pub slope_in: f32,
    pub slope_out: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalarValue {
    F32(f32),
    I32(i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Evaluation {
    Value(ScalarValue),
    Unchanged,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationError(pub String);

impl fmt::Display for AnimationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AnimationError {}

impl Track {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, AnimationError> {
        if !block.is_tag(b"TRK ") {
            return Err(AnimationError("block is not TRK ".into()));
        }

        let target = read_unsigned(file, block, 0x53)? as u16;
        let key_count = read_unsigned(file, block, 0x57)? as u16;
        let format = read_unsigned(file, block, 0x54)?;
        let range_start = read_signed(file, block, 0x58)?;
        let range_end = read_signed(file, block, 0x59)?;
        let low = format & 3;
        let family = format & 0x70;
        let key_block = block
            .children
            .iter()
            .rev()
            .find(|child| child.is_tag(b"KEY "));

        let keys = match (low, family, key_block) {
            (_, _, None) if key_count == 0 => KeyData::Unsupported,
            (0 | 1, 0x10, Some(keys)) => {
                KeyData::Key8F32(parse_key8(file, keys, key_count, |property| {
                    property.read_scalar_as_f32(file)
                })?)
            }
            (0 | 1, 0x40, Some(keys)) => {
                KeyData::Key8I32(parse_key8(file, keys, key_count, |property| {
                    property.read_signed_scalar(file)
                })?)
            }
            (0 | 1, 0x50, Some(keys)) => {
                KeyData::Key8Bytes4(parse_key8_bytes4(file, keys, key_count)?)
            }
            (3, 0x10, Some(keys)) => {
                KeyData::Key20F32(parse_key20(file, keys, key_count, |property| {
                    property.read_scalar_as_f32(file)
                })?)
            }
            (3, 0x20 | 0x40, Some(keys)) => {
                KeyData::Key20I32(parse_key20(file, keys, key_count, |property| {
                    property.read_signed_scalar(file)
                })?)
            }
            _ => KeyData::Unsupported,
        };

        Ok(Self {
            target,
            key_count,
            format,
            range_start,
            range_end,
            keys,
        })
    }

    pub fn evaluate(&self, frame: f32) -> Evaluation {
        if self.key_count == 0 {
            return Evaluation::Unsupported;
        }
        let frame = wrap_time(self.format, self.range_start, self.range_end, frame);
        match self.format & 3 {
            0 | 1 => self.evaluate_key8(frame),
            2 => Evaluation::Unchanged,
            3 => self.evaluate_key20(frame),
            _ => unreachable!(),
        }
    }

    fn evaluate_key8(&self, frame: f32) -> Evaluation {
        match (&self.keys, self.format & 0x70) {
            (KeyData::Key8F32(keys), 0x10) => evaluate_key8_f32(keys, frame),
            (KeyData::Key8I32(keys), 0x40) => evaluate_key8_i32_linear(keys, frame),
            (KeyData::Key8Bytes4(_), 0x50) => Evaluation::Unsupported,
            _ => Evaluation::Unsupported,
        }
    }

    fn evaluate_key20(&self, frame: f32) -> Evaluation {
        match (&self.keys, self.format & 0x70) {
            (KeyData::Key20F32(keys), 0x10) => evaluate_key20_f32(keys, frame),
            (KeyData::Key20I32(keys), 0x20) => evaluate_key20_i32(keys, frame, false),
            (KeyData::Key20I32(keys), 0x40) => evaluate_key20_i32(keys, frame, true),
            _ => Evaluation::Unsupported,
        }
    }
}

fn read_unsigned(file: &SrdFile, block: &Block, code: u8) -> Result<u32, AnimationError> {
    block
        .last_property(code)
        .and_then(|property| property.read_unsigned_scalar(file))
        .ok_or_else(|| AnimationError(format!("missing/invalid property {code:#04x}")))
}

fn read_signed(file: &SrdFile, block: &Block, code: u8) -> Result<i32, AnimationError> {
    block
        .last_property(code)
        .and_then(|property| property.read_signed_scalar(file))
        .ok_or_else(|| AnimationError(format!("missing/invalid property {code:#04x}")))
}

fn parse_key8<T: Copy>(
    file: &SrdFile,
    block: &Block,
    expected: u16,
    read_value: impl Fn(&Property) -> Option<T>,
) -> Result<Vec<Key8<T>>, AnimationError> {
    let mut keys = Vec::with_capacity(usize::from(expected));
    for property in &block.properties {
        match property.code {
            0x5a => {
                let frame = property
                    .read_signed_scalar(file)
                    .ok_or_else(|| AnimationError("invalid KEY frame".into()))?;
                keys.push((frame, None));
            }
            0x5b => {
                let (_, value) = keys
                    .last_mut()
                    .ok_or_else(|| AnimationError("KEY value precedes frame".into()))?;
                *value = read_value(property);
            }
            _ => {}
        }
    }
    if keys.len() != usize::from(expected) {
        return Err(AnimationError(format!(
            "KEY count mismatch: expected {expected}, parsed {}",
            keys.len()
        )));
    }
    keys.into_iter()
        .map(|(frame, value)| {
            Ok(Key8 {
                frame,
                value: value.ok_or_else(|| AnimationError("KEY missing value".into()))?,
            })
        })
        .collect()
}

fn parse_key8_bytes4(
    file: &SrdFile,
    block: &Block,
    expected: u16,
) -> Result<Vec<Key8<[u8; 4]>>, AnimationError> {
    parse_key8(file, block, expected, |property| {
        let source = property.value_bytes(file);
        (source.len() >= 4).then(|| [source[1], source[2], source[3], source[0]])
    })
}

fn parse_key20<T: Copy>(
    file: &SrdFile,
    block: &Block,
    expected: u16,
    read_value: impl Fn(&Property) -> Option<T>,
) -> Result<Vec<Key20<T>>, AnimationError> {
    #[derive(Clone, Copy)]
    struct Pending<T> {
        frame: i32,
        value: Option<T>,
        mode: Option<u32>,
        slope_in: Option<f32>,
        slope_out: Option<f32>,
    }

    let mut keys: Vec<Pending<T>> = Vec::with_capacity(usize::from(expected));
    for property in &block.properties {
        match property.code {
            0x5a => keys.push(Pending {
                frame: property
                    .read_signed_scalar(file)
                    .ok_or_else(|| AnimationError("invalid KEY frame".into()))?,
                value: None,
                mode: None,
                slope_in: None,
                slope_out: None,
            }),
            0x5b => current(&mut keys)?.value = read_value(property),
            0x5c => current(&mut keys)?.mode = property.read_unsigned_scalar(file),
            0x5d => current(&mut keys)?.slope_in = property.read_scalar_as_f32(file),
            0x5e => current(&mut keys)?.slope_out = property.read_scalar_as_f32(file),
            _ => {}
        }
    }
    if keys.len() != usize::from(expected) {
        return Err(AnimationError(format!(
            "KEY count mismatch: expected {expected}, parsed {}",
            keys.len()
        )));
    }
    keys.into_iter()
        .map(|key| {
            Ok(Key20 {
                frame: key.frame,
                value: key
                    .value
                    .ok_or_else(|| AnimationError("KEY missing value".into()))?,
                mode: key
                    .mode
                    .ok_or_else(|| AnimationError("KEY missing mode".into()))?,
                slope_in: key
                    .slope_in
                    .ok_or_else(|| AnimationError("KEY missing +0x0c field".into()))?,
                slope_out: key
                    .slope_out
                    .ok_or_else(|| AnimationError("KEY missing +0x10 field".into()))?,
            })
        })
        .collect()
}

fn current<T>(keys: &mut [T]) -> Result<&mut T, AnimationError> {
    keys.last_mut()
        .ok_or_else(|| AnimationError("KEY field precedes frame".into()))
}

fn wrap_time(format: u32, start: i32, end: i32, frame: f32) -> f32 {
    if format & 0x300 == 0 {
        return frame;
    }
    let start = start as f32;
    let end = end as f32;
    if frame >= start && frame < end {
        return frame;
    }
    let span = end - start;
    let quotient = (frame - start) / span;
    let wrapped_quotient = if quotient > 0.0 {
        cvtt_f32_to_i32(quotient) as f32
    } else if quotient < 0.0 {
        cvtt_f32_to_i32(quotient) as f32 - 1.0
    } else {
        quotient
    };
    frame - wrapped_quotient * span
}

fn evaluate_key8_f32(keys: &[Key8<f32>], frame: f32) -> Evaluation {
    match segment8(keys, frame) {
        Segment8::Empty => Evaluation::Unsupported,
        Segment8::Value(value) => Evaluation::Value(ScalarValue::F32(value)),
        Segment8::Pair(left, right) => {
            let t = normalized(frame, left.frame, right.frame);
            Evaluation::Value(ScalarValue::F32((1.0 - t) * left.value + right.value * t))
        }
    }
}

fn evaluate_key8_i32_linear(keys: &[Key8<i32>], frame: f32) -> Evaluation {
    match segment8(keys, frame) {
        Segment8::Empty => Evaluation::Unsupported,
        Segment8::Value(value) => Evaluation::Value(ScalarValue::I32(value)),
        Segment8::Pair(left, right) => {
            let t = normalized(frame, left.frame, right.frame);
            let value = left.value as f32 * (1.0 - t) + right.value as f32 * t;
            Evaluation::Value(ScalarValue::I32(cvtt_f32_to_i32(value)))
        }
    }
}

fn evaluate_key20_f32(keys: &[Key20<f32>], frame: f32) -> Evaluation {
    match segment20(keys, frame) {
        Segment20::Empty => Evaluation::Unsupported,
        Segment20::Value(value) => Evaluation::Value(ScalarValue::F32(value)),
        Segment20::Pair(left, right) => {
            let value = match left.mode {
                0 => left.value,
                2 => cubic_f32(left, right, frame),
                _ => {
                    let t = normalized(frame, left.frame, right.frame);
                    (1.0 - t) * left.value + right.value * t
                }
            };
            Evaluation::Value(ScalarValue::F32(value))
        }
    }
}

fn evaluate_key20_i32(keys: &[Key20<i32>], frame: f32, family40: bool) -> Evaluation {
    match segment20(keys, frame) {
        Segment20::Empty => Evaluation::Unsupported,
        Segment20::Value(value) => Evaluation::Value(ScalarValue::I32(value)),
        Segment20::Pair(left, right) => {
            let value = match left.mode {
                0 => left.value,
                2 if family40 => cubic_i32_family40(left, right, frame),
                _ => {
                    let t = normalized(frame, left.frame, right.frame);
                    cvtt_f32_to_i32(left.value as f32 * (1.0 - t) + right.value as f32 * t)
                }
            };
            Evaluation::Value(ScalarValue::I32(value))
        }
    }
}

fn cubic_f32(left: &Key20<f32>, right: &Key20<f32>, frame: f32) -> f32 {
    let span = (right.frame - left.frame) as f32;
    let t = (frame - left.frame as f32) / span;
    let delta = right.value - left.value;
    let mut a = right.slope_in + left.slope_out;
    a *= span;
    a -= delta * 2.0;
    let mut b = delta * 3.0;
    let mut slopes = left.slope_out * 2.0;
    slopes += right.slope_in;
    slopes *= span;
    b -= slopes;
    a *= t;
    a += b;
    a *= t;
    let c = left.slope_out * span;
    a += c;
    a *= t;
    a + left.value
}

fn cubic_i32_family40(left: &Key20<i32>, right: &Key20<i32>, frame: f32) -> i32 {
    const TO_FLOAT_DOMAIN: f32 = f32::from_bits(0x3bb4_0000);
    const FROM_FLOAT_DOMAIN_NEGATED: f32 = f32::from_bits(0xc336_0b60);

    let left_frame = left.frame as f32;
    let mut span = right.frame as f32;
    span -= left_frame;
    let mut t = frame;
    t -= left_frame;
    t /= span;

    let mut delta = right.value as f32;
    delta *= TO_FLOAT_DOMAIN;
    let mut left_value = left.value as f32;
    left_value *= TO_FLOAT_DOMAIN;
    delta -= left_value;

    let mut polynomial = right.slope_in;
    polynomial += left.slope_out;
    polynomial *= span;
    let mut twice_delta = delta;
    twice_delta *= 2.0;
    polynomial -= twice_delta;

    let mut b = delta;
    b *= 3.0;
    let mut slopes = left.slope_out;
    slopes *= 2.0;
    slopes += right.slope_in;
    slopes *= span;
    b -= slopes;

    let c = left.slope_out * span;
    polynomial *= t;
    polynomial += b;
    polynomial *= t;
    polynomial += c;
    polynomial *= t;
    polynomial *= FROM_FLOAT_DOMAIN_NEGATED;

    left.value - cvtt_f32_to_i32(polynomial)
}

fn normalized(frame: f32, left: i32, right: i32) -> f32 {
    (frame - left as f32) / (right - left) as f32
}

enum Segment8<'a, T: Copy> {
    Empty,
    Value(T),
    Pair(&'a Key8<T>, &'a Key8<T>),
}

fn segment8<T: Copy>(keys: &[Key8<T>], frame: f32) -> Segment8<'_, T> {
    match keys {
        [] => Segment8::Empty,
        [key] => Segment8::Value(key.value),
        _ if frame <= keys[0].frame as f32 => Segment8::Value(keys[0].value),
        _ if frame >= keys[keys.len() - 1].frame as f32 => {
            Segment8::Value(keys[keys.len() - 1].value)
        }
        _ => {
            let upper = keys.partition_point(|key| key.frame as f32 <= frame);
            Segment8::Pair(&keys[upper - 1], &keys[upper])
        }
    }
}

enum Segment20<'a, T: Copy> {
    Empty,
    Value(T),
    Pair(&'a Key20<T>, &'a Key20<T>),
}

fn segment20<T: Copy>(keys: &[Key20<T>], frame: f32) -> Segment20<'_, T> {
    match keys {
        [] => Segment20::Empty,
        [key] => Segment20::Value(key.value),
        _ if frame <= keys[0].frame as f32 => Segment20::Value(keys[0].value),
        _ if frame >= keys[keys.len() - 1].frame as f32 => {
            Segment20::Value(keys[keys.len() - 1].value)
        }
        _ => {
            let upper = keys.partition_point(|key| key.frame as f32 <= frame);
            Segment20::Pair(&keys[upper - 1], &keys[upper])
        }
    }
}

fn cvtt_f32_to_i32(value: f32) -> i32 {
    if !value.is_finite() || !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}
