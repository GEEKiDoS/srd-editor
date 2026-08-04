use crate::shader::CEYLON_SIMPLE_SHADER_KEY_LENGTH;

pub const FIRST_FIXTURE_SIMPLE_KEY: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] = *b"AAEBABBAAAGAAAAAAA";
pub const FIRST_TEXTURED_FIXTURE_SIMPLE_KEY: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] =
    *b"AAEBABBAABGAAAAAAA";
pub const FIRST_2D_FIXTURE_SIMPLE_KEY: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] =
    *b"EAEBABBAAAGAAAAAAA";
pub const FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] =
    *b"EAEBABBAABGAAAAAAA";
/// Exact compact Simple-selector key compiled from the game's Cg source for a
/// normal 3D Fennel atlas batch (Ceylon vertex format 13).
pub const FENNEL_TEXTURED_SIMPLE_KEY: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] =
    *b"AAMAAABAABGAAAAAAA";
/// Exact compact Simple-selector key for the same Fennel batch with
/// DrawPacket +0x60 bit 7 (`ShapeEnv2D`) set.
pub const FENNEL_TEXTURED_2D_SIMPLE_KEY: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] =
    *b"EAMAAABAABGAAAAAAA";

pub const FIRST_FIXTURE_VERTEX_SHADER_SHA256: &str =
    "86669F24505A70D6DB560C6B2838EBA7D262B0206825BA3927658AB5A7112D61";
pub const FIRST_FIXTURE_PIXEL_SHADER_SHA256: &str =
    "B7D50CF8DAC3A981DB13F2B5C3C7CAF8935FC4392B385B516620F7584EC2E53F";
pub const FIRST_TEXTURED_FIXTURE_PIXEL_SHADER_SHA256: &str =
    "066761E3FE149084A9526FDD1A091138B9DC894EAC29FA707D71992E4ED4E23F";
pub const FIRST_2D_FIXTURE_VERTEX_SHADER_SHA256: &str =
    "A3E0CA2EFA3452A529DDE7E92EB638FAAD4A230DF5A5537D2D50BE53BC045BD4";

pub const EMBEDDED_SIMPLE_SHADER_KEYS: [[u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH]; 6] = [
    FIRST_FIXTURE_SIMPLE_KEY,
    FIRST_TEXTURED_FIXTURE_SIMPLE_KEY,
    FIRST_2D_FIXTURE_SIMPLE_KEY,
    FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY,
    FENNEL_TEXTURED_SIMPLE_KEY,
    FENNEL_TEXTURED_2D_SIMPLE_KEY,
];

pub struct EmbeddedSimpleShaderPair {
    pub vertex_shader: &'static [u32],
    pub pixel_shader: &'static [u32],
}

pub fn embedded_simple_shader_pair(
    key: &[u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH],
) -> Option<EmbeddedSimpleShaderPair> {
    match *key {
        FIRST_FIXTURE_SIMPLE_KEY => Some(EmbeddedSimpleShaderPair {
            vertex_shader: &FIRST_FIXTURE_VERTEX_SHADER,
            pixel_shader: &FIRST_FIXTURE_PIXEL_SHADER,
        }),
        FIRST_TEXTURED_FIXTURE_SIMPLE_KEY => Some(EmbeddedSimpleShaderPair {
            vertex_shader: &FIRST_FIXTURE_VERTEX_SHADER,
            pixel_shader: &FIRST_TEXTURED_FIXTURE_PIXEL_SHADER,
        }),
        FIRST_2D_FIXTURE_SIMPLE_KEY => Some(EmbeddedSimpleShaderPair {
            vertex_shader: &FIRST_2D_FIXTURE_VERTEX_SHADER,
            pixel_shader: &FIRST_FIXTURE_PIXEL_SHADER,
        }),
        FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY => Some(EmbeddedSimpleShaderPair {
            vertex_shader: &FIRST_2D_FIXTURE_VERTEX_SHADER,
            pixel_shader: &FIRST_TEXTURED_FIXTURE_PIXEL_SHADER,
        }),
        FENNEL_TEXTURED_SIMPLE_KEY => Some(EmbeddedSimpleShaderPair {
            vertex_shader: &FIRST_FIXTURE_VERTEX_SHADER,
            pixel_shader: &FIRST_TEXTURED_FIXTURE_PIXEL_SHADER,
        }),
        FENNEL_TEXTURED_2D_SIMPLE_KEY => Some(EmbeddedSimpleShaderPair {
            vertex_shader: &FIRST_2D_FIXTURE_VERTEX_SHADER,
            pixel_shader: &FIRST_TEXTURED_FIXTURE_PIXEL_SHADER,
        }),
        _ => None,
    }
}

const FIRST_FIXTURE_VERTEX_SHADER: [u32; 83] = [
    0xFFFE0300, 0x0200001F, 0x80000000, 0xE00F0000, 0x0200001F, 0x8000000A, 0xE00F0001, 0x0200001F,
    0x8001000A, 0xE00F0002, 0x0200001F, 0x80000005, 0xE00F0003, 0x05000051, 0xA00F000E, 0x00000000,
    0x3F800000, 0x00000000, 0x00000000, 0x0200001F, 0x80000000, 0x900F0000, 0x0200001F, 0x80000005,
    0x900F0001, 0x0200001F, 0x8000000A, 0x900F0002, 0x0200001F, 0x8001000A, 0x900F0003, 0x02000001,
    0x80080001, 0xA055000E, 0x02000001, 0x80070001, 0x90E40000, 0x03000009, 0x80080000, 0x80E40001,
    0xA0E40003, 0x03000009, 0x80040000, 0x80E40001, 0xA0E40002, 0x03000009, 0x80010000, 0x80E40001,
    0xA0E40000, 0x03000009, 0x80020000, 0x80E40001, 0xA0E40001, 0x03000009, 0xE0080000, 0x80E40000,
    0xA0E4000D, 0x03000009, 0xE0040000, 0x80E40000, 0xA0E4000C, 0x03000009, 0xE0020000, 0x80E40000,
    0xA0E4000B, 0x03000009, 0xE0010000, 0x80E40000, 0xA0E4000A, 0x02000001, 0xE00F0001, 0x90E40002,
    0x02000001, 0xE00F0002, 0x90E40003, 0x03000002, 0xE0030003, 0x90E40001, 0xA0EE0008, 0x02000001,
    0xE00C0003, 0xA000000E, 0x0000FFFF,
];

const FIRST_2D_FIXTURE_VERTEX_SHADER: [u32; 96] = [
    0xfffe0300, 0x0200001f, 0x80000000, 0xe00f0000, 0x0200001f, 0x8000000a, 0xe00f0001, 0x0200001f,
    0x8001000a, 0xe00f0002, 0x0200001f, 0x80000005, 0xe00f0003, 0x05000051, 0xa00f000b, 0x00000000,
    0x3f800000, 0x3f000000, 0xbf000000, 0x05000051, 0xa00f000c, 0xbf800000, 0x00000000, 0x00000000,
    0x00000000, 0x0200001f, 0x80000000, 0x900f0000, 0x0200001f, 0x80000005, 0x900f0001, 0x0200001f,
    0x8000000a, 0x900f0002, 0x0200001f, 0x8001000a, 0x900f0003, 0x02000001, 0x80080000, 0xa055000b,
    0x02000001, 0x80070000, 0x90e40000, 0x03000009, 0x80010001, 0x80e40000, 0xa0e40001, 0x02000006,
    0x80020001, 0xa055000a, 0x03000002, 0x80010001, 0x81e40001, 0xa0aa000b, 0x04000004, 0xe0020000,
    0x80000001, 0x80e40001, 0xa0e4000b, 0x03000009, 0x80010001, 0x80e40000, 0xa0e40000, 0x02000006,
    0x80020001, 0xa000000a, 0x03000002, 0x80010001, 0x80e40001, 0xa0ff000b, 0x04000004, 0xe0010000,
    0x80e40001, 0x80550001, 0xa0e4000c, 0x03000009, 0xe0080000, 0x80e40000, 0xa0e40003, 0x02000001,
    0xe00f0001, 0x90e40002, 0x02000001, 0xe00f0002, 0x90e40003, 0x03000002, 0xe0030003, 0x90e40001,
    0xa0ee0008, 0x02000001, 0xe00c0003, 0xa000000b, 0x02000001, 0xe0040000, 0xa000000b, 0x0000ffff,
];

const FIRST_FIXTURE_PIXEL_SHADER: [u32; 54] = [
    0xFFFF0300, 0x05000051, 0xA00F0002, 0x4479F99A, 0x00000000, 0x3F800000, 0x00000000, 0x0200001F,
    0x8000000A, 0x900F0000, 0x0200001F, 0x8001000A, 0x90070001, 0x02000001, 0x80070000, 0x90E40001,
    0x03000002, 0x80070000, 0x90E40000, 0x80E40000, 0x02000001, 0x80080000, 0x90E40000, 0x03000005,
    0x80070000, 0x80E40000, 0xA0AA0000, 0x03000002, 0x800F0001, 0x81E40000, 0xA0000002, 0x04000058,
    0x800F0001, 0x80E40001, 0xA0550002, 0xA0AA0002, 0x03000002, 0x80310001, 0x80E40001, 0x80550001,
    0x03000002, 0x80310001, 0x80E40001, 0x80AA0001, 0x03000002, 0x80310001, 0x80E40001, 0x80FF0001,
    0x04000058, 0x800F0800, 0x81000001, 0x80E40000, 0xA0550002, 0x0000FFFF,
];

const FIRST_TEXTURED_FIXTURE_PIXEL_SHADER: [u32; 62] = [
    0xFFFF0300, 0x0200001F, 0x90000000, 0xA00F0800, 0x05000051, 0xA00F0002, 0x4479F99A, 0x00000000,
    0x3F800000, 0x00000000, 0x0200001F, 0x8000000A, 0x900F0000, 0x0200001F, 0x8001000A, 0x90070001,
    0x0200001F, 0x80000005, 0x90030002, 0x03000042, 0x800F0000, 0x90E40002, 0xA0E40800, 0x03000005,
    0x800F0000, 0x80E40000, 0x90E40000, 0x03000002, 0x80070000, 0x80E40000, 0x90E40001, 0x03000005,
    0x80070000, 0x80E40000, 0xA0AA0000, 0x03000002, 0x800F0001, 0x81E40000, 0xA0000002, 0x04000058,
    0x800F0001, 0x80E40001, 0xA0550002, 0xA0AA0002, 0x03000002, 0x80310001, 0x80E40001, 0x80550001,
    0x03000002, 0x80310001, 0x80E40001, 0x80AA0001, 0x03000002, 0x80310001, 0x80E40001, 0x80FF0001,
    0x04000058, 0x800F0800, 0x81000001, 0x80E40000, 0xA0550002, 0x0000FFFF,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_evidence_complete_fixture_keys_are_packaged() {
        let pair = embedded_simple_shader_pair(&FIRST_FIXTURE_SIMPLE_KEY).unwrap();
        assert_eq!(pair.vertex_shader.len() * 4, 332);
        assert_eq!(pair.pixel_shader.len() * 4, 216);
        assert_eq!(pair.vertex_shader[0], 0xFFFE0300);
        assert_eq!(pair.pixel_shader[0], 0xFFFF0300);
        assert_eq!(pair.vertex_shader.last(), Some(&0x0000FFFF));
        assert_eq!(pair.pixel_shader.last(), Some(&0x0000FFFF));

        let textured = embedded_simple_shader_pair(&FIRST_TEXTURED_FIXTURE_SIMPLE_KEY).unwrap();
        assert_eq!(textured.vertex_shader, pair.vertex_shader);
        assert_eq!(textured.pixel_shader.len() * 4, 248);
        assert_eq!(textured.pixel_shader[0], 0xFFFF0300);
        assert_eq!(textured.pixel_shader.last(), Some(&0x0000FFFF));

        let two_d = embedded_simple_shader_pair(&FIRST_2D_FIXTURE_SIMPLE_KEY).unwrap();
        assert_eq!(two_d.vertex_shader.len() * 4, 384);
        assert_eq!(two_d.pixel_shader, pair.pixel_shader);
        assert_ne!(two_d.vertex_shader, pair.vertex_shader);
        assert_eq!(two_d.vertex_shader.last(), Some(&0x0000ffff));

        let textured_two_d =
            embedded_simple_shader_pair(&FIRST_TEXTURED_2D_FIXTURE_SIMPLE_KEY).unwrap();
        assert_eq!(textured_two_d.vertex_shader, two_d.vertex_shader);
        assert_eq!(textured_two_d.pixel_shader, textured.pixel_shader);

        let fennel = embedded_simple_shader_pair(&FENNEL_TEXTURED_SIMPLE_KEY).unwrap();
        assert_eq!(fennel.vertex_shader.len() * 4, 332);
        assert_eq!(fennel.pixel_shader.len() * 4, 248);
        assert_eq!(fennel.vertex_shader, textured.vertex_shader);
        assert_eq!(fennel.pixel_shader, textured.pixel_shader);

        let fennel_two_d = embedded_simple_shader_pair(&FENNEL_TEXTURED_2D_SIMPLE_KEY).unwrap();
        assert_eq!(fennel_two_d.vertex_shader.len() * 4, 384);
        assert_eq!(fennel_two_d.pixel_shader.len() * 4, 248);
        assert_eq!(fennel_two_d.vertex_shader, textured_two_d.vertex_shader);
        assert_eq!(fennel_two_d.pixel_shader, textured_two_d.pixel_shader);

        let mut unsupported = FIRST_FIXTURE_SIMPLE_KEY;
        unsupported[0] = b'B';
        assert!(embedded_simple_shader_pair(&unsupported).is_none());
    }
}
