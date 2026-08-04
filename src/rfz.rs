use std::fmt;

const DICTIONARY_SIZE: usize = 0x1000;
const INITIAL_SYMBOLS: usize = 0x100;
const RESET_CODE: usize = DICTIONARY_SIZE - 2;
const INITIAL_CODE_WIDTH: u8 = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfzError {
    pub offset: usize,
    pub message: String,
}

impl RfzError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for RfzError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "RFZ decode error at {:#x}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for RfzError {}

#[derive(Clone, Copy, Default)]
struct DictionaryEntry {
    byte: u8,
    prefix: Option<u16>,
}

pub fn decompress(bytes: &[u8]) -> Result<Vec<u8>, RfzError> {
    if bytes.len() < 4 {
        return Err(RfzError::new(0, "truncated four-byte stream header"));
    }
    if bytes[0] != b'Y' || bytes[1] != b'S' {
        return Err(RfzError::new(0, "missing YS stream signature"));
    }
    if bytes[2] != 2 {
        return Err(RfzError::new(
            2,
            format!("unsupported stream version {}", bytes[2]),
        ));
    }

    let mut dictionary = [DictionaryEntry::default(); DICTIONARY_SIZE];
    for (code, entry) in dictionary[..INITIAL_SYMBOLS].iter_mut().enumerate() {
        entry.byte = code as u8;
    }

    let mut reader = BitReader::new(&bytes[4..], 4);
    let Some(first_code) = reader.read(INITIAL_CODE_WIDTH)? else {
        return Ok(Vec::new());
    };
    if first_code as usize >= INITIAL_SYMBOLS {
        return Err(RfzError::new(
            reader.absolute_offset(),
            format!("initial code {first_code} is outside the literal dictionary"),
        ));
    }

    let mut output = Vec::new();
    let mut stack = Vec::with_capacity(DICTIONARY_SIZE);
    let mut previous_code = first_code;
    let mut previous_first = expand(
        previous_code,
        &dictionary,
        INITIAL_SYMBOLS,
        &mut stack,
        &mut output,
        reader.absolute_offset(),
    )?;
    let mut next_code = INITIAL_SYMBOLS;
    let mut code_width = INITIAL_CODE_WIDTH;

    while let Some(code) = reader.read(code_width)? {
        let code = code as usize;
        if code > next_code {
            return Err(RfzError::new(
                reader.absolute_offset(),
                format!("code {code} exceeds next dictionary code {next_code}"),
            ));
        }

        let current_first = if code == next_code {
            let first = expand(
                previous_code,
                &dictionary,
                next_code,
                &mut stack,
                &mut output,
                reader.absolute_offset(),
            )?;
            output.push(previous_first);
            first
        } else {
            expand(
                code as u16,
                &dictionary,
                next_code,
                &mut stack,
                &mut output,
                reader.absolute_offset(),
            )?
        };

        if next_code != RESET_CODE {
            dictionary[next_code] = DictionaryEntry {
                byte: current_first,
                prefix: Some(previous_code),
            };
            let inserted_code = next_code;
            next_code += 1;
            if inserted_code + 2 == (1usize << code_width) {
                code_width += 1;
            }
            previous_code = code as u16;
            previous_first = current_first;
            continue;
        }

        for entry in &mut dictionary[INITIAL_SYMBOLS..] {
            entry.prefix = None;
        }
        next_code = INITIAL_SYMBOLS;
        code_width = INITIAL_CODE_WIDTH;

        let Some(code) = reader.read(code_width)? else {
            break;
        };
        if code as usize >= next_code {
            return Err(RfzError::new(
                reader.absolute_offset(),
                format!("post-reset code {code} is outside the literal dictionary"),
            ));
        }
        previous_code = code;
        previous_first = expand(
            previous_code,
            &dictionary,
            next_code,
            &mut stack,
            &mut output,
            reader.absolute_offset(),
        )?;
    }

    Ok(output)
}

fn expand(
    code: u16,
    dictionary: &[DictionaryEntry; DICTIONARY_SIZE],
    next_code: usize,
    stack: &mut Vec<u8>,
    output: &mut Vec<u8>,
    offset: usize,
) -> Result<u8, RfzError> {
    if usize::from(code) >= next_code {
        return Err(RfzError::new(
            offset,
            format!("dictionary code {code} has not been defined"),
        ));
    }

    stack.clear();
    let mut current = code;
    loop {
        let entry = dictionary[usize::from(current)];
        stack.push(entry.byte);
        let Some(prefix) = entry.prefix else {
            break;
        };
        current = prefix;
        if stack.len() == DICTIONARY_SIZE {
            return Err(RfzError::new(offset, "dictionary prefix chain is cyclic"));
        }
    }
    let first = *stack.last().unwrap();
    output.extend(stack.iter().rev());
    Ok(first)
}

struct BitReader<'a> {
    bytes: &'a [u8],
    base_offset: usize,
    byte_offset: usize,
    accumulator: u32,
    bits: u8,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8], base_offset: usize) -> Self {
        Self {
            bytes,
            base_offset,
            byte_offset: 0,
            accumulator: 0,
            bits: 0,
        }
    }

    fn read(&mut self, width: u8) -> Result<Option<u16>, RfzError> {
        while self.bits < width {
            let Some(byte) = self.bytes.get(self.byte_offset).copied() else {
                return Ok(None);
            };
            self.byte_offset += 1;
            self.accumulator = (self.accumulator << 8) | u32::from(byte);
            self.bits += 8;
        }

        let remaining = self.bits - width;
        let code = self.accumulator >> remaining;
        self.accumulator &= if remaining == 0 {
            0
        } else {
            (1u32 << remaining) - 1
        };
        self.bits = remaining;
        u16::try_from(code)
            .map(Some)
            .map_err(|_| RfzError::new(self.absolute_offset(), "code exceeds u16"))
    }

    fn absolute_offset(&self) -> usize {
        self.base_offset + self.byte_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_rfz_header() {
        let error = decompress(b"VTBF").unwrap_err();
        assert!(error.message.contains("YS"));
    }

    #[test]
    fn decodes_msb_first_codes() {
        let mut reader = BitReader::new(&[0b1010_0101, 0b1100_0011], 4);
        assert_eq!(reader.read(3).unwrap(), Some(0b101));
        assert_eq!(reader.read(5).unwrap(), Some(0b00101));
        assert_eq!(reader.read(4).unwrap(), Some(0b1100));
        assert_eq!(reader.read(4).unwrap(), Some(0b0011));
        assert_eq!(reader.read(1).unwrap(), None);
    }
}
