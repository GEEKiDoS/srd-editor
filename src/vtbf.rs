use std::fmt;
use std::ops::Range;

pub const TYPE_SIZES: [i8; 16] = [0, 1, -1, 1, 1, 2, 2, 2, 4, 4, 4, 4, 4, 8, 0, -1];
pub const MULTIPLIERS: [u32; 8] = [2, 3, 4, 9, 16, 0, 0, 0];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

impl ParseError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VTBF parse error at {:#x}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone)]
pub struct SrdFile {
    bytes: Vec<u8>,
    pub unknown_04: [u8; 4],
    pub format: [u8; 4],
    pub unknown_0e: [u8; 2],
    pub blocks: Vec<Block>,
    pub trailing: Range<usize>,
}

impl SrdFile {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ParseError> {
        require(&bytes, 0, 16)?;
        if &bytes[0..4] != b"VTBF" {
            return Err(ParseError::new(0, "missing VTBF magic"));
        }

        let unknown_04 = bytes[4..8].try_into().unwrap();
        let format = bytes[8..12].try_into().unwrap();
        let block_count = read_u16(&bytes, 12)? as usize;
        let unknown_0e = bytes[14..16].try_into().unwrap();
        let mut cursor = 16;
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let (block, next) = parse_block(&bytes, cursor, 0)?;
            blocks.push(block);
            cursor = next;
        }

        Ok(Self {
            trailing: cursor..bytes.len(),
            bytes,
            unknown_04,
            format,
            unknown_0e,
            blocks,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn slice(&self, range: &Range<usize>) -> &[u8] {
        &self.bytes[range.clone()]
    }

    pub fn blocks_depth_first(&self) -> BlocksDepthFirst<'_> {
        BlocksDepthFirst {
            stack: self.blocks.iter().rev().collect(),
        }
    }
}

pub struct BlocksDepthFirst<'a> {
    stack: Vec<&'a Block>,
}

impl<'a> Iterator for BlocksDepthFirst<'a> {
    type Item = &'a Block;

    fn next(&mut self) -> Option<Self::Item> {
        let block = self.stack.pop()?;
        self.stack.extend(block.children.iter().rev());
        Some(block)
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub offset: usize,
    pub sub_sig: [u8; 4],
    pub size_field: u32,
    pub tag: [u8; 4],
    pub properties: Vec<Property>,
    pub property_tail: Range<usize>,
    pub children: Vec<Block>,
}

impl Block {
    pub fn is_tag(&self, tag: &[u8; 4]) -> bool {
        &self.tag == tag
    }

    pub fn properties_with_code(&self, code: u8) -> impl DoubleEndedIterator<Item = &Property> {
        self.properties
            .iter()
            .filter(move |property| property.code == code)
    }

    pub fn last_property(&self, code: u8) -> Option<&Property> {
        self.properties_with_code(code).next_back()
    }
}

#[derive(Debug, Clone)]
pub struct Property {
    pub offset: usize,
    pub code: u8,
    pub flags: u8,
    pub type_code: u8,
    pub count: u32,
    pub multiplier: u32,
    pub encoded: Range<usize>,
    pub value: Range<usize>,
    pub string_prefix: Option<Range<usize>>,
}

impl Property {
    pub fn encoded_bytes<'a>(&self, file: &'a SrdFile) -> &'a [u8] {
        file.slice(&self.encoded)
    }

    pub fn value_bytes<'a>(&self, file: &'a SrdFile) -> &'a [u8] {
        file.slice(&self.value)
    }

    pub fn string_bytes<'a>(&self, file: &'a SrdFile) -> Option<&'a [u8]> {
        (self.type_code == 2).then(|| self.value_bytes(file))
    }

    pub fn read_unsigned_scalar(&self, file: &SrdFile) -> Option<u32> {
        let value = self.value_bytes(file);
        match self.type_code {
            1 | 3 | 4 => value.first().copied().map(u32::from),
            5..=7 => read_u16(value, 0).ok().map(u32::from),
            8 | 9 | 11 | 12 => read_u32(value, 0).ok(),
            10 => read_f32(value, 0)
                .ok()
                .map(cvtt_f32_to_i32)
                .map(|v| v as u32),
            _ => None,
        }
    }

    pub fn read_signed_scalar(&self, file: &SrdFile) -> Option<i32> {
        self.read_signed_scalar_at(file, 0)
    }

    pub fn read_signed_scalar_at(&self, file: &SrdFile, index: usize) -> Option<i32> {
        let size = usize::try_from(*TYPE_SIZES.get(usize::from(self.type_code))?).ok()?;
        if size == 0 {
            return None;
        }
        let offset = index.checked_mul(size)?;
        let value = self.value_bytes(file).get(offset..)?;
        match self.type_code {
            1 | 4 => value.first().copied().map(i32::from),
            3 => value.first().copied().map(|v| i32::from(v as i8)),
            5 | 7 => read_u16(value, 0).ok().map(|v| i32::from(v as i16)),
            6 => read_u16(value, 0).ok().map(i32::from),
            8 | 9 | 11 | 12 => read_u32(value, 0).ok().map(|v| v as i32),
            10 => read_f32(value, 0).ok().map(cvtt_f32_to_i32),
            _ => None,
        }
    }

    pub fn read_scalar_as_f32(&self, file: &SrdFile) -> Option<f32> {
        self.read_scalar_as_f32_at(file, 0)
    }

    pub fn read_scalar_as_f32_at(&self, file: &SrdFile, index: usize) -> Option<f32> {
        let size = usize::try_from(*TYPE_SIZES.get(usize::from(self.type_code))?).ok()?;
        if size == 0 {
            return None;
        }
        let offset = index.checked_mul(size)?;
        let value = self.value_bytes(file).get(offset..)?;
        match self.type_code {
            1 | 4 => value.first().copied().map(f32::from),
            3 => value.first().copied().map(|v| f32::from(v as i8)),
            5 | 7 => read_u16(value, 0).ok().map(|v| f32::from(v as i16)),
            6 => read_u16(value, 0).ok().map(f32::from),
            8 | 9 | 11 | 12 => read_u32(value, 0).ok().map(|v| (v as i32) as f32),
            10 => read_f32(value, 0).ok(),
            _ => None,
        }
    }
}

fn parse_block(data: &[u8], offset: usize, depth: usize) -> Result<(Block, usize), ParseError> {
    if depth > 1024 {
        return Err(ParseError::new(offset, "block nesting exceeds 1024"));
    }
    require(data, offset, 16)?;
    let sub_sig: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
    if &sub_sig != b"vtc0" {
        return Err(ParseError::new(offset, "block does not start with vtc0"));
    }

    let size_field = read_u32(data, offset + 4)?;
    if size_field < 8 {
        return Err(ParseError::new(
            offset + 4,
            "block size is smaller than its tag header",
        ));
    }
    let own_end = offset
        .checked_add(8)
        .and_then(|value| value.checked_add(size_field as usize))
        .ok_or_else(|| ParseError::new(offset + 4, "block size overflow"))?;
    if own_end > data.len() {
        return Err(ParseError::new(
            offset + 4,
            "block own-data range exceeds file",
        ));
    }

    let tag = data[offset + 8..offset + 12].try_into().unwrap();
    let child_count = read_u16(data, offset + 12)? as usize;
    let property_count = read_u16(data, offset + 14)? as usize;
    let mut property_cursor = offset + 16;
    let mut properties = Vec::with_capacity(property_count);
    for _ in 0..property_count {
        let (property, next) = parse_property(data, property_cursor, own_end)?;
        properties.push(property);
        property_cursor = next;
    }
    if property_cursor > own_end {
        return Err(ParseError::new(
            property_cursor,
            "properties exceed block own-data range",
        ));
    }

    let mut child_cursor = own_end;
    let mut children = Vec::with_capacity(child_count);
    for _ in 0..child_count {
        let (child, next) = parse_block(data, child_cursor, depth + 1)?;
        children.push(child);
        child_cursor = next;
    }

    Ok((
        Block {
            offset,
            sub_sig,
            size_field,
            tag,
            properties,
            property_tail: property_cursor..own_end,
            children,
        },
        child_cursor,
    ))
}

fn parse_property(
    data: &[u8],
    offset: usize,
    block_own_end: usize,
) -> Result<(Property, usize), ParseError> {
    require_to(data, offset, 2, block_own_end)?;
    let code = data[offset];
    let flags = data[offset + 1];
    let type_code = flags & 0x3f;
    let has_multiplier = flags & 0x40 != 0;
    let has_count = flags & 0x80 != 0;
    let mut cursor = offset + 2;
    let mut count = 1u32;
    let mut multiplier = 1u32;

    if has_count || has_multiplier {
        require_to(data, cursor, 1, block_own_end)?;
        let extension = data[cursor];
        cursor += 1;
        if has_count {
            let stored = match extension & 0x18 {
                0x08 => {
                    require_to(data, cursor, 1, block_own_end)?;
                    let value = u32::from(data[cursor]);
                    cursor += 1;
                    value
                }
                0x10 => {
                    require_to(data, cursor, 2, block_own_end)?;
                    let value = u32::from(read_u16(data, cursor)?);
                    cursor += 2;
                    value
                }
                0x18 => {
                    require_to(data, cursor, 4, block_own_end)?;
                    let value = read_u32(data, cursor)?;
                    cursor += 4;
                    value
                }
                _ => 0,
            };
            count = stored
                .checked_add(1)
                .ok_or_else(|| ParseError::new(cursor, "property count overflow"))?;
        }
        if has_multiplier {
            multiplier = MULTIPLIERS[usize::from(extension & 7)];
        }
    }

    let (string_prefix, value_start, value_end) = if type_code == 2 {
        require_to(data, cursor, 1, block_own_end)?;
        let prefix_start = cursor;
        let first = data[cursor];
        cursor += 1;
        let length = if first & 0x80 == 0 {
            usize::from(first)
        } else {
            require_to(data, cursor, 1, block_own_end)?;
            let second = data[cursor];
            cursor += 1;
            (usize::from(first & 0x7f) << 8) | usize::from(second)
        };
        let value_start = cursor;
        require_to(data, value_start, length, block_own_end)?;
        let value_end = value_start + length;
        (Some(prefix_start..value_start), value_start, value_end)
    } else {
        let size = TYPE_SIZES
            .get(usize::from(type_code))
            .copied()
            .ok_or_else(|| ParseError::new(offset + 1, format!("unknown type code {type_code}")))?;
        if size < 0 {
            return Err(ParseError::new(
                offset + 1,
                format!("unsupported negative-sized type code {type_code}"),
            ));
        }
        let length = u64::from(count)
            .checked_mul(u64::from(multiplier))
            .and_then(|value| value.checked_mul(size as u64))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| ParseError::new(offset, "property byte length overflow"))?;
        let value_start = cursor;
        require_to(data, value_start, length, block_own_end)?;
        let value_end = value_start + length;
        (None, value_start, value_end)
    };

    Ok((
        Property {
            offset,
            code,
            flags,
            type_code,
            count,
            multiplier,
            encoded: offset..value_end,
            value: value_start..value_end,
            string_prefix,
        },
        value_end,
    ))
}

fn require(data: &[u8], offset: usize, size: usize) -> Result<(), ParseError> {
    require_to(data, offset, size, data.len())
}

fn require_to(data: &[u8], offset: usize, size: usize, end: usize) -> Result<(), ParseError> {
    let requested_end = offset
        .checked_add(size)
        .ok_or_else(|| ParseError::new(offset, "range overflow"))?;
    if requested_end > end || requested_end > data.len() {
        Err(ParseError::new(offset, format!("need {size} bytes")))
    } else {
        Ok(())
    }
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, ParseError> {
    require(data, offset, 2)?;
    Ok(u16::from_le_bytes(
        data[offset..offset + 2].try_into().unwrap(),
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, ParseError> {
    require(data, offset, 4)?;
    Ok(u32::from_le_bytes(
        data[offset..offset + 4].try_into().unwrap(),
    ))
}

fn read_f32(data: &[u8], offset: usize) -> Result<f32, ParseError> {
    Ok(f32::from_bits(read_u32(data, offset)?))
}

fn cvtt_f32_to_i32(value: f32) -> i32 {
    if !value.is_finite() || !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}
