use std::fmt;
use std::ops::Range;

const HEADER_SIZE: usize = 0x80;
const ENTRY_SIZE: usize = 0x400;
const ENTRY_NAME_SIZE: usize = 0x200;
const ENTRY_FIELD_200: usize = 0x200;
const ENTRY_FIELD_204: usize = 0x204;
const ENTRY_DATA_SIZE: usize = 0x208;
const ENTRY_DATA_OFFSET: usize = 0x20c;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvtsError {
    pub offset: usize,
    pub message: String,
}

impl AvtsError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for AvtsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "AVTS parse error at {:#x}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for AvtsError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvtsEntry {
    pub name: Vec<u8>,
    pub field_200: u32,
    pub field_204: u32,
    pub data: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct AvtsFile<'a> {
    bytes: &'a [u8],
    pub entries: Vec<AvtsEntry>,
}

impl<'a> AvtsFile<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, AvtsError> {
        if bytes.get(..4) != Some(b"AVTS") {
            return Err(AvtsError::new(0, "missing AVTS magic"));
        }
        let entry_count = read_u32(bytes, 4, "entry count")?;
        let entry_count = usize::try_from(entry_count)
            .map_err(|_| AvtsError::new(4, "entry count does not fit usize"))?;
        let table_size = entry_count
            .checked_mul(ENTRY_SIZE)
            .and_then(|size| HEADER_SIZE.checked_add(size))
            .ok_or_else(|| AvtsError::new(4, "entry table size overflows"))?;
        if table_size > bytes.len() {
            return Err(AvtsError::new(4, "entry table is truncated"));
        }

        let mut entries = Vec::with_capacity(entry_count);
        for index in 0..entry_count {
            let base = HEADER_SIZE + index * ENTRY_SIZE;
            let name_field = &bytes[base..base + ENTRY_NAME_SIZE];
            let name_end = name_field
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| AvtsError::new(base, "entry name is not NUL-terminated"))?;
            let field_200 = read_u32(bytes, base + ENTRY_FIELD_200, "entry field_200")?;
            let field_204 = read_u32(bytes, base + ENTRY_FIELD_204, "entry field_204")?;
            let data_size = read_u32(bytes, base + ENTRY_DATA_SIZE, "entry data size")?;
            let data_offset = read_u32(bytes, base + ENTRY_DATA_OFFSET, "entry data offset")?;
            let data_size = usize::try_from(data_size).map_err(|_| {
                AvtsError::new(base + ENTRY_DATA_SIZE, "entry data size does not fit usize")
            })?;
            let data_offset = usize::try_from(data_offset).map_err(|_| {
                AvtsError::new(
                    base + ENTRY_DATA_OFFSET,
                    "entry data offset does not fit usize",
                )
            })?;
            let data_end = data_offset.checked_add(data_size).ok_or_else(|| {
                AvtsError::new(base + ENTRY_DATA_OFFSET, "entry data range overflows")
            })?;
            if data_offset < table_size {
                return Err(AvtsError::new(
                    base + ENTRY_DATA_OFFSET,
                    "entry data overlaps the AVTS header or directory",
                ));
            }
            if data_end > bytes.len() {
                return Err(AvtsError::new(
                    base + ENTRY_DATA_SIZE,
                    "entry data is truncated",
                ));
            }
            entries.push(AvtsEntry {
                name: name_field[..name_end].to_vec(),
                field_200,
                field_204,
                data: data_offset..data_end,
            });
        }

        let mut ranges = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.data.clone(), index))
            .collect::<Vec<_>>();
        ranges.sort_by_key(|(range, _)| range.start);
        for pair in ranges.windows(2) {
            if pair[0].0.end > pair[1].0.start {
                return Err(AvtsError::new(
                    pair[1].0.start,
                    format!("entries {} and {} overlap", pair[0].1, pair[1].1),
                ));
            }
        }

        Ok(Self { bytes, entries })
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn entry_data(&self, entry: &AvtsEntry) -> &'a [u8] {
        &self.bytes[entry.data.clone()]
    }
}

fn read_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, AvtsError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| AvtsError::new(offset, format!("{label} is truncated")))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_binary_proven_directory_layout() {
        let mut bytes = vec![0; HEADER_SIZE + ENTRY_SIZE + 4];
        bytes[..4].copy_from_slice(b"AVTS");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[HEADER_SIZE..HEADER_SIZE + 5].copy_from_slice(b"test\0");
        bytes[HEADER_SIZE + ENTRY_FIELD_200..HEADER_SIZE + ENTRY_FIELD_200 + 4]
            .copy_from_slice(&3u32.to_le_bytes());
        bytes[HEADER_SIZE + ENTRY_FIELD_204..HEADER_SIZE + ENTRY_FIELD_204 + 4]
            .copy_from_slice(&4u32.to_le_bytes());
        bytes[HEADER_SIZE + ENTRY_DATA_SIZE..HEADER_SIZE + ENTRY_DATA_SIZE + 4]
            .copy_from_slice(&4u32.to_le_bytes());
        bytes[HEADER_SIZE + ENTRY_DATA_OFFSET..HEADER_SIZE + ENTRY_DATA_OFFSET + 4]
            .copy_from_slice(&((HEADER_SIZE + ENTRY_SIZE) as u32).to_le_bytes());
        let data_offset = HEADER_SIZE + ENTRY_SIZE;
        bytes[data_offset..].copy_from_slice(b"data");

        let avts = AvtsFile::parse(&bytes).unwrap();
        assert_eq!(avts.entries.len(), 1);
        assert_eq!(avts.entries[0].name, b"test");
        assert_eq!(avts.entries[0].field_200, 3);
        assert_eq!(avts.entries[0].field_204, 4);
        assert_eq!(avts.entry_data(&avts.entries[0]), b"data");
    }
}
