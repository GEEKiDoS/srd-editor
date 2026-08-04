use std::fmt;
use std::ops::Range;

use crate::rfz::{self, RfzError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxError {
    pub offset: usize,
    pub message: String,
}

impl YabxError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for YabxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "YABX parse error at {:#x}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for YabxError {}

#[derive(Debug)]
pub enum RfzYabxError {
    Rfz(RfzError),
    Yabx(YabxError),
}

impl fmt::Display for RfzYabxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rfz(error) => error.fmt(formatter),
            Self::Yabx(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RfzYabxError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxLibrary {
    pub name: Vec<u8>,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxLegacyLibrary {
    pub flags: u8,
    pub name: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxField {
    pub name: Vec<u8>,
    pub flags: u8,
    pub storage_size: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxClass {
    pub name: Vec<u8>,
    pub flags: u8,
    pub parent_index: i16,
    pub fields: Vec<YabxField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxLocalId {
    pub name: Vec<u8>,
    pub encoded_id: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YabxObject {
    pub class_index: i16,
    pub data: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct YabxFile {
    bytes: Vec<u8>,
    pub version: u16,
    pub flags: u16,
    pub payload_size: u32,
    pub payload_crc: u32,
    pub legacy_libraries: Vec<YabxLegacyLibrary>,
    pub database_name: Vec<u8>,
    pub libraries: Vec<YabxLibrary>,
    pub classes: Vec<YabxClass>,
    pub local_ids: Vec<YabxLocalId>,
    pub object_names: Vec<Vec<u8>>,
    pub objects: Vec<YabxObject>,
    pub trailing: Range<usize>,
}

impl YabxFile {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, YabxError> {
        let mut reader = Reader::new(&bytes);
        let magic = reader.read_bytes(4)?;
        if magic == b"XBAY" {
            return Err(YabxError::new(
                0,
                "big-endian XBAY streams are not implemented",
            ));
        }
        if magic != b"YABX" {
            return Err(YabxError::new(0, "missing YABX magic"));
        }

        let version = reader.read_u16()?;
        let flags = reader.read_u16()?;
        let payload_size = reader.read_u32()?;
        let payload_crc = reader.read_u32()?;
        let payload_size_usize = usize::try_from(payload_size)
            .map_err(|_| YabxError::new(8, "payload size does not fit usize"))?;
        let payload_end = 16usize
            .checked_add(payload_size_usize)
            .ok_or_else(|| YabxError::new(8, "payload size overflows"))?;
        if payload_end > bytes.len() {
            return Err(YabxError::new(8, "declared payload is truncated"));
        }
        let actual_crc = crc32_be(&bytes[16..payload_end]);
        if actual_crc != payload_crc {
            return Err(YabxError::new(
                12,
                format!("payload CRC mismatch: {actual_crc:#010x} != {payload_crc:#010x}"),
            ));
        }

        let mut legacy_libraries = Vec::new();
        loop {
            let flags = reader.read_u8()?;
            if flags == 0 {
                break;
            }
            legacy_libraries.push(YabxLegacyLibrary {
                flags,
                name: reader.read_long_string()?,
            });
        }

        let database_name = reader.read_compact_string()?;
        let mut libraries = Vec::new();
        loop {
            let name = reader.read_compact_string()?;
            if name.is_empty() {
                break;
            }
            libraries.push(YabxLibrary {
                name,
                version: reader.read_u32()?,
            });
        }

        let mut classes = Vec::new();
        loop {
            let name = reader.read_compact_string()?;
            if name.is_empty() {
                break;
            }
            let flags = reader.read_u8()?;
            let parent_index = reader.read_i16()?;
            let mut fields = Vec::new();
            loop {
                let name = reader.read_compact_string()?;
                if name.is_empty() {
                    break;
                }
                fields.push(YabxField {
                    name,
                    flags: reader.read_u8()?,
                    storage_size: reader.read_i16()?,
                });
            }
            classes.push(YabxClass {
                name,
                flags,
                parent_index,
                fields,
            });
        }

        let mut local_ids = Vec::new();
        loop {
            let name = reader.read_compact_string()?;
            if name.is_empty() {
                break;
            }
            local_ids.push(YabxLocalId {
                name,
                encoded_id: reader.read_i16()?,
            });
        }

        let mut object_names = Vec::new();
        loop {
            let name = reader.read_compact_string()?;
            if name.is_empty() {
                break;
            }
            object_names.push(name);
        }

        let object_count_offset = reader.offset;
        let object_count = reader.read_i16()?;
        let object_count = usize::try_from(object_count)
            .map_err(|_| YabxError::new(object_count_offset, "negative serialized object count"))?;
        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            let class_index = reader.read_i16()?;
            let data_size_offset = reader.offset;
            let data_size = reader.read_u32()?;
            let data_size = usize::try_from(data_size).map_err(|_| {
                YabxError::new(data_size_offset, "object data size does not fit usize")
            })?;
            let start = reader.offset;
            reader.read_bytes(data_size)?;
            objects.push(YabxObject {
                class_index,
                data: start..reader.offset,
            });
        }

        if reader.offset > payload_end {
            return Err(YabxError::new(
                payload_end,
                "metadata or objects exceed the declared payload",
            ));
        }
        let trailing = reader.offset..bytes.len();
        Ok(Self {
            bytes,
            version,
            flags,
            payload_size,
            payload_crc,
            legacy_libraries,
            database_name,
            libraries,
            classes,
            local_ids,
            object_names,
            objects,
            trailing,
        })
    }

    pub fn from_rfz(bytes: &[u8]) -> Result<Self, RfzYabxError> {
        let decompressed = rfz::decompress(bytes).map_err(RfzYabxError::Rfz)?;
        Self::parse(decompressed).map_err(RfzYabxError::Yabx)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn object_data(&self, object: &YabxObject) -> &[u8] {
        &self.bytes[object.data.clone()]
    }

    pub fn object_class(&self, object: &YabxObject) -> Option<&YabxClass> {
        usize::try_from(object.class_index)
            .ok()
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| self.classes.get(index))
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[self.trailing.clone()]
    }
}

pub fn crc32_be(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    !crc
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_bytes(&mut self, size: usize) -> Result<&'a [u8], YabxError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| YabxError::new(self.offset, "read size overflows"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| YabxError::new(self.offset, format!("need {size} bytes")))?;
        self.offset = end;
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8, YabxError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, YabxError> {
        Ok(u16::from_le_bytes(self.read_bytes(2)?.try_into().unwrap()))
    }

    fn read_i16(&mut self) -> Result<i16, YabxError> {
        Ok(i16::from_le_bytes(self.read_bytes(2)?.try_into().unwrap()))
    }

    fn read_u32(&mut self) -> Result<u32, YabxError> {
        Ok(u32::from_le_bytes(self.read_bytes(4)?.try_into().unwrap()))
    }

    fn read_compact_string(&mut self) -> Result<Vec<u8>, YabxError> {
        let length_offset = self.offset;
        let length = self.read_u8()? as i8;
        let length = usize::try_from(length)
            .map_err(|_| YabxError::new(length_offset, "negative compact string length"))?;
        self.read_string_bytes(length)
    }

    fn read_long_string(&mut self) -> Result<Vec<u8>, YabxError> {
        let length_offset = self.offset;
        let length = self.read_i16()?;
        let length = usize::try_from(length)
            .map_err(|_| YabxError::new(length_offset, "negative long string length"))?;
        self.read_string_bytes(length)
    }

    fn read_string_bytes(&mut self, length: usize) -> Result<Vec<u8>, YabxError> {
        let offset = self.offset;
        let bytes = self.read_bytes(length)?;
        if length == 0 {
            return Ok(Vec::new());
        }
        if bytes.last() != Some(&0) {
            return Err(YabxError::new(
                offset,
                "serialized string is not NUL-terminated",
            ));
        }
        Ok(bytes[..length - 1].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_game_variant() {
        assert_eq!(crc32_be(b"123456789"), 0xfc89_1918);
    }

    #[test]
    fn rejects_bad_magic() {
        let error = YabxFile::parse(vec![0; 16]).unwrap_err();
        assert!(error.message.contains("YABX"));
    }
}
