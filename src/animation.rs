use std::fmt;

use crate::transform::SpatialTransform;
use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationDefinition {
    pub name: Vec<u8>,
    pub flags: u32,
    pub declared_motion_count: u32,
    pub duration: i32,
    pub motions: Vec<Motion>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeAnimationState {
    pub frame: f32,
    pub duration: f32,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    pub target: i32,
    pub tracks: Vec<Track>,
}

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
    Bytes4([u8; 4]),
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
        let frame = self.wrapped_frame(frame);
        match self.format & 3 {
            0 | 1 => self.evaluate_key8(frame),
            2 => Evaluation::Unchanged,
            3 => self.evaluate_key20(frame),
            _ => unreachable!(),
        }
    }

    pub fn wrapped_frame(&self, frame: f32) -> f32 {
        wrap_time(self.format, self.range_start, self.range_end, frame)
    }

    fn evaluate_key8(&self, frame: f32) -> Evaluation {
        match (&self.keys, self.format & 0x70) {
            (KeyData::Key8F32(keys), 0x10) => evaluate_key8_f32(keys, frame),
            (KeyData::Key8I32(keys), 0x40) => evaluate_key8_i32_linear(keys, frame),
            (KeyData::Key8Bytes4(keys), 0x50) => evaluate_key8_bytes4(keys, frame),
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

impl AnimationDefinition {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, AnimationError> {
        if !block.is_tag(b"ANIM") {
            return Err(AnimationError("block is not ANIM".into()));
        }
        let name = block
            .last_property(0x03)
            .and_then(|property| property.string_bytes(file))
            .ok_or_else(|| AnimationError("missing/invalid ANIM 0x03".into()))?
            .iter()
            .copied()
            .take(64)
            .collect();
        let flags = read_unsigned(file, block, 0x5f)?;
        let declared_motion_count = read_unsigned(file, block, 0x50)?;
        let duration = read_signed(file, block, 0x56)?;
        let mut motions = block
            .children
            .iter()
            .filter(|child| child.is_tag(b"MOT "))
            .map(|child| Motion::from_block(file, child))
            .collect::<Result<Vec<_>, _>>()?;
        let motion_slots = usize::try_from(declared_motion_count)
            .map_err(|_| AnimationError("ANIM 0x50 does not fit usize".into()))?;
        if motions.len() > motion_slots {
            return Err(AnimationError(format!(
                "ANIM has {} MOT blocks but only {declared_motion_count} motion slots",
                motions.len()
            )));
        }
        motions.resize_with(motion_slots, || Motion {
            target: -1,
            tracks: Vec::new(),
        });
        Ok(Self {
            name,
            flags,
            declared_motion_count,
            duration,
            motions,
        })
    }

    pub fn runtime_duration(&self) -> f32 {
        if self.duration >= 0 {
            return self.duration as f32;
        }
        self.motions
            .iter()
            .flat_map(|motion| &motion.tracks)
            .fold(0.0f32, |duration, track| {
                duration.max(track.range_end as f32)
            })
    }

    pub fn initial_runtime_state(&self) -> RuntimeAnimationState {
        RuntimeAnimationState {
            frame: 0.0,
            duration: self.runtime_duration(),
            flags: 9 | ((self.flags & 1) << 1),
        }
    }

    pub fn apply_common_channels(
        &self,
        transforms: &mut [SpatialTransform],
        frame: f32,
    ) -> Result<usize, AnimationError> {
        let mut applied = 0usize;
        let transform_count = transforms.len();
        for motion in &self.motions {
            if motion.target < 0 {
                continue;
            }
            let index = usize::try_from(motion.target)
                .map_err(|_| AnimationError("MOT target does not fit usize".into()))?;
            let transform = transforms.get_mut(index).ok_or_else(|| {
                AnimationError(format!(
                    "MOT target {index} is outside {} runtime CASTs",
                    transform_count
                ))
            })?;
            applied += motion.apply_proven_common_channels(transform, frame);
        }
        Ok(applied)
    }
}

impl Motion {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, AnimationError> {
        if !block.is_tag(b"MOT ") {
            return Err(AnimationError("block is not MOT ".into()));
        }
        let target = i32::from(read_signed(file, block, 0x51)? as i16);
        let tracks = block
            .children
            .iter()
            .filter(|child| child.is_tag(b"TRK "))
            .map(|child| Track::from_block(file, child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { target, tracks })
    }

    pub fn apply_proven_common_channels(
        &self,
        transform: &mut SpatialTransform,
        frame: f32,
    ) -> usize {
        self.tracks
            .iter()
            .filter(|track| transform.apply_common_track(track.target, track.evaluate(frame)))
            .count()
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

fn evaluate_key8_bytes4(keys: &[Key8<[u8; 4]>], frame: f32) -> Evaluation {
    const BYTE_TO_UNIT: f32 = f32::from_bits(0x3b80_8081);
    const UNIT_TO_BYTE: f32 = f32::from_bits(0x437f_0000);

    match segment8(keys, frame) {
        Segment8::Empty => Evaluation::Unsupported,
        Segment8::Value(value) => Evaluation::Value(ScalarValue::Bytes4(value)),
        Segment8::Pair(left, right) => {
            let t = normalized(frame, left.frame, right.frame);
            let inverse = 1.0 - t;
            let mut value = [0u8; 4];
            for (index, destination) in value.iter_mut().enumerate() {
                let mut component = left.value[index] as f32;
                component *= BYTE_TO_UNIT;
                component *= inverse;
                let mut right_component = right.value[index] as f32;
                right_component *= BYTE_TO_UNIT;
                right_component *= t;
                component += right_component;
                component *= UNIT_TO_BYTE;
                *destination = cvtt_f32_to_i32(component) as u8;
            }
            Evaluation::Value(ScalarValue::Bytes4(value))
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
        _ if frame.is_nan() => Segment8::Value(keys[0].value),
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
        _ if frame.is_nan() => Segment20::Value(keys[0].value),
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

pub(crate) fn cvtt_f32_to_i32(value: f32) -> i32 {
    if !value.is_finite() || !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_animation_uses_explicit_or_track_derived_duration_and_binary_flags() {
        let track = Track {
            target: 0,
            key_count: 0,
            format: 0,
            range_start: 0,
            range_end: 42,
            keys: KeyData::Unsupported,
        };
        let mut animation = AnimationDefinition {
            name: b"animation".to_vec(),
            flags: 1,
            declared_motion_count: 1,
            duration: -1,
            motions: vec![Motion {
                target: 0,
                tracks: vec![track],
            }],
        };
        assert_eq!(animation.runtime_duration(), 42.0);
        assert_eq!(
            animation.initial_runtime_state(),
            RuntimeAnimationState {
                frame: 0.0,
                duration: 42.0,
                flags: 11,
            }
        );
        animation.duration = 12;
        assert_eq!(animation.runtime_duration(), 12.0);
    }

    #[test]
    fn nan_frames_select_the_first_key_like_the_binary_comparison() {
        let track = Track {
            target: 0,
            key_count: 2,
            format: 0x10,
            range_start: 0,
            range_end: 10,
            keys: KeyData::Key8F32(vec![
                Key8 {
                    frame: 0,
                    value: 3.0,
                },
                Key8 {
                    frame: 10,
                    value: 9.0,
                },
            ]),
        };
        assert_eq!(
            track.evaluate(f32::NAN),
            Evaluation::Value(ScalarValue::F32(3.0))
        );
    }

    #[test]
    fn byte4_keys_use_the_binary_normalize_lerp_and_truncate_chain() {
        let track = Track {
            target: 13,
            key_count: 2,
            format: 0x51,
            range_start: 0,
            range_end: 10,
            keys: KeyData::Key8Bytes4(vec![
                Key8 {
                    frame: 0,
                    value: [0, 64, 128, 255],
                },
                Key8 {
                    frame: 10,
                    value: [255, 128, 0, 0],
                },
            ]),
        };
        assert_eq!(
            track.evaluate(5.0),
            Evaluation::Value(ScalarValue::Bytes4([127, 96, 64, 127]))
        );
        assert_eq!(
            track.evaluate(0.0),
            Evaluation::Value(ScalarValue::Bytes4([0, 64, 128, 255]))
        );
    }
}
