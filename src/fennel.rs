use crate::projection::Matrix4x4;
use crate::render::{CeylonDrawPacketPresetState, CeylonShaderKey, CeylonShaderKeyInput};
use crate::ruhuna::{RuhunaRuntimeFont, RuhunaRuntimeGlyphRecord};
use crate::text::{TextDefinition, fennel_alignment_code_from_text_flags};

/// The FontManager initialization at `0x7B719C..0x7B71A4` passes encoding
/// mode zero and 0x20 font slots. `font::TextBoxObject::setTextByString`
/// (`sub_7C8F40`) dispatches mode zero to `sub_F525D0`, the game's UTF-8 to
/// UTF-16 converter.
pub const FENNEL_GAME_ENCODING_UTF8: u32 = 0;

/// `sub_F32B40` stores this value at FontManager implementation offset +0x08;
/// `sub_F321E0` returns it to the token iterator as the command prefix.
pub const FENNEL_CONTROL_PREFIX: u16 = b'$' as u16;
const FENNEL_NEWLINE_COMMAND_UPPER: u16 = b'N' as u16;
const FENNEL_NEWLINE_COMMAND_LOWER: u16 = b'n' as u16;
const FENNEL_POSITION_COMMAND_UPPER: u16 = b'T' as u16;
const FENNEL_POSITION_COMMAND_LOWER: u16 = b't' as u16;

pub const FENNEL_RECORD_LINE_END: i32 = -1;
pub const FENNEL_RECORD_ZERO_WIDTH_GLYPH: i32 = -2;
pub const FENNEL_RECORD_LINE_TABLE_OVERFLOW: i32 = -0xFE;
pub const FENNEL_RECORD_STREAM_END: i32 = -0xFF;
pub const FENNEL_LINE_START_CAPACITY: usize = 0x80;
pub const FENNEL_UV_BIAS: f32 = f32::from_bits(0x3727_C5AC);
pub const FENNEL_TRIANGLE_CORNER_INDICES: [usize; 6] = [1, 0, 2, 3, 1, 2];

/// The 45 UTF-16 values copied from `word_1940AB8` into the FontManager
/// implementation's `+0xE0` lookup by `sub_F33C30`.
pub const FENNEL_FONT_MANAGER_SET_E0: [u16; 45] = [
    0x0029, 0x002C, 0x002E, 0x2019, 0x201D, 0x2025, 0x2026, 0x226B, 0x3001, 0x3002, 0x3009, 0x300B,
    0x300D, 0x300F, 0x3011, 0x3015, 0x3041, 0x3043, 0x3045, 0x3047, 0x3049, 0x3063, 0x3083, 0x3085,
    0x3087, 0x309B, 0x309C, 0x30A1, 0x30A3, 0x30A5, 0x30A7, 0x30A9, 0x30C3, 0x30E3, 0x30E5, 0x30E7,
    0x30FC, 0xFF01, 0xFF09, 0xFF0C, 0xFF0E, 0xFF1F, 0xFF3D, 0xFF5D, 0xFF5E,
];

/// The 14 UTF-16 values copied from `word_1940A9C` into the FontManager
/// implementation's `+0xE4` lookup by `sub_F33C30`.
pub const FENNEL_FONT_MANAGER_SET_E4: [u16; 14] = [
    0x0025, 0x0028, 0x2018, 0x201C, 0x226A, 0x3008, 0x300A, 0x300C, 0x300E, 0x3010, 0x3014, 0xFF08,
    0xFF3B, 0xFF5B,
];

/// Exact membership queried by `sub_F38DB0` from FontManager `+0xE0`.
pub fn fennel_font_manager_set_e0_contains(code: u16) -> bool {
    FENNEL_FONT_MANAGER_SET_E0.binary_search(&code).is_ok()
}

/// Exact membership queried by `sub_F38D20` from FontManager `+0xE4`.
pub fn fennel_font_manager_set_e4_contains(code: u16) -> bool {
    FENNEL_FONT_MANAGER_SET_E4.binary_search(&code).is_ok()
}

/// DrawPacket state constructed at `sub_6CD8A0`, then changed by
/// `teaFontRenderer` construction and `sub_7C7F90` for the default mode at
/// renderer offset +0x330. The latter selects packet bit 25 and binds exactly
/// one texture at packet +0x30.
pub const FENNEL_DEFAULT_DRAW_FLAGS_00: u32 = 0x02AF_E003;
pub const FENNEL_DEFAULT_FLAGS_60: u32 = 0x0000_4020;
pub const FENNEL_VERTEX_FORMAT: u32 = 13;

pub const fn fennel_default_draw_packet(is_2d: bool) -> CeylonDrawPacketPresetState {
    CeylonDrawPacketPresetState {
        draw_flags_00: FENNEL_DEFAULT_DRAW_FLAGS_00,
        packed_08: 0,
        flags_0c: 0,
        field_2c: 0,
        flags_58: 0xff,
        flags_60: FENNEL_DEFAULT_FLAGS_60 | ((is_2d as u32) << 7),
    }
}

/// Reproduces the ShapeEnv key formed by a normal one-atlas Fennel batch.
/// `is_2d` is the explicit boolean copied by `sub_6DF020` to DrawPacket +0x60
/// bit 7; it is supplied by the TextBox draw caller rather than inferred.
pub fn fennel_default_shader_key(is_2d: bool) -> CeylonShaderKey {
    CeylonShaderKeyInput {
        draw_flags_00: FENNEL_DEFAULT_DRAW_FLAGS_00,
        field_28: 31,
        field_2c: 0,
        flags_60: FENNEL_DEFAULT_FLAGS_60 | (u32::from(is_2d) << 7),
        vertex_format_70: FENNEL_VERTEX_FORMAT,
        texture_present: [true, false, false],
    }
    .shader_key()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FennelTextDecodeError {
    pub valid_prefix_len: usize,
}

impl std::fmt::Display for FennelTextDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Fennel UTF-8 text becomes invalid at byte {}",
            self.valid_prefix_len
        )
    }
}

impl std::error::Error for FennelTextDecodeError {}

/// Decodes the valid-input domain of the game's encoding-mode-zero path.
///
/// The source is a C string, so bytes after the first NUL are ignored. On
/// valid UTF-8 this produces the same UTF-16 code units, including surrogate
/// pairs for supplementary characters, as `sub_F525D0`. Invalid byte handling
/// remains explicit instead of silently choosing a replacement policy.
pub fn decode_fennel_game_text(bytes: &[u8]) -> Result<Vec<u16>, FennelTextDecodeError> {
    let c_string = &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())];
    let text = std::str::from_utf8(c_string).map_err(|error| FennelTextDecodeError {
        valid_prefix_len: error.valid_up_to(),
    })?;
    Ok(text.encode_utf16().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FennelPlainToken {
    Glyph(u16),
    NewLine,
    SetPosition { x: i32, y: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedFennelControl {
    pub unit_index: usize,
    pub command: Option<u16>,
}

impl std::fmt::Display for UnsupportedFennelControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.command {
            Some(command) => write!(
                formatter,
                "unsupported Fennel control ${command:04X} at UTF-16 unit {}",
                self.unit_index
            ),
            None => write!(
                formatter,
                "unterminated Fennel control prefix at UTF-16 unit {}",
                self.unit_index
            ),
        }
    }
}

impl std::error::Error for UnsupportedFennelControl {}

/// Reproduces the evidence-complete plain-token subset of `sub_F3BD40`.
///
/// Direct UTF-16 units produce type-zero glyph tokens. CRLF, CR, LF and the
/// proven `$N`/`$n` aliases produce type-three newline tokens. `$$` produces a
/// literal dollar glyph. The `$t[x:y]` branch writes signed decimal x/y values
/// to iterator/output offsets +0x20/+0x24. Other commands are rejected until
/// their parameter grammar and state writes are closed from the binary.
pub fn tokenize_fennel_plain_text(
    units: &[u16],
) -> Result<Vec<FennelPlainToken>, UnsupportedFennelControl> {
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < units.len() {
        let unit = units[index];
        if unit == 0 || unit == 0x1A {
            break;
        }
        match unit {
            0x0D => {
                if units.get(index + 1) == Some(&0x0A) {
                    index += 1;
                }
                tokens.push(FennelPlainToken::NewLine);
            }
            0x0A => tokens.push(FennelPlainToken::NewLine),
            FENNEL_CONTROL_PREFIX => {
                let Some(&command) = units.get(index + 1) else {
                    break;
                };
                match command {
                    FENNEL_CONTROL_PREFIX => {
                        tokens.push(FennelPlainToken::Glyph(FENNEL_CONTROL_PREFIX));
                        index += 1;
                    }
                    FENNEL_NEWLINE_COMMAND_UPPER | FENNEL_NEWLINE_COMMAND_LOWER => {
                        tokens.push(FennelPlainToken::NewLine);
                        index += 1;
                    }
                    FENNEL_POSITION_COMMAND_UPPER | FENNEL_POSITION_COMMAND_LOWER => {
                        let Some((x, y, end_index)) = parse_fennel_position_control(units, index)
                        else {
                            return Err(UnsupportedFennelControl {
                                unit_index: index,
                                command: Some(command),
                            });
                        };
                        tokens.push(FennelPlainToken::SetPosition { x, y });
                        index = end_index;
                    }
                    _ => {
                        return Err(UnsupportedFennelControl {
                            unit_index: index,
                            command: Some(command),
                        });
                    }
                }
            }
            _ => tokens.push(FennelPlainToken::Glyph(unit)),
        }
        index += 1;
    }
    Ok(tokens)
}

fn parse_fennel_position_control(units: &[u16], prefix_index: usize) -> Option<(i32, i32, usize)> {
    let open_index = prefix_index.checked_add(2)?;
    if units.get(open_index) != Some(&(b'[' as u16)) {
        return None;
    }
    let colon_index = units[open_index + 1..]
        .iter()
        .position(|unit| *unit == b':' as u16)?
        + open_index
        + 1;
    let close_index = units[colon_index + 1..]
        .iter()
        .position(|unit| *unit == b']' as u16)?
        + colon_index
        + 1;
    let x = parse_fennel_signed_decimal(&units[open_index + 1..colon_index])?;
    let y = parse_fennel_signed_decimal(&units[colon_index + 1..close_index])?;
    Some((x, y, close_index))
}

fn parse_fennel_signed_decimal(units: &[u16]) -> Option<i32> {
    let (negative, digits) = match units {
        [minus, rest @ ..] if *minus == b'-' as u16 => (true, rest),
        _ => (false, units),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value = 0i32;
    for &unit in digits {
        if !(b'0' as u16..=b'9' as u16).contains(&unit) {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(i32::from(unit - b'0' as u16))?;
    }
    negative.then_some(value.checked_neg()?).or(Some(value))
}

#[derive(Debug, Clone, PartialEq)]
pub struct FennelPlainRecordStream {
    pub records: Vec<FennelGlyphLayoutRecord>,
    pub line_start_indices: Vec<usize>,
    pub glyph_count: usize,
}

/// Runtime-glyph fields read by the default horizontal layout routine
/// `sub_7C1F90` when TextBoxObject flag `0x200` is clear. Static SRD TextCast
/// initialization produces flags `3` or `7`, so that alternate metric mode is
/// not part of this evidence-complete subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FennelLayoutGlyphMetrics {
    pub code: u16,
    pub bearing_x: i32,
    pub width: u32,
    pub advance_x: u32,
    pub em_pixels_y: i32,
}

impl From<&RuhunaRuntimeGlyphRecord> for FennelLayoutGlyphMetrics {
    fn from(glyph: &RuhunaRuntimeGlyphRecord) -> Self {
        Self {
            code: glyph.code,
            bearing_x: glyph.bearing_x,
            width: glyph.width,
            advance_x: glyph.advance_x,
            em_pixels_y: glyph.em_pixels_y,
        }
    }
}

/// Inputs copied by `sub_AC6F50` into the TextBoxObject before the static SRD
/// TextCast path calls `setTextByWideString`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FennelStaticLayoutInput {
    pub text_flags: u32,
    pub box_width: f32,
    pub box_height: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub line_spacing: i32,
}

/// Static SRD values whose source is closed from `sub_AC6F50`, the
/// TextBoxObject constructors, and the normal-token branch of `sub_F3BD40`.
///
/// The iterator starts x/y at zero. Its output `+0x6C`, copied to layout-record
/// `+0x20`, comes from TEXT property `0x7C`. The remaining values are copied to
/// the TextBoxObject by the SrTextCast initializer. Missing properties are
/// rejected because their serialized default behavior has not been proven.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FennelStaticTextProperties {
    pub initial_x: i32,
    pub initial_y: i32,
    pub glyph_spacing: i32,
    pub layout: FennelStaticLayoutInput,
}

impl FennelStaticTextProperties {
    pub fn from_text_definition(
        text: &TextDefinition,
        box_width: f32,
        box_height: f32,
    ) -> Result<Self, FennelStaticTextPropertyError> {
        let text_flags = text
            .field_78
            .ok_or(FennelStaticTextPropertyError::MissingProperty { code: 0x78 })?;
        let [scale_x, scale_y] = text
            .field_36
            .ok_or(FennelStaticTextPropertyError::MissingProperty { code: 0x36 })?;
        let glyph_spacing = i32::from(
            text.field_7c
                .ok_or(FennelStaticTextPropertyError::MissingProperty { code: 0x7C })?,
        );
        let line_spacing = i32::from(
            text.field_41
                .ok_or(FennelStaticTextPropertyError::MissingProperty { code: 0x41 })?,
        );
        Ok(Self {
            initial_x: 0,
            initial_y: 0,
            glyph_spacing,
            layout: FennelStaticLayoutInput {
                text_flags,
                box_width,
                box_height,
                scale_x,
                scale_y,
                line_spacing,
            },
        })
    }

    pub const fn glyph_placement(
        self,
        glyph_token: u32,
        field_0c: u32,
        colors: [u32; 4],
    ) -> FennelGlyphPlacementInput {
        FennelGlyphPlacementInput {
            glyph_token,
            field_0c,
            x: self.initial_x,
            y: self.initial_y,
            field_20: self.glyph_spacing,
            colors,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FennelStaticTextPropertyError {
    MissingProperty { code: u8 },
}

impl std::fmt::Display for FennelStaticTextPropertyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProperty { code } => {
                write!(
                    formatter,
                    "static Fennel TEXT property {code:#04x} is absent"
                )
            }
        }
    }
}

impl std::error::Error for FennelStaticTextPropertyError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FennelFittingLayoutResult {
    pub effective_scale_x: f32,
    pub effective_scale_y: f32,
    pub total_height: f32,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FennelFittingLayoutError {
    InvalidLineTable,
    UnsupportedRecordKind {
        record_index: usize,
        kind: i32,
    },
    MissingGlyphMetrics {
        record_index: usize,
        glyph_token: u32,
    },
    HorizontalWrapRequired {
        line_index: usize,
        record_index: usize,
    },
    VerticalOverflow {
        line_index: usize,
    },
}

impl std::fmt::Display for FennelFittingLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLineTable => formatter.write_str("invalid Fennel line-start table"),
            Self::UnsupportedRecordKind { record_index, kind } => write!(
                formatter,
                "unsupported Fennel record kind {kind} at record {record_index}"
            ),
            Self::MissingGlyphMetrics {
                record_index,
                glyph_token,
            } => write!(
                formatter,
                "no Fennel glyph metrics for token {glyph_token:#010x} at record {record_index}"
            ),
            Self::HorizontalWrapRequired {
                line_index,
                record_index,
            } => write!(
                formatter,
                "Fennel line {line_index} requires the not-yet-implemented wrap branch at record {record_index}"
            ),
            Self::VerticalOverflow { line_index } => write!(
                formatter,
                "Fennel line {line_index} reaches the game's vertical-overflow branch"
            ),
        }
    }
}

impl std::error::Error for FennelFittingLayoutError {}

#[derive(Debug, Clone, Copy)]
struct FennelMeasuredLine {
    start: usize,
    end: usize,
    x_offset: f32,
    y_bottom: f32,
}

/// Reproduces the default `sub_7C1F90` path while every explicit line fits
/// horizontally and vertically.
///
/// The function is deliberately atomic: it first measures every line and only
/// mutates the records after proving that none enters the game's automatic
/// wrapping, kinsoku, word-break, or vertical-overflow branches. Those paths
/// return an explicit error until their complete binary behavior is ported.
pub fn layout_fennel_static_fitting_lines<F>(
    stream: &mut FennelPlainRecordStream,
    input: FennelStaticLayoutInput,
    mut glyph_metrics: F,
) -> Result<FennelFittingLayoutResult, FennelFittingLayoutError>
where
    F: FnMut(u32) -> Option<FennelLayoutGlyphMetrics>,
{
    let line_count = stream
        .line_start_indices
        .len()
        .checked_sub(1)
        .ok_or(FennelFittingLayoutError::InvalidLineTable)?;
    if line_count == 0 {
        return Err(FennelFittingLayoutError::InvalidLineTable);
    }
    let Some(&stream_end_index) = stream.line_start_indices.last() else {
        return Err(FennelFittingLayoutError::InvalidLineTable);
    };
    if stream_end_index >= stream.records.len()
        || stream.records[stream_end_index].kind != FENNEL_RECORD_STREAM_END
    {
        return Err(FennelFittingLayoutError::InvalidLineTable);
    }

    let mut metrics = vec![None; stream.records.len()];
    let mut line_ranges = Vec::with_capacity(line_count);
    for line_index in 0..line_count {
        let start = stream.line_start_indices[line_index];
        let next_start = stream.line_start_indices[line_index + 1];
        let Some(end) = next_start.checked_sub(1) else {
            return Err(FennelFittingLayoutError::InvalidLineTable);
        };
        if start > end
            || end >= stream.records.len()
            || stream.records[end].kind != FENNEL_RECORD_LINE_END
        {
            return Err(FennelFittingLayoutError::InvalidLineTable);
        }
        for record_index in start..end {
            let record = &stream.records[record_index];
            if record.kind != 0 && record.kind != FENNEL_RECORD_ZERO_WIDTH_GLYPH {
                return Err(FennelFittingLayoutError::UnsupportedRecordKind {
                    record_index,
                    kind: record.kind,
                });
            }
            metrics[record_index] = Some(glyph_metrics(record.glyph_token).ok_or(
                FennelFittingLayoutError::MissingGlyphMetrics {
                    record_index,
                    glyph_token: record.glyph_token,
                },
            )?);
        }
        line_ranges.push((start, end));
    }

    // TextBoxObject constructs +0x2E0 as 3. `sub_AC6F50` clears the mode
    // groups, and static state mode zero adds bit 2 only when TEXT bit 0 is
    // clear. Thus static SRD input reaches this routine with exactly 3 or 7.
    let static_layout_flags = if input.text_flags & 1 == 0 {
        7u32
    } else {
        3u32
    };
    let mut effective_scale_x = input.scale_x;
    let effective_scale_y = input.scale_y;
    let mut scale_x_multiplier = 1.0f32;

    if static_layout_flags & 4 != 0
        && !fennel_float_nearly_equal(input.scale_x, 0.0)
        && !fennel_float_nearly_equal(input.scale_y, 0.0)
    {
        let (first_start, first_end) = line_ranges[0];
        let mut advance = 0.0f32;
        let mut visual_extent = 0.0f32;
        for record_index in first_start..first_end {
            let record = &stream.records[record_index];
            let metric = metrics[record_index].expect("metrics were populated above");
            visual_extent = fennel_visual_width(metric) * input.scale_x + advance;
            advance += fennel_advance(record, metric, input.scale_x);
        }
        if visual_extent > input.box_width {
            effective_scale_x =
                ((input.scale_x * input.box_width) / visual_extent) * FENNEL_AUTO_FIT_BIAS;
            scale_x_multiplier = effective_scale_x / input.scale_x;
        }
    }

    let fallback_line_height = stream
        .records
        .iter()
        .enumerate()
        .take(stream_end_index)
        .find(|(_, record)| record.kind >= 0)
        .and_then(|(index, _)| metrics[index])
        .map(|metric| (metric.em_pixels_y as f32) * effective_scale_y)
        .unwrap_or(0.0);

    let horizontal_alignment = fennel_alignment_code_from_text_flags(input.text_flags) % 3;
    let mut measured_lines = Vec::with_capacity(line_count);
    let mut current_y = 0.0f32;
    for (line_index, &(start, end)) in line_ranges.iter().enumerate() {
        let mut advance = 0.0f32;
        let mut visual_extent = 0.0f32;
        let mut line_height = 0.0f32;
        for record_index in start..end {
            let record = &stream.records[record_index];
            let metric = metrics[record_index].expect("metrics were populated above");
            let glyph_visual_end = fennel_visual_width(metric) * effective_scale_x + advance;
            if glyph_visual_end > input.box_width {
                return Err(FennelFittingLayoutError::HorizontalWrapRequired {
                    line_index,
                    record_index,
                });
            }
            visual_extent = glyph_visual_end;
            advance += fennel_advance(record, metric, effective_scale_x);
            let glyph_height = (metric.em_pixels_y as f32) * effective_scale_y;
            if glyph_height > line_height {
                line_height = glyph_height;
            }
        }
        if line_height == 0.0 {
            line_height = fallback_line_height;
        }
        if current_y.abs() + line_height > input.box_height {
            return Err(FennelFittingLayoutError::VerticalOverflow { line_index });
        }
        let x_offset = match horizontal_alignment {
            0 => 0.0,
            1 => fennel_cvttss2si_as_f32((input.box_width - visual_extent) * 0.5),
            2 => input.box_width - visual_extent,
            _ => unreachable!(),
        };
        let line_advance = if line_index == 0 {
            line_height
        } else {
            line_height + input.line_spacing as f32
        };
        measured_lines.push(FennelMeasuredLine {
            start,
            end,
            x_offset,
            y_bottom: current_y + line_advance,
        });
        current_y += line_advance;
    }

    let alignment_code = fennel_alignment_code_from_text_flags(input.text_flags);
    let vertical_offset = match alignment_code / 3 {
        0 => 0.0,
        1 => fennel_cvttss2si_as_f32((input.box_height - current_y) * 0.5),
        2 => input.box_height - current_y,
        _ => unreachable!(),
    };

    for line in &measured_lines {
        let mut advance = 0.0f32;
        for record_index in line.start..line.end {
            let record = &mut stream.records[record_index];
            let metric = metrics[record_index].expect("metrics were populated above");
            record.x += line.x_offset + advance;
            record.y += line.y_bottom + vertical_offset;
            record.scale_x *= scale_x_multiplier;
            advance += fennel_advance(record, metric, effective_scale_x);
        }
    }

    Ok(FennelFittingLayoutResult {
        effective_scale_x,
        effective_scale_y,
        total_height: current_y,
        line_count,
    })
}

const FENNEL_AUTO_FIT_BIAS: f32 = f32::from_bits(0x3F7F_BE77);
const FENNEL_FLOAT_EQUAL_EPSILON: f32 = f32::from_bits(0x3400_0000);

fn fennel_float_nearly_equal(left: f32, right: f32) -> bool {
    (left - right).abs() <= FENNEL_FLOAT_EQUAL_EPSILON
}

fn fennel_visual_width(metric: FennelLayoutGlyphMetrics) -> f32 {
    metric.bearing_x.wrapping_add(metric.width as i32) as f32
}

fn fennel_advance(
    record: &FennelGlyphLayoutRecord,
    metric: FennelLayoutGlyphMetrics,
    scale_x: f32,
) -> f32 {
    (record.field_20 + (metric.advance_x as i32) as f32) * scale_x
}

fn fennel_cvttss2si_as_f32(value: f32) -> f32 {
    let converted = if value.is_nan() || !(-2_147_483_648.0f32..2_147_483_648.0f32).contains(&value)
    {
        i32::MIN
    } else {
        value.trunc() as i32
    };
    converted as f32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FennelPlainRecordError {
    Decode(FennelTextDecodeError),
    Control(UnsupportedFennelControl),
    MissingGlyph { code: u16 },
    RecordCapacity { capacity: usize, required: usize },
}

impl std::fmt::Display for FennelPlainRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::Control(error) => error.fmt(formatter),
            Self::MissingGlyph { code } => {
                write!(formatter, "Ruhuna font has no glyph for U+{code:04X}")
            }
            Self::RecordCapacity { capacity, required } => write!(
                formatter,
                "Fennel record capacity {capacity} is smaller than required {required}"
            ),
        }
    }
}

impl std::error::Error for FennelPlainRecordError {}

/// Builds the pre-layout record stream produced by the proven plain-token
/// subset of `sub_7C90A0`.
///
/// The caller supplies the iterator-derived placement state and an opaque
/// token for each runtime glyph. Missing glyphs are rejected because the
/// game's `fennel_npc` EmbeddedSprite fallback has not yet been reproduced.
pub fn build_fennel_plain_record_stream<F>(
    bytes: &[u8],
    font: &RuhunaRuntimeFont,
    placement: FennelGlyphPlacementInput,
    record_capacity: usize,
    mut glyph_token: F,
) -> Result<FennelPlainRecordStream, FennelPlainRecordError>
where
    F: FnMut(u16, &RuhunaRuntimeGlyphRecord) -> u32,
{
    let units = decode_fennel_game_text(bytes).map_err(FennelPlainRecordError::Decode)?;
    let tokens = tokenize_fennel_plain_text(&units).map_err(FennelPlainRecordError::Control)?;
    let required = tokens
        .iter()
        .filter(|token| !matches!(token, FennelPlainToken::SetPosition { .. }))
        .count()
        .checked_add(2)
        .unwrap_or(usize::MAX);
    if required > record_capacity {
        return Err(FennelPlainRecordError::RecordCapacity {
            capacity: record_capacity,
            required,
        });
    }

    let mut records = Vec::with_capacity(required);
    let mut line_start_indices = vec![0];
    let mut glyph_count = 0usize;
    let mut current_placement = placement;
    for token in tokens {
        match token {
            FennelPlainToken::Glyph(code) => {
                let glyph = font
                    .glyph(code)
                    .ok_or(FennelPlainRecordError::MissingGlyph { code })?;
                let mut glyph_placement = current_placement;
                glyph_placement.glyph_token = glyph_token(code, glyph);
                records.push(FennelGlyphLayoutRecord::from_runtime_glyph(
                    glyph,
                    glyph_placement,
                ));
                glyph_count += 1;
            }
            FennelPlainToken::NewLine => {
                let kind = if line_start_indices.len() < FENNEL_LINE_START_CAPACITY {
                    FENNEL_RECORD_LINE_END
                } else {
                    FENNEL_RECORD_LINE_TABLE_OVERFLOW
                };
                records.push(FennelGlyphLayoutRecord {
                    kind,
                    ..Default::default()
                });
                if line_start_indices.len() < FENNEL_LINE_START_CAPACITY {
                    line_start_indices.push(records.len());
                }
            }
            FennelPlainToken::SetPosition { x, y } => {
                current_placement.x = x;
                current_placement.y = y;
            }
        }
    }

    records.push(FennelGlyphLayoutRecord {
        kind: FENNEL_RECORD_LINE_END,
        ..Default::default()
    });
    if line_start_indices.len() < FENNEL_LINE_START_CAPACITY {
        line_start_indices.push(records.len());
    }
    records.push(FennelGlyphLayoutRecord {
        kind: FENNEL_RECORD_STREAM_END,
        ..Default::default()
    });

    Ok(FennelPlainRecordStream {
        records,
        line_start_indices,
        glyph_count,
    })
}

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

/// Exact 28-byte vertex written by `sub_7C10B0` for Fennel glyph triangles.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FennelRenderVertex {
    pub position: [f32; 3],
    pub primary_color_bgra: [u8; 4],
    pub secondary_color_bgra: [u8; 4],
    pub texture_coordinates: [f32; 2],
}

/// Reproduces the non-clipping branch (`TextBoxObject flags & 0x400 == 0`) of
/// `sub_7C10B0` for one already-positioned glyph record.
pub fn build_fennel_unclipped_vertices(
    record: &FennelGlyphLayoutRecord,
    origin: [f32; 2],
    transform: &Matrix4x4,
    secondary_color: u32,
) -> [FennelRenderVertex; 6] {
    let far_x = record.width * record.scale_x + origin[0] - record.field_2c;
    let far_y = record.height * record.scale_y + origin[1] - record.field_30;
    let near_x = record.field_24 + origin[0];
    let near_y = record.field_28 + origin[1];
    let positions = [
        transform_fennel_point(transform, near_x, near_y),
        transform_fennel_point(transform, far_x, near_y),
        transform_fennel_point(transform, near_x, far_y),
        transform_fennel_point(transform, far_x, far_y),
    ];
    let uvs = [record.uv0, record.uv1, record.uv2, record.uv3];
    let secondary_color_bgra = fennel_packed_color_to_bgra(secondary_color);

    FENNEL_TRIANGLE_CORNER_INDICES.map(|corner| FennelRenderVertex {
        position: positions[corner],
        primary_color_bgra: fennel_packed_color_to_bgra(record.colors[corner]),
        secondary_color_bgra,
        texture_coordinates: [
            uvs[corner][0] + FENNEL_UV_BIAS,
            uvs[corner][1] + FENNEL_UV_BIAS,
        ],
    })
}

fn transform_fennel_point(transform: &Matrix4x4, x: f32, y: f32) -> [f32; 3] {
    let rows = &transform.rows;
    let transformed_x = rows[0][0] * x + rows[0][1] * y + rows[0][3];
    let transformed_y = rows[1][0] * x + rows[1][1] * y + rows[1][3];
    let transformed_z = rows[2][0] * x + rows[2][1] * y + rows[2][3];
    let transformed_w = rows[3][0] * x + rows[3][1] * y + rows[3][3];
    [
        transformed_x / transformed_w,
        transformed_y / transformed_w,
        transformed_z / transformed_w,
    ]
}

fn fennel_packed_color_to_bgra(color: u32) -> [u8; 4] {
    let bytes = color.to_le_bytes();
    [bytes[2], bytes[1], bytes[0], bytes[3]]
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

impl FennelRenderVertex {
    pub const STRIDE: usize = 28;
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
                FENNEL_RECORD_ZERO_WIDTH_GLYPH,
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

    fn fitting_record(glyph_token: u32) -> FennelGlyphLayoutRecord {
        FennelGlyphLayoutRecord {
            glyph_token,
            scale_x: 1.0,
            scale_y: 1.0,
            ..Default::default()
        }
    }

    fn one_line_stream(tokens: &[u32]) -> FennelPlainRecordStream {
        let mut records = tokens
            .iter()
            .copied()
            .map(fitting_record)
            .collect::<Vec<_>>();
        records.push(FennelGlyphLayoutRecord {
            kind: FENNEL_RECORD_LINE_END,
            ..Default::default()
        });
        let stream_end_index = records.len();
        records.push(FennelGlyphLayoutRecord {
            kind: FENNEL_RECORD_STREAM_END,
            ..Default::default()
        });
        FennelPlainRecordStream {
            records,
            line_start_indices: vec![0, stream_end_index],
            glyph_count: tokens.len(),
        }
    }

    fn two_line_stream(first: &[u32], second: &[u32]) -> FennelPlainRecordStream {
        let mut records = first
            .iter()
            .copied()
            .map(fitting_record)
            .collect::<Vec<_>>();
        records.push(FennelGlyphLayoutRecord {
            kind: FENNEL_RECORD_LINE_END,
            ..Default::default()
        });
        let second_start = records.len();
        records.extend(second.iter().copied().map(fitting_record));
        records.push(FennelGlyphLayoutRecord {
            kind: FENNEL_RECORD_LINE_END,
            ..Default::default()
        });
        let stream_end_index = records.len();
        records.push(FennelGlyphLayoutRecord {
            kind: FENNEL_RECORD_STREAM_END,
            ..Default::default()
        });
        FennelPlainRecordStream {
            records,
            line_start_indices: vec![0, second_start, stream_end_index],
            glyph_count: first.len() + second.len(),
        }
    }

    fn metrics(token: u32) -> Option<FennelLayoutGlyphMetrics> {
        match token {
            1 => Some(FennelLayoutGlyphMetrics {
                code: b'A' as u16,
                bearing_x: 1,
                width: 8,
                advance_x: 10,
                em_pixels_y: 12,
            }),
            2 => Some(FennelLayoutGlyphMetrics {
                code: b'B' as u16,
                bearing_x: 2,
                width: 7,
                advance_x: 11,
                em_pixels_y: 12,
            }),
            _ => None,
        }
    }

    #[test]
    fn default_batch_shape_keys_and_simple_inputs_match_format_13() {
        let three_d = fennel_default_shader_key(false);
        let two_d = fennel_default_shader_key(true);
        assert_eq!(fennel_default_draw_packet(false).encoded_preset_id(), 3);
        assert_eq!(fennel_default_draw_packet(false).flags_60, 0x4020);
        assert_eq!(fennel_default_draw_packet(true).flags_60, 0x40A0);
        assert_eq!(three_d.low, 0x0036_BFB0);
        assert_eq!(three_d.high, 0);
        assert_eq!(two_d.low, 0x0036_BFB8);
        assert_eq!(two_d.high, 0);
        assert_eq!(
            three_d
                .srd_simple_shader_direct_contributions()
                .unwrap()
                .compact_key(),
            *b"AAMAAABAABGAAAAAAA"
        );
        assert_eq!(
            two_d
                .srd_simple_shader_direct_contributions()
                .unwrap()
                .compact_key(),
            *b"EAMAAABAABGAAAAAAA"
        );

        for (key, expects_2d) in [(three_d, false), (two_d, true)] {
            let bits = key.srd_simple_shader_direct_contributions().unwrap();
            assert!(!bits.contains(9));
            assert!(bits.contains(10));
            assert!(bits.contains(11));
            assert!(!bits.contains(12));
            assert!(bits.contains(36));
            assert!(!bits.contains(37));
            assert_eq!(bits.contains(2), expects_2d);
        }
    }

    #[test]
    fn font_manager_line_break_sets_match_the_binary_tables() {
        assert_eq!(FENNEL_FONT_MANAGER_SET_E0.len(), 45);
        assert_eq!(FENNEL_FONT_MANAGER_SET_E4.len(), 14);
        assert!(FENNEL_FONT_MANAGER_SET_E0.is_sorted());
        assert!(FENNEL_FONT_MANAGER_SET_E4.is_sorted());
        assert!(fennel_font_manager_set_e0_contains(0x3002));
        assert!(!fennel_font_manager_set_e0_contains(0x3008));
        assert!(fennel_font_manager_set_e4_contains(0x3008));
        assert!(!fennel_font_manager_set_e4_contains(0x3002));
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
        assert_eq!(std::mem::size_of::<FennelRenderVertex>(), 28);
        assert_eq!(std::mem::offset_of!(FennelRenderVertex, position), 0);
        assert_eq!(
            std::mem::offset_of!(FennelRenderVertex, primary_color_bgra),
            12
        );
        assert_eq!(
            std::mem::offset_of!(FennelRenderVertex, secondary_color_bgra),
            16
        );
        assert_eq!(
            std::mem::offset_of!(FennelRenderVertex, texture_coordinates),
            20
        );
    }

    #[test]
    fn fitting_layout_applies_center_alignment_and_binary_metric_order() {
        let mut stream = one_line_stream(&[1, 2]);
        let result = layout_fennel_static_fitting_lines(
            &mut stream,
            FennelStaticLayoutInput {
                text_flags: 0x15,
                box_width: 100.0,
                box_height: 40.0,
                scale_x: 1.0,
                scale_y: 1.0,
                line_spacing: 0,
            },
            metrics,
        )
        .unwrap();

        assert_eq!(result.effective_scale_x, 1.0);
        assert_eq!(result.effective_scale_y, 1.0);
        assert_eq!(result.total_height, 12.0);
        assert_eq!(result.line_count, 1);
        // Final visual extent is 10 advance + (2 bearing + 7 width) = 19.
        // cvttss2si((100 - 19) / 2) is 40; vertical center is 14.
        assert_eq!((stream.records[0].x, stream.records[0].y), (40.0, 26.0));
        assert_eq!((stream.records[1].x, stream.records[1].y), (50.0, 26.0));
    }

    #[test]
    fn fitting_layout_reproduces_first_line_auto_fit_bias() {
        let mut stream = one_line_stream(&[1]);
        let metric = FennelLayoutGlyphMetrics {
            code: b'A' as u16,
            bearing_x: 0,
            width: 20,
            advance_x: 20,
            em_pixels_y: 10,
        };
        let result = layout_fennel_static_fitting_lines(
            &mut stream,
            FennelStaticLayoutInput {
                text_flags: 0,
                box_width: 10.0,
                box_height: 20.0,
                scale_x: 1.0,
                scale_y: 1.0,
                line_spacing: 0,
            },
            |_| Some(metric),
        )
        .unwrap();

        let expected = ((1.0 * 10.0) / 20.0) * FENNEL_AUTO_FIT_BIAS;
        assert_eq!(result.effective_scale_x.to_bits(), expected.to_bits());
        assert_eq!(stream.records[0].scale_x.to_bits(), expected.to_bits());
        assert_eq!((stream.records[0].x, stream.records[0].y), (0.0, 10.0));
    }

    #[test]
    fn fitting_layout_inserts_line_spacing_between_explicit_lines() {
        let mut stream = two_line_stream(&[1], &[2]);
        let result = layout_fennel_static_fitting_lines(
            &mut stream,
            FennelStaticLayoutInput {
                text_flags: 0,
                box_width: 100.0,
                box_height: 100.0,
                scale_x: 1.0,
                scale_y: 1.0,
                line_spacing: 5,
            },
            metrics,
        )
        .unwrap();

        assert_eq!(result.total_height, 29.0);
        assert_eq!(stream.records[0].y, 12.0);
        assert_eq!(stream.records[2].y, 29.0);
    }

    #[test]
    fn static_text_properties_use_only_binary_proven_sources() {
        let text = TextDefinition {
            field_78: Some(0x24),
            font_index: Some(0),
            text: b"A".to_vec(),
            field_36: Some([0.75, 0.5]),
            field_7b: None,
            field_7c: Some(-2),
            field_41: Some(7),
        };
        let properties =
            FennelStaticTextProperties::from_text_definition(&text, 320.0, 80.0).unwrap();
        assert_eq!((properties.initial_x, properties.initial_y), (0, 0));
        assert_eq!(properties.glyph_spacing, -2);
        assert_eq!(properties.layout.text_flags, 0x24);
        assert_eq!(
            (properties.layout.scale_x, properties.layout.scale_y),
            (0.75, 0.5)
        );
        assert_eq!(properties.layout.line_spacing, 7);
        assert_eq!(
            properties.glyph_placement(9, 11, [1, 2, 3, 4]),
            FennelGlyphPlacementInput {
                glyph_token: 9,
                field_0c: 11,
                x: 0,
                y: 0,
                field_20: -2,
                colors: [1, 2, 3, 4],
            }
        );

        let mut missing = text;
        missing.field_7c = None;
        assert_eq!(
            FennelStaticTextProperties::from_text_definition(&missing, 320.0, 80.0).unwrap_err(),
            FennelStaticTextPropertyError::MissingProperty { code: 0x7C }
        );
    }

    #[test]
    fn fitting_layout_rejects_wrap_atomically() {
        let mut stream = one_line_stream(&[1, 2]);
        stream.records[0].x = 3.0;
        let before = stream.clone();
        assert_eq!(
            layout_fennel_static_fitting_lines(
                &mut stream,
                FennelStaticLayoutInput {
                    text_flags: 1,
                    box_width: 8.0,
                    box_height: 40.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    line_spacing: 0,
                },
                metrics,
            )
            .unwrap_err(),
            FennelFittingLayoutError::HorizontalWrapRequired {
                line_index: 0,
                record_index: 0,
            }
        );
        assert_eq!(stream, before);
    }

    #[test]
    fn builds_unclipped_six_vertex_glyph_triangles() {
        let record = FennelGlyphLayoutRecord {
            width: 4.0,
            height: 6.0,
            field_24: 1.0,
            field_28: 2.0,
            field_2c: 0.5,
            field_30: 1.0,
            colors: [0x4433_2211, 0x8877_6655, 0xCCBB_AA99, 0xFFEE_DDCC],
            scale_x: 2.0,
            scale_y: 3.0,
            uv0: [0.0, 0.1],
            uv1: [0.2, 0.3],
            uv2: [0.4, 0.5],
            uv3: [0.6, 0.7],
            ..Default::default()
        };
        let vertices = build_fennel_unclipped_vertices(
            &record,
            [10.0, 20.0],
            &crate::projection::identity_matrix4x4_game(),
            0xA4A3_A2A1,
        );

        assert_eq!(
            vertices.map(|vertex| vertex.position),
            [
                [17.5, 22.0, 0.0],
                [11.0, 22.0, 0.0],
                [11.0, 37.0, 0.0],
                [17.5, 37.0, 0.0],
                [17.5, 22.0, 0.0],
                [11.0, 37.0, 0.0],
            ]
        );
        assert_eq!(vertices[0].primary_color_bgra, [0x77, 0x66, 0x55, 0x88]);
        assert_eq!(vertices[0].secondary_color_bgra, [0xA3, 0xA2, 0xA1, 0xA4]);
        assert_eq!(
            vertices.map(|vertex| vertex.texture_coordinates),
            [
                [0.2 + FENNEL_UV_BIAS, 0.3 + FENNEL_UV_BIAS],
                [FENNEL_UV_BIAS, 0.1 + FENNEL_UV_BIAS],
                [0.4 + FENNEL_UV_BIAS, 0.5 + FENNEL_UV_BIAS],
                [0.6 + FENNEL_UV_BIAS, 0.7 + FENNEL_UV_BIAS],
                [0.2 + FENNEL_UV_BIAS, 0.3 + FENNEL_UV_BIAS],
                [0.4 + FENNEL_UV_BIAS, 0.5 + FENNEL_UV_BIAS],
            ]
        );
    }

    #[test]
    fn decodes_the_games_utf8_mode_to_utf16() {
        assert_eq!(FENNEL_GAME_ENCODING_UTF8, 0);
        assert_eq!(FENNEL_CONTROL_PREFIX, 0x24);
        assert_eq!(
            decode_fennel_game_text("A中😀".as_bytes()).unwrap(),
            vec![0x0041, 0x4E2D, 0xD83D, 0xDE00]
        );
        assert_eq!(
            decode_fennel_game_text(b"before\0ignored").unwrap(),
            "before".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            decode_fennel_game_text(&[b'A', 0xFF]).unwrap_err(),
            FennelTextDecodeError {
                valid_prefix_len: 1
            }
        );
    }

    #[test]
    fn tokenizes_proven_plain_glyph_and_newline_forms() {
        let units = "A\r\nB\rC\nD$$E$NF$nG".encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            tokenize_fennel_plain_text(&units).unwrap(),
            vec![
                FennelPlainToken::Glyph(b'A' as u16),
                FennelPlainToken::NewLine,
                FennelPlainToken::Glyph(b'B' as u16),
                FennelPlainToken::NewLine,
                FennelPlainToken::Glyph(b'C' as u16),
                FennelPlainToken::NewLine,
                FennelPlainToken::Glyph(b'D' as u16),
                FennelPlainToken::Glyph(b'$' as u16),
                FennelPlainToken::Glyph(b'E' as u16),
                FennelPlainToken::NewLine,
                FennelPlainToken::Glyph(b'F' as u16),
                FennelPlainToken::NewLine,
                FennelPlainToken::Glyph(b'G' as u16),
            ]
        );
    }

    #[test]
    fn rejects_unclosed_control_grammars() {
        let units = "$C[FFFFFFFF]".encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            tokenize_fennel_plain_text(&units).unwrap_err(),
            UnsupportedFennelControl {
                unit_index: 0,
                command: Some(b'C' as u16),
            }
        );
        assert!(
            tokenize_fennel_plain_text(&[FENNEL_CONTROL_PREFIX])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn tokenizes_proven_signed_position_control() {
        let units = "A$t[-6:0]B$T[12:-34]C".encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            tokenize_fennel_plain_text(&units).unwrap(),
            vec![
                FennelPlainToken::Glyph(b'A' as u16),
                FennelPlainToken::SetPosition { x: -6, y: 0 },
                FennelPlainToken::Glyph(b'B' as u16),
                FennelPlainToken::SetPosition { x: 12, y: -34 },
                FennelPlainToken::Glyph(b'C' as u16),
            ]
        );
    }

    #[test]
    fn builds_plain_record_stream_with_binary_markers() {
        let font = RuhunaRuntimeFont {
            minimum_code: b'A' as u16,
            maximum_code: b'B' as u16,
            dense_glyph_indices: vec![0, 1],
            glyph_pages: vec![Some(0), Some(0)],
            glyphs: vec![
                RuhunaRuntimeGlyphRecord {
                    code: b'A' as u16,
                    texture_token: 7,
                    width: 3,
                    height: 4,
                    ..Default::default()
                },
                RuhunaRuntimeGlyphRecord {
                    code: b'B' as u16,
                    texture_token: 8,
                    width: 5,
                    height: 6,
                    ..Default::default()
                },
            ],
        };
        let stream =
            build_fennel_plain_record_stream(b"A\nB", &font, input(), 5, |code, _glyph| {
                u32::from(code)
            })
            .unwrap();

        assert_eq!(stream.glyph_count, 2);
        assert_eq!(stream.line_start_indices, vec![0, 2, 4]);
        assert_eq!(
            stream
                .records
                .iter()
                .map(|record| record.kind)
                .collect::<Vec<_>>(),
            vec![
                0,
                FENNEL_RECORD_LINE_END,
                0,
                FENNEL_RECORD_LINE_END,
                FENNEL_RECORD_STREAM_END
            ]
        );
        assert_eq!(stream.records[0].glyph_token, u32::from(b'A'));
        assert_eq!(stream.records[2].texture_token, 8);
    }

    #[test]
    fn plain_record_stream_refuses_unproven_fallbacks_and_overflow() {
        let font = RuhunaRuntimeFont {
            minimum_code: b'A' as u16,
            maximum_code: b'A' as u16,
            dense_glyph_indices: vec![0],
            glyph_pages: vec![Some(0)],
            glyphs: vec![RuhunaRuntimeGlyphRecord {
                code: b'A' as u16,
                ..Default::default()
            }],
        };
        assert_eq!(
            build_fennel_plain_record_stream(b"B", &font, input(), 3, |_, _| 1).unwrap_err(),
            FennelPlainRecordError::MissingGlyph { code: b'B' as u16 }
        );
        assert_eq!(
            build_fennel_plain_record_stream(b"A", &font, input(), 2, |_, _| 1).unwrap_err(),
            FennelPlainRecordError::RecordCapacity {
                capacity: 2,
                required: 3,
            }
        );
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
