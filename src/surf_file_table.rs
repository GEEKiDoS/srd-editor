use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfFileTableRecord {
    pub id: u32,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub path: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfFileTableError(pub String);

impl fmt::Display for SurfFileTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SurfFileTableError {}

/// Parses Chusan's `db/SurfFileTableRecord.bin` record stream.
///
/// The file starts with a little-endian record count. Each record is an id
/// followed by three length-prefixed byte strings: symbolic name, display
/// description, and the SRD path relative to the game's `surfboard` root.
pub fn parse_surf_file_table(bytes: &[u8]) -> Result<Vec<SurfFileTableRecord>, SurfFileTableError> {
    let mut cursor = 0usize;
    let count = read_u32(bytes, &mut cursor, "record count")? as usize;
    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let id = read_u32(bytes, &mut cursor, &format!("record {index} id"))?;
        let name = read_bytes(bytes, &mut cursor, &format!("record {index} name"))?;
        let description = read_bytes(bytes, &mut cursor, &format!("record {index} description"))?;
        let path = read_bytes(bytes, &mut cursor, &format!("record {index} path"))?;
        records.push(SurfFileTableRecord {
            id,
            name,
            description,
            path,
        });
    }
    if cursor != bytes.len() {
        return Err(SurfFileTableError(format!(
            "SurfFileTable has {} trailing bytes after {count} records",
            bytes.len() - cursor
        )));
    }
    Ok(records)
}

fn read_u32(bytes: &[u8], cursor: &mut usize, field: &str) -> Result<u32, SurfFileTableError> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| SurfFileTableError(format!("{field} offset overflow")))?;
    let encoded = bytes.get(*cursor..end).ok_or_else(|| {
        SurfFileTableError(format!(
            "{field} is truncated at offset {} (file length {})",
            *cursor,
            bytes.len()
        ))
    })?;
    *cursor = end;
    Ok(u32::from_le_bytes(encoded.try_into().unwrap()))
}

fn read_bytes(
    bytes: &[u8],
    cursor: &mut usize,
    field: &str,
) -> Result<Vec<u8>, SurfFileTableError> {
    let length = read_u32(bytes, cursor, &format!("{field} length"))? as usize;
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| SurfFileTableError(format!("{field} range overflow")))?;
    let value = bytes.get(*cursor..end).ok_or_else(|| {
        SurfFileTableError(format!(
            "{field} declares {length} bytes at offset {}, beyond file length {}",
            *cursor,
            bytes.len()
        ))
    })?;
    *cursor = end;
    Ok(value.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_length_prefixed_records_and_requires_exact_eof() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&7u32.to_le_bytes());
        for value in [b"Name".as_slice(), b"Description", b"path/file.srd"] {
            bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
            bytes.extend_from_slice(value);
        }
        assert_eq!(
            parse_surf_file_table(&bytes).unwrap(),
            vec![SurfFileTableRecord {
                id: 7,
                name: b"Name".to_vec(),
                description: b"Description".to_vec(),
                path: b"path/file.srd".to_vec(),
            }]
        );

        bytes.push(0);
        assert!(parse_surf_file_table(&bytes).is_err());
    }

    #[test]
    fn rejects_truncated_strings() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(b"abc");
        assert!(parse_surf_file_table(&bytes).is_err());
    }
}
