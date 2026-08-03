use crate::animation::{Evaluation, ScalarValue};

const ROTATION_UNIT: f32 = f32::from_bits(0x38c9_0fdb);

const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const INV_TAU: f64 = f64::from_bits(0x3fc4_5f30_6dc9_c883);
const ONE_EIGHTH: f64 = f64::from_bits(0x3fc0_0000_0000_0000);
const THREE_EIGHTHS: f64 = f64::from_bits(0x3fd8_0000_0000_0000);
const TAU: f64 = f64::from_bits(0x4019_21fb_5444_2d18);
const SIN_C1: f64 = f64::from_bits(0x3ec7_1de3_a556_c734);
const SIN_C2: f64 = f64::from_bits(0x3f2a_01a0_1a01_a01a);
const SIN_C3: f64 = f64::from_bits(0x3f81_1111_1111_1111);
const SIN_C4: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
const COS_C1: f64 = f64::from_bits(0x3efa_01a0_1a01_a01a);
const COS_C2: f64 = f64::from_bits(0x3f56_c16c_16c1_6c17);
const COS_C3: f64 = f64::from_bits(0x3fa5_5555_5555_5555);
const ONE_QUARTER: f64 = f64::from_bits(0x3fd0_0000_0000_0000);
const ONE_HALF: f64 = f64::from_bits(0x3fe0_0000_0000_0000);
const ONE: f64 = f64::from_bits(0x3ff0_0000_0000_0000);
const FIVE_EIGHTHS: f64 = f64::from_bits(0x3fe4_0000_0000_0000);
const THREE_QUARTERS: f64 = f64::from_bits(0x3fe8_0000_0000_0000);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialTransform {
    pub translation: [f32; 3],
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    pub multiply_color: [u8; 4],
    pub additive_color: [u8; 4],
    pub visibility_word: u32,
}

impl Default for SpatialTransform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0; 3],
            scale: [1.0; 3],
            multiply_color: [255; 4],
            additive_color: [0; 4],
            visibility_word: 1,
        }
    }
}

impl SpatialTransform {
    pub fn apply_common_track(&mut self, target: u16, evaluation: Evaluation) -> bool {
        match target {
            0..=8 | 10 => {
                let Evaluation::Value(value) = evaluation else {
                    return false;
                };
                let bits = scalar_bits(value);
                match target {
                    0..=2 => self.translation[usize::from(target)] = f32::from_bits(bits),
                    3..=5 => self.rotation[usize::from(target - 3)] = bits as i32,
                    6..=8 => self.scale[usize::from(target - 6)] = f32::from_bits(bits),
                    10 => self.visibility_word = bits,
                    _ => unreachable!(),
                }
            }
            9 | 19 => {
                let Evaluation::Value(value) = evaluation else {
                    return false;
                };
                let bytes = scalar_bits(value).to_le_bytes();
                let color = if target == 9 {
                    &mut self.multiply_color
                } else {
                    &mut self.additive_color
                };
                color[0] = bytes[2];
                color[1] = bytes[1];
                color[2] = bytes[0];
            }
            21 | 22 => {
                let value = match evaluation {
                    Evaluation::Value(value) => f32::from_bits(scalar_bits(value)),
                    Evaluation::Unchanged | Evaluation::Unsupported => 1.0,
                };
                let alpha = normalized_alpha_to_u8(value);
                if target == 21 {
                    self.multiply_color[3] = alpha;
                } else {
                    self.additive_color[3] = alpha;
                }
            }
            _ => return false,
        }
        true
    }

    pub fn is_visible(&self) -> bool {
        self.visibility_word & 0xff != 0
    }
}

fn normalized_alpha_to_u8(value: f32) -> u8 {
    let clamped = if value > 1.0 {
        1.0
    } else if value.is_nan() {
        0.0
    } else {
        value.max(0.0)
    };
    (clamped * 255.0) as u8
}

fn scalar_bits(value: ScalarValue) -> u32 {
    match value {
        ScalarValue::F32(value) => value.to_bits(),
        ScalarValue::I32(value) => value as u32,
        ScalarValue::Bytes4(value) => u32::from_le_bytes(value),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine3x4 {
    pub rows: [[f32; 4]; 3],
}

impl Affine3x4 {
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };

    pub fn mul_game(self, right: Self) -> Self {
        let mut result = [[0.0; 4]; 3];
        for (row_index, left) in self.rows.iter().enumerate() {
            for (column, destination) in result[row_index].iter_mut().enumerate() {
                let unit = if column == 3 { 1.0 } else { 0.0 };
                let high = left[3] * unit + left[2] * right.rows[2][column];
                let low = left[1] * right.rows[1][column] + left[0] * right.rows[0][column];
                *destination = high + low;
            }
        }
        Self { rows: result }
    }
}

pub fn build_local_matrix(
    transform: &SpatialTransform,
    is_2d: bool,
    flip_y: bool,
    offset: [f32; 2],
) -> Affine3x4 {
    let tx = transform.translation[0] + offset[0];
    let mut ty = transform.translation[1] + offset[1];
    if flip_y {
        ty = -ty;
    }
    let tz = transform.translation[2];

    let mut matrix = if is_2d {
        build_rotation_2d(transform.rotation[2])
    } else {
        build_rotation_3d(transform.rotation)
    };

    let [scale_x, scale_y, scale_z] = transform.scale;
    for row in &mut matrix.rows {
        row[0] *= scale_x;
        row[1] *= scale_y;
        row[2] *= scale_z;
    }
    matrix.rows[0][3] = tx;
    matrix.rows[1][3] = ty;
    matrix.rows[2][3] = tz;
    matrix
}

fn build_rotation_2d(rotation_z: i32) -> Affine3x4 {
    let angle = rotation_z.wrapping_neg() as f32 * ROTATION_UNIT;
    let cosine = game_cos_f32(angle);
    let sine = game_sin_f32(angle);
    Affine3x4 {
        rows: [
            [cosine, -sine, 0.0, 0.0],
            [sine, cosine, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    }
}

fn build_rotation_3d(rotation: [i32; 3]) -> Affine3x4 {
    let angle_z = rotation[2] as f32 * ROTATION_UNIT;
    let cosine_z = game_cos_f32(angle_z);
    let sine_z = game_sin_f32(angle_z);

    let mut m00 = cosine_z + sine_z * 0.0;
    let mut m01 = cosine_z * 0.0 + -sine_z;
    let mut m02 = 0.0;
    let mut m10 = cosine_z * 0.0 + sine_z;
    let mut m11 = -sine_z * 0.0 + cosine_z;
    let mut m12 = 0.0;
    let mut m20 = cosine_z * 0.0 + sine_z * 0.0;
    let mut m21 = -sine_z * 0.0 + cosine_z * 0.0;
    let mut m22 = 1.0;

    let angle_y = rotation[1] as f32 * ROTATION_UNIT;
    let cosine_y = game_cos_f32(angle_y);
    let sine_y = game_sin_f32(angle_y);
    let negative_sine_y = -sine_y;

    let old_m00 = m00;
    let old_m02 = m02;
    m00 = cosine_y * old_m00 + negative_sine_y * old_m02;
    m02 = old_m00 * sine_y + cosine_y * old_m02;

    let old_m10 = m10;
    let old_m12 = m12;
    m10 = cosine_y * old_m10 + negative_sine_y * old_m12;
    m12 = old_m10 * sine_y + cosine_y * old_m12;

    let old_m20 = m20;
    let old_m22 = m22;
    m20 = cosine_y * old_m20 + negative_sine_y * old_m22;
    m22 = old_m20 * sine_y + cosine_y * old_m22;

    let angle_x = rotation[0] as f32 * ROTATION_UNIT;
    let cosine_x = game_cos_f32(angle_x);
    let sine_x = game_sin_f32(angle_x);
    let negative_sine_x = -sine_x;

    let old_m01 = m01;
    let old_m02 = m02;
    m01 = sine_x * old_m02 + cosine_x * old_m01;
    m02 = old_m01 * negative_sine_x + cosine_x * old_m02;

    let old_m11 = m11;
    let old_m12 = m12;
    m11 = sine_x * old_m12 + cosine_x * old_m11;
    m12 = old_m11 * negative_sine_x + cosine_x * old_m12;

    let old_m21 = m21;
    let old_m22 = m22;
    m21 = sine_x * old_m22 + cosine_x * old_m21;
    m22 = old_m21 * negative_sine_x + cosine_x * old_m22;

    Affine3x4 {
        rows: [
            [m00, m01, m02, 0.0],
            [m10, m11, m12, 0.0],
            [m20, m21, m22, 0.0],
        ],
    }
}

pub fn game_cos_f32(input: f32) -> f32 {
    let mut value = f64::from(input);
    if input < 0.0 {
        value = -value;
    }
    let phase = reduce_phase(value);
    let result = if phase >= THREE_EIGHTHS {
        if phase >= FIVE_EIGHTHS {
            let x = (phase - THREE_QUARTERS) * TAU;
            sin_polynomial(x)
        } else {
            let x = (phase - ONE_HALF) * TAU;
            -cos_polynomial(x * x)
        }
    } else if phase >= ONE_EIGHTH {
        let x = (phase - ONE_QUARTER) * TAU;
        -sin_polynomial(x)
    } else {
        let x = phase * TAU;
        cos_polynomial(x * x)
    };
    result as f32
}

pub fn game_sin_f32(input: f32) -> f32 {
    let mut value = f64::from(input);
    if input < 0.0 {
        value = PI - value;
    }
    let phase = reduce_phase(value);
    let result = if phase >= THREE_EIGHTHS {
        if phase >= FIVE_EIGHTHS {
            let x = (phase - THREE_QUARTERS) * TAU;
            -cos_polynomial(x * x)
        } else {
            let x = (phase - ONE_HALF) * TAU;
            -sin_polynomial(x)
        }
    } else if phase >= ONE_EIGHTH {
        let x = (phase - ONE_QUARTER) * TAU;
        cos_polynomial(x * x)
    } else {
        let x = phase * TAU;
        sin_polynomial(x)
    };
    result as f32
}

fn reduce_phase(value: f64) -> f64 {
    let mut phase = value * INV_TAU;
    phase += ONE_EIGHTH;
    let integer = cvtt_f64_to_i32(phase);
    phase -= f64::from(integer);
    phase -= ONE_EIGHTH;
    phase
}

fn sin_polynomial(x: f64) -> f64 {
    let square = x * x;
    let mut result = square * SIN_C1;
    result -= SIN_C2;
    result *= square;
    result += SIN_C3;
    result *= square;
    result -= SIN_C4;
    result *= square;
    result += ONE;
    result * x
}

fn cos_polynomial(square: f64) -> f64 {
    let mut result = square * COS_C1;
    result -= COS_C2;
    result *= square;
    result += COS_C3;
    result *= square;
    result -= ONE_HALF;
    result *= square;
    result + ONE
}

fn cvtt_f64_to_i32(value: f64) -> i32 {
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
    fn game_trig_cardinal_turns_match_expected_bits() {
        assert_eq!(game_cos_f32(0.0).to_bits(), 1.0f32.to_bits());
        assert_eq!(game_sin_f32(0.0).to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn local_matrix_applies_game_translation_scale_and_2d_sign() {
        let transform = SpatialTransform {
            translation: [10.0, 20.0, 30.0],
            rotation: [0, 0, 16_384],
            scale: [2.0, 3.0, 4.0],
            multiply_color: [255; 4],
            additive_color: [0; 4],
            visibility_word: 1,
        };
        let matrix = build_local_matrix(&transform, true, true, [5.0, 7.0]);
        assert_eq!(matrix.rows[0][3], 15.0);
        assert_eq!(matrix.rows[1][3], -27.0);
        assert_eq!(matrix.rows[2][3], 30.0);
        assert!(matrix.rows[0][1] > 0.0);
        assert!(matrix.rows[1][0] < 0.0);
        assert_eq!(matrix.rows[2][2], 4.0);
    }

    #[test]
    fn affine_product_is_parent_times_local() {
        let parent = Affine3x4 {
            rows: [
                [1.0, 0.0, 0.0, 10.0],
                [0.0, 1.0, 0.0, 20.0],
                [0.0, 0.0, 1.0, 30.0],
            ],
        };
        let local = Affine3x4 {
            rows: [
                [2.0, 0.0, 0.0, 1.0],
                [0.0, 3.0, 0.0, 2.0],
                [0.0, 0.0, 4.0, 3.0],
            ],
        };
        assert_eq!(
            parent.mul_game(local),
            Affine3x4 {
                rows: [
                    [2.0, 0.0, 0.0, 11.0],
                    [0.0, 3.0, 0.0, 22.0],
                    [0.0, 0.0, 4.0, 33.0],
                ]
            }
        );
    }

    #[test]
    fn animation_writes_raw_scalar_bits_like_the_game() {
        let mut transform = SpatialTransform::default();
        assert!(transform.apply_common_track(0, Evaluation::Value(ScalarValue::I32(0x3f80_0000))));
        assert_eq!(transform.translation[0], 1.0);
        assert!(
            transform
                .apply_common_track(3, Evaluation::Value(ScalarValue::F32(f32::from_bits(25))))
        );
        assert_eq!(transform.rotation[0], 25);
    }

    #[test]
    fn color_channels_follow_the_binary_component_setters() {
        let mut transform = SpatialTransform::default();
        assert!(transform.apply_common_track(
            9,
            Evaluation::Value(ScalarValue::Bytes4([0x11, 0x22, 0x33, 0x44]))
        ));
        assert_eq!(transform.multiply_color, [0x33, 0x22, 0x11, 0xff]);

        assert!(transform.apply_common_track(19, Evaluation::Value(ScalarValue::I32(0x1020_3040))));
        assert_eq!(transform.additive_color, [0x20, 0x30, 0x40, 0]);

        assert!(transform.apply_common_track(21, Evaluation::Value(ScalarValue::F32(0.5))));
        assert_eq!(transform.multiply_color[3], 127);
        assert!(transform.apply_common_track(22, Evaluation::Value(ScalarValue::F32(f32::NAN))));
        assert_eq!(transform.additive_color[3], 0);
        assert!(transform.apply_common_track(21, Evaluation::Unchanged));
        assert_eq!(transform.multiply_color[3], 255);
    }
}
