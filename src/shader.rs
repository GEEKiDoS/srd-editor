use std::collections::BTreeMap;

pub const CEYLON_DEFAULT_SHADER_SELECTOR_COUNT: u8 = 9;
pub const CEYLON_SIMPLE_SHADER_SELECTOR_INDEX: u8 = 9;
pub const CEYLON_SHADER_SELECTOR_COUNT: u8 = 10;
pub const CEYLON_SIMPLE_SHADER_FEATURE_COUNT: usize = 71;
pub const CEYLON_SIMPLE_SHADER_KEY_LENGTH: usize = 18;
pub const SEA_EMBEDDED_SHADER_SOURCE_XOR: u32 = 0x5963_4649;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonSimpleSelectorParameterDescriptor {
    pub parameter_id: u32,
    /// Feature positions are ordered by source value bit, starting at bit 0.
    pub positions: &'static [u8],
}

const SIMPLE_PARAMETER_1_POSITIONS: &[u8] = &[2];
const SIMPLE_PARAMETER_6_POSITIONS: &[u8] = &[41, 42, 43, 44, 45, 46];
const SIMPLE_PARAMETER_7_POSITIONS: &[u8] = &[47, 48, 49, 50];
const SIMPLE_PARAMETER_8_POSITIONS: &[u8] = &[51, 52, 53];
const SIMPLE_PARAMETER_9_POSITIONS: &[u8] = &[16, 17];
const SIMPLE_PARAMETER_11_POSITIONS: &[u8] = &[19];
const SIMPLE_PARAMETER_22_POSITIONS: &[u8] = &[27];
const SIMPLE_PARAMETER_23_POSITIONS: &[u8] = &[25];
const SIMPLE_PARAMETER_24_POSITIONS: &[u8] = &[39, 40];
const SIMPLE_PARAMETER_30_POSITIONS: &[u8] = &[26];
const SIMPLE_PARAMETER_40_POSITIONS: &[u8] = &[34, 35];
const SIMPLE_PARAMETER_52_POSITIONS: &[u8] = &[28, 29];
const SIMPLE_PARAMETER_53_POSITIONS: &[u8] = &[30, 31];
const SIMPLE_PARAMETER_61_POSITIONS: &[u8] = &[32, 33];
const SIMPLE_PARAMETER_66_POSITIONS: &[u8] = &[69];
const SIMPLE_PARAMETER_68_POSITIONS: &[u8] = &[70];

/// Complete integer selector-parameter table constructed by
/// `sea_simple_shader_selector_construct` (`0x65ED50`). This table describes
/// how a final parameter-set value becomes Simple feature bits; it does not
/// imply that any particular provider is active for an SRD draw.
pub const CEYLON_SIMPLE_SELECTOR_PARAMETERS: [CeylonSimpleSelectorParameterDescriptor; 16] = [
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 1,
        positions: SIMPLE_PARAMETER_1_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 6,
        positions: SIMPLE_PARAMETER_6_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 7,
        positions: SIMPLE_PARAMETER_7_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 8,
        positions: SIMPLE_PARAMETER_8_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 9,
        positions: SIMPLE_PARAMETER_9_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 11,
        positions: SIMPLE_PARAMETER_11_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 22,
        positions: SIMPLE_PARAMETER_22_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 23,
        positions: SIMPLE_PARAMETER_23_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 24,
        positions: SIMPLE_PARAMETER_24_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 30,
        positions: SIMPLE_PARAMETER_30_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 40,
        positions: SIMPLE_PARAMETER_40_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 52,
        positions: SIMPLE_PARAMETER_52_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 53,
        positions: SIMPLE_PARAMETER_53_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 61,
        positions: SIMPLE_PARAMETER_61_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 66,
        positions: SIMPLE_PARAMETER_66_POSITIONS,
    },
    CeylonSimpleSelectorParameterDescriptor {
        parameter_id: 68,
        positions: SIMPLE_PARAMETER_68_POSITIONS,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedCeylonSimpleSelectorParameter {
    pub parameter_id: u32,
}

/// Final integer selector-parameter values appended by
/// `sea::LightShadowParallel` before `SimpleShaderSelector` builds its key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CeylonShadowParallelParameters {
    /// Parameter 43. The provider raises this once per live cascade node.
    pub cascade_count: u32,
    /// Parameter 44. Any non-zero value enables edge hiding.
    pub edge_hide: u32,
    /// Parameter 45. The selector clamps this value to 3.
    pub color_shadow: u32,
    /// Parameters 46, 47 and 48. The selector clamps each value to 7.
    pub cascade_modes: [u32; 3],
}

impl CeylonShadowParallelParameters {
    /// Reproduces the values emitted by `sea::LightShadowParallel` from its
    /// authored `CascadeNum`, `Softness`, `EdgeHide` and `ColorShadow`
    /// properties. `CascadeNum` is normalized by the binary to 1..=3 before
    /// the cascade-node vector is resized.
    pub fn from_light_shadow_parallel(
        cascade_num: u32,
        softness: u32,
        edge_hide: bool,
        color_shadow: u32,
    ) -> Self {
        let cascade_count = if cascade_num == 0 {
            1
        } else {
            cascade_num.min(3)
        };
        let next_mode = if softness >= 2 { 1 } else { 0 };
        Self {
            cascade_count,
            edge_hide: u32::from(edge_hide),
            color_shadow,
            cascade_modes: [softness, if cascade_count >= 2 { next_mode } else { 0 }, 0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedShaderSourceLengthError {
    pub encoded_length: usize,
}

/// Decodes the static shader-source records registered by the game's shader
/// manager. The binary transforms one complete little-endian dword at a time;
/// all registered records are therefore four-byte aligned.
pub fn decode_embedded_shader_source(
    resource_hash: u32,
    encoded: &[u8],
) -> Result<Vec<u8>, EmbeddedShaderSourceLengthError> {
    if !encoded.len().is_multiple_of(4) {
        return Err(EmbeddedShaderSourceLengthError {
            encoded_length: encoded.len(),
        });
    }

    let mut decoded = Vec::with_capacity(encoded.len());
    for word in encoded.chunks_exact(4) {
        let encoded_word = u32::from_le_bytes(word.try_into().expect("four-byte chunk"));
        decoded.extend_from_slice(
            &(encoded_word ^ resource_hash ^ SEA_EMBEDDED_SHADER_SOURCE_XOR).to_le_bytes(),
        );
    }
    Ok(decoded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeylonShaderSelectorKind {
    Default,
    Simple,
}

/// Returns the concrete selector installed in the initial ten-entry registry.
/// The game's lookup applies modulo by the registry length before indexing.
pub const fn ceylon_shader_selector_kind(index: u8) -> CeylonShaderSelectorKind {
    if index % CEYLON_SHADER_SELECTOR_COUNT < CEYLON_DEFAULT_SHADER_SELECTOR_COUNT {
        CeylonShaderSelectorKind::Default
    } else {
        CeylonShaderSelectorKind::Simple
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeylonSimpleShaderFeatureDescriptor {
    pub position: u8,
    /// Per-position name stored by the binary descriptor table.
    pub feature_name: &'static str,
    /// Name accumulated into the emitted define. Shared multi-bit parameters
    /// use one name for all of their positions.
    pub registered_name: &'static str,
    pub value_bit: u8,
}

const fn feature(position: u8, feature_name: &'static str) -> CeylonSimpleShaderFeatureDescriptor {
    CeylonSimpleShaderFeatureDescriptor {
        position,
        feature_name,
        registered_name: feature_name,
        value_bit: 0,
    }
}

const fn parameter_bit(
    position: u8,
    feature_name: &'static str,
    registered_name: &'static str,
    value_bit: u8,
) -> CeylonSimpleShaderFeatureDescriptor {
    CeylonSimpleShaderFeatureDescriptor {
        position,
        feature_name,
        registered_name,
        value_bit,
    }
}

/// Exact 71-entry descriptor table at `0x189CD58`.
pub const CEYLON_SIMPLE_SHADER_FEATURES: [CeylonSimpleShaderFeatureDescriptor;
    CEYLON_SIMPLE_SHADER_FEATURE_COUNT] = [
    feature(0, "SSF_None"),
    feature(1, "SSF_UserShader"),
    feature(2, "SSF_2DTransform"),
    feature(3, "SSF_Vertex_Normal"),
    feature(4, "SSF_Vertex_NormalUByte4N"),
    feature(5, "SSF_Vertex_Tangent"),
    feature(6, "SSF_Vertex_TangentUByte4N"),
    feature(7, "SSF_Vertex_Binormal"),
    feature(8, "SSF_Vertex_BinormalUByte4N"),
    parameter_bit(9, "SSF_Vertex_ColorBit0", "SSF_Vertex_Color", 0),
    parameter_bit(10, "SSF_Vertex_ColorBit1", "SSF_Vertex_Color", 1),
    parameter_bit(11, "SSF_Vertex_TexcoordBit0", "SSF_Vertex_Texcoord", 0),
    parameter_bit(12, "SSF_Vertex_TexcoordBit1", "SSF_Vertex_Texcoord", 1),
    feature(13, "SSF_Vertex_BlendWeight"),
    feature(14, "SSF_Vertex_BlendIndices"),
    feature(15, "SSF_Vertex_PointSize"),
    parameter_bit(16, "SSF_OutColor_ModeBit0", "SSF_OutColor_Mode", 0),
    parameter_bit(17, "SSF_OutColor_ModeBit1", "SSF_OutColor_Mode", 1),
    feature(18, "SSF_OutDistance_Color0A"),
    feature(19, "SSF_OutDistance_Color0RD2G"),
    feature(20, "SSF_NoUpdateDistance"),
    parameter_bit(21, "SSF_OutVelocityBit0", "SSF_OutVelocity", 0),
    parameter_bit(22, "SSF_OutVelocityBit1", "SSF_OutVelocity", 1),
    feature(23, "SSF_PixelAlphaTest"),
    feature(24, "SSF_AlphaBlend"),
    feature(25, "SSF_SoftEdge"),
    feature(26, "SSF_ParticleShader"),
    feature(27, "SSF_ReductionMode"),
    parameter_bit(28, "SSF_Fog_ModeBit0", "SSF_Fog_Mode", 0),
    parameter_bit(29, "SSF_Fog_ModeBit1", "SSF_Fog_Mode", 1),
    parameter_bit(30, "SSF_Vtf_Fog_ModeBit0", "SSF_Vtf_Fog_Mode", 0),
    parameter_bit(31, "SSF_Vtf_Fog_ModeBit1", "SSF_Vtf_Fog_Mode", 1),
    parameter_bit(32, "SSF_LightEffectModeBit0", "SSF_LightEffectMode", 0),
    parameter_bit(33, "SSF_LightEffectModeBit1", "SSF_LightEffectMode", 1),
    parameter_bit(34, "SSF_LightParallelBit0", "SSF_LightParallel", 0),
    parameter_bit(35, "SSF_LightParallelBit1", "SSF_LightParallel", 1),
    feature(36, "SSF_BaseMap"),
    feature(37, "SSF_MultiTexMap0"),
    feature(38, "SSF_MultiTexMap1"),
    parameter_bit(39, "SSF_RefractionMapBit0", "SSF_RefractionMap", 0),
    parameter_bit(40, "SSF_RefractionMapBit1", "SSF_RefractionMap", 1),
    parameter_bit(41, "SSF_BlendModeBit0", "SSF_BlendMode", 0),
    parameter_bit(42, "SSF_BlendModeBit1", "SSF_BlendMode", 1),
    parameter_bit(43, "SSF_BlendModeBit2", "SSF_BlendMode", 2),
    parameter_bit(44, "SSF_BlendModeBit3", "SSF_BlendMode", 3),
    parameter_bit(45, "SSF_BlendModeBit4", "SSF_BlendMode", 4),
    parameter_bit(46, "SSF_BlendModeBit5", "SSF_BlendMode", 5),
    parameter_bit(
        47,
        "SSF_MultiTex0BlendModeBit0",
        "SSF_MultiTex0BlendMode",
        0,
    ),
    parameter_bit(
        48,
        "SSF_MultiTex0BlendModeBit1",
        "SSF_MultiTex0BlendMode",
        1,
    ),
    parameter_bit(
        49,
        "SSF_MultiTex0BlendModeBit2",
        "SSF_MultiTex0BlendMode",
        2,
    ),
    parameter_bit(
        50,
        "SSF_MultiTex0BlendModeBit3",
        "SSF_MultiTex0BlendMode",
        3,
    ),
    parameter_bit(
        51,
        "SSF_MultiTex1BlendModeBit0",
        "SSF_MultiTex1BlendMode",
        0,
    ),
    parameter_bit(
        52,
        "SSF_MultiTex1BlendModeBit1",
        "SSF_MultiTex1BlendMode",
        1,
    ),
    parameter_bit(
        53,
        "SSF_MultiTex1BlendModeBit2",
        "SSF_MultiTex1BlendMode",
        2,
    ),
    feature(54, "SSF_ShadowParallel"),
    parameter_bit(55, "SSF_ShadowMapBit0", "SSF_ShadowMap", 0),
    parameter_bit(56, "SSF_ShadowMapBit1", "SSF_ShadowMap", 1),
    feature(57, "SSF_ShadowMapEdgeHide"),
    parameter_bit(
        58,
        "SSF_ShadowMapColorShadowBit0",
        "SSF_ShadowMapColorShadow",
        0,
    ),
    parameter_bit(
        59,
        "SSF_ShadowMapColorShadowBit1",
        "SSF_ShadowMapColorShadow",
        1,
    ),
    parameter_bit(60, "SSF_ShadowMap0_ModeBit0", "SSF_ShadowMap0_Mode", 0),
    parameter_bit(61, "SSF_ShadowMap0_ModeBit1", "SSF_ShadowMap0_Mode", 1),
    parameter_bit(62, "SSF_ShadowMap0_ModeBit2", "SSF_ShadowMap0_Mode", 2),
    parameter_bit(63, "SSF_ShadowMap1_ModeBit0", "SSF_ShadowMap1_Mode", 0),
    parameter_bit(64, "SSF_ShadowMap1_ModeBit1", "SSF_ShadowMap1_Mode", 1),
    parameter_bit(65, "SSF_ShadowMap1_ModeBit2", "SSF_ShadowMap1_Mode", 2),
    parameter_bit(66, "SSF_ShadowMap2_ModeBit0", "SSF_ShadowMap2_Mode", 0),
    parameter_bit(67, "SSF_ShadowMap2_ModeBit1", "SSF_ShadowMap2_Mode", 1),
    parameter_bit(68, "SSF_ShadowMap2_ModeBit2", "SSF_ShadowMap2_Mode", 2),
    feature(69, "SSF_DepthWrite"),
    feature(70, "SSF_Debug"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CeylonSimpleShaderBits {
    words: [u64; 2],
}

impl CeylonSimpleShaderBits {
    pub const fn contains(self, position: usize) -> bool {
        position < CEYLON_SIMPLE_SHADER_FEATURE_COUNT
            && self.words[position / 64] & (1_u64 << (position % 64)) != 0
    }

    pub fn set(&mut self, position: usize, enabled: bool) {
        if position >= CEYLON_SIMPLE_SHADER_FEATURE_COUNT {
            return;
        }
        let mask = 1_u64 << (position % 64);
        if enabled {
            self.words[position / 64] |= mask;
        } else {
            self.words[position / 64] &= !mask;
        }
    }

    fn set_parameter(&mut self, first_position: usize, width: usize, value: u32) {
        for bit in 0..width {
            self.set(first_position + bit, value & (1 << bit) != 0);
        }
    }

    /// Applies one final integer selector-parameter value using the exact
    /// parameter-ID to feature-position vector registered by the binary.
    /// Activation and provider accumulation are intentionally outside this
    /// method; callers must supply the value present after those steps.
    pub fn set_selector_parameter_value(
        &mut self,
        parameter_id: u32,
        value: u32,
    ) -> Result<(), UnsupportedCeylonSimpleSelectorParameter> {
        let Some(descriptor) = CEYLON_SIMPLE_SELECTOR_PARAMETERS
            .iter()
            .find(|descriptor| descriptor.parameter_id == parameter_id)
        else {
            return Err(UnsupportedCeylonSimpleSelectorParameter { parameter_id });
        };
        for (value_bit, position) in descriptor.positions.iter().copied().enumerate() {
            self.set(usize::from(position), value & (1 << value_bit) != 0);
        }
        Ok(())
    }

    /// Reproduces the special parameter-10 branch in
    /// `sea_simple_shader_selector_build_compact_key`: an active
    /// `PassEnvWriteDistance` sets `SSF_OutDistance_Color0A` only when shape
    /// alpha blending is disabled. Parameter 10 is intentionally not part of
    /// `CEYLON_SIMPLE_SELECTOR_PARAMETERS` because the selector handles it
    /// directly instead of through the registered position vectors.
    pub fn set_write_distance_target_contribution(
        &mut self,
        parameter_10_value: u32,
        alpha_blend_enabled: bool,
    ) {
        self.set(18, parameter_10_value != 0 && !alpha_blend_enabled);
    }

    /// Reproduces the special parameter 43..48 branch in
    /// `sea_simple_shader_selector_build_compact_key`.
    ///
    /// Parameter 43 alone enables `SSF_ShadowParallel`. The remaining bits
    /// are read only when the shape's shadow-detail flag is enabled (the
    /// Ceylon ShapeEnv cache key's low bit 6). The binary clamps values with
    /// `min(value, 3)` or `min(value, 7)` before extracting their bits.
    pub fn set_shadow_parallel_contribution(
        &mut self,
        parameters: CeylonShadowParallelParameters,
        shadow_detail_enabled: bool,
    ) {
        let active = parameters.cascade_count != 0;
        self.set(54, active);

        let detailed = active && shadow_detail_enabled;
        self.set_parameter(
            55,
            2,
            if detailed {
                parameters.cascade_count.min(3)
            } else {
                0
            },
        );
        self.set(57, detailed && parameters.edge_hide != 0);
        self.set_parameter(
            58,
            2,
            if detailed {
                parameters.color_shadow.min(3)
            } else {
                0
            },
        );
        for (cascade, mode) in parameters.cascade_modes.into_iter().enumerate() {
            self.set_parameter(60 + cascade * 3, 3, if detailed { mode.min(7) } else { 0 });
        }
    }

    /// Applies the vertex inputs produced by Ceylon vertex format 14:
    /// COLOR0/COLOR1 and TEXCOORD0/TEXCOORD1. The selector records each pair
    /// as a two-bit count with value 2.
    pub fn apply_srd_vertex_format_14(&mut self) {
        self.set_parameter(9, 2, 2);
        self.set_parameter(11, 2, 2);
    }

    /// Applies the vertex inputs produced by Ceylon vertex format 13:
    /// COLOR0/COLOR1 and TEXCOORD0. `sub_671D30` case 13 registers exactly
    /// the first four elements of format 14 and omits only TEXCOORD1.
    pub fn apply_fennel_vertex_format_13(&mut self) {
        self.set_parameter(9, 2, 2);
        self.set_parameter(11, 2, 1);
    }

    /// Applies positions 36..38 once the selector's texture-feature gate is
    /// active. The binary sets BaseMap, MultiTexMap0 and MultiTexMap1 for the
    /// first, second and third non-null texture objects respectively.
    pub fn apply_enabled_texture_count(&mut self, texture_count: usize) {
        self.set(36, texture_count > 0);
        self.set(37, texture_count > 1);
        self.set(38, texture_count > 2);
    }

    /// Applies the three ShapeEnv parameters whose exact Simple-selector
    /// position vectors are registered by `sea::SimpleShaderSelector`.
    pub fn apply_shape_environment_variants(
        &mut self,
        blend_mode: u32,
        multi_tex0_blend_mode: u32,
        multi_tex1_blend_mode: u32,
    ) {
        self.set_selector_parameter_value(6, blend_mode)
            .expect("parameter 6 is registered");
        self.set_selector_parameter_value(7, multi_tex0_blend_mode)
            .expect("parameter 7 is registered");
        self.set_selector_parameter_value(8, multi_tex1_blend_mode)
            .expect("parameter 8 is registered");
    }

    /// Exact nibble encoding used by `sub_65EB40`: four feature positions per
    /// byte, low bit first, then ASCII `A` is added. Position 71 is padding.
    pub fn compact_key(self) -> [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] {
        let mut key = [b'A'; CEYLON_SIMPLE_SHADER_KEY_LENGTH];
        for (byte_index, byte) in key.iter_mut().enumerate() {
            let mut value = 0_u8;
            for bit in 0..4 {
                let position = byte_index * 4 + bit;
                if self.contains(position) {
                    value |= 1 << bit;
                }
            }
            *byte = b'A' + value;
        }
        key
    }

    /// Exact 18-byte decoder used by `sub_65EA80`. As in the game, only the
    /// low four bits after subtracting ASCII `A` affect feature positions.
    pub fn from_compact_key(key: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH]) -> Self {
        let mut bits = Self::default();
        for (byte_index, byte) in key.into_iter().enumerate() {
            let value = byte.wrapping_sub(b'A');
            for bit in 0..4 {
                let position = byte_index * 4 + bit;
                if position < CEYLON_SIMPLE_SHADER_FEATURE_COUNT {
                    bits.set(position, value & (1 << bit) != 0);
                }
            }
        }
        bits
    }

    /// Reproduces the Simple selector's registered-parameter accumulation and
    /// lexicographically ordered uppercase `#define` output.
    pub fn define_prefix(self) -> String {
        let mut values = BTreeMap::<String, i32>::new();
        for descriptor in CEYLON_SIMPLE_SHADER_FEATURES {
            let name = descriptor.registered_name.to_ascii_uppercase();
            let value = values.entry(name).or_default();
            if self.contains(usize::from(descriptor.position)) {
                *value += 1_i32 << descriptor.value_bit;
            }
        }

        let mut prefix = String::new();
        for (name, value) in values {
            prefix.push_str("#define ");
            prefix.push_str(&name);
            prefix.push(' ');
            prefix.push_str(&value.to_string());
            prefix.push('\n');
        }
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceylon_registry_places_simple_after_nine_default_selectors() {
        for index in 0..9 {
            assert_eq!(
                ceylon_shader_selector_kind(index),
                CeylonShaderSelectorKind::Default
            );
        }
        assert_eq!(
            ceylon_shader_selector_kind(CEYLON_SIMPLE_SHADER_SELECTOR_INDEX),
            CeylonShaderSelectorKind::Simple
        );
        assert_eq!(
            ceylon_shader_selector_kind(19),
            CeylonShaderSelectorKind::Simple
        );
    }

    #[test]
    fn embedded_simple_shader_prefixes_decode_with_the_binary_dword_transform() {
        let pixel = decode_embedded_shader_source(
            0x5273_75cc,
            &[0xaa, 0x1c, 0x3f, 0x26, 0xa8, 0x1e, 0x3d, 0x26],
        )
        .unwrap();
        let vertex = decode_embedded_shader_source(
            0x68d9_8052,
            &[0x34, 0xe9, 0x95, 0x1c, 0x36, 0xeb, 0x97, 0x1c],
        )
        .unwrap();
        assert_eq!(pixel, b"///-----");
        assert_eq!(vertex, b"///-----");
    }

    #[test]
    fn embedded_shader_decoder_rejects_unregistered_partial_dwords() {
        assert_eq!(
            decode_embedded_shader_source(0, &[0, 1, 2]),
            Err(EmbeddedShaderSourceLengthError { encoded_length: 3 })
        );
    }

    #[test]
    fn simple_descriptor_table_is_position_complete() {
        for (position, descriptor) in CEYLON_SIMPLE_SHADER_FEATURES.iter().enumerate() {
            assert_eq!(usize::from(descriptor.position), position);
        }
        assert_eq!(
            CEYLON_SIMPLE_SHADER_FEATURES[47].registered_name,
            "SSF_MultiTex0BlendMode"
        );
        assert_eq!(CEYLON_SIMPLE_SHADER_FEATURES[50].value_bit, 3);
        assert_eq!(
            CEYLON_SIMPLE_SHADER_FEATURES[53].registered_name,
            "SSF_MultiTex1BlendMode"
        );
    }

    #[test]
    fn simple_compact_key_round_trips_all_71_positions() {
        let mut bits = CeylonSimpleShaderBits::default();
        for position in 0..CEYLON_SIMPLE_SHADER_FEATURE_COUNT {
            bits.set(position, position % 3 == 1);
        }
        let key = bits.compact_key();
        assert_eq!(key.len(), 18);
        assert_eq!(CeylonSimpleShaderBits::from_compact_key(key), bits);
        assert_eq!((key[17] - b'A') & 8, 0);
    }

    #[test]
    fn shape_environment_parameters_land_on_the_registered_simple_bits() {
        let mut bits = CeylonSimpleShaderBits::default();
        bits.apply_shape_environment_variants(0x23, 9, 5);
        for bit in 0..6 {
            assert_eq!(bits.contains(41 + bit), 0x23 & (1 << bit) != 0);
        }
        assert!(bits.contains(47));
        assert!(!bits.contains(48));
        assert!(!bits.contains(49));
        assert!(bits.contains(50));
        assert!(bits.contains(51));
        assert!(!bits.contains(52));
        assert!(bits.contains(53));
    }

    #[test]
    fn complete_selector_parameter_table_matches_constructor_order_and_widths() {
        assert_eq!(
            CEYLON_SIMPLE_SELECTOR_PARAMETERS
                .iter()
                .map(|descriptor| (descriptor.parameter_id, descriptor.positions))
                .collect::<Vec<_>>(),
            vec![
                (1, &[2][..]),
                (6, &[41, 42, 43, 44, 45, 46][..]),
                (7, &[47, 48, 49, 50][..]),
                (8, &[51, 52, 53][..]),
                (9, &[16, 17][..]),
                (11, &[19][..]),
                (22, &[27][..]),
                (23, &[25][..]),
                (24, &[39, 40][..]),
                (30, &[26][..]),
                (40, &[34, 35][..]),
                (52, &[28, 29][..]),
                (53, &[30, 31][..]),
                (61, &[32, 33][..]),
                (66, &[69][..]),
                (68, &[70][..]),
            ]
        );
    }

    #[test]
    fn selector_parameter_values_set_only_their_registered_value_bits() {
        for descriptor in CEYLON_SIMPLE_SELECTOR_PARAMETERS {
            let mut bits = CeylonSimpleShaderBits::default();
            let value = 0xaaaa_aaaa;
            bits.set_selector_parameter_value(descriptor.parameter_id, value)
                .unwrap();
            for position in 0..CEYLON_SIMPLE_SHADER_FEATURE_COUNT {
                let expected = descriptor
                    .positions
                    .iter()
                    .position(|candidate| usize::from(*candidate) == position)
                    .is_some_and(|value_bit| value & (1 << value_bit) != 0);
                assert_eq!(bits.contains(position), expected);
            }
        }

        let mut bits = CeylonSimpleShaderBits::default();
        assert_eq!(
            bits.set_selector_parameter_value(67, 1),
            Err(UnsupportedCeylonSimpleSelectorParameter { parameter_id: 67 })
        );
        assert_eq!(bits, CeylonSimpleShaderBits::default());
    }

    #[test]
    fn write_distance_target_is_a_separate_non_blended_context_branch() {
        let mut bits = CeylonSimpleShaderBits::default();
        bits.set_write_distance_target_contribution(1, false);
        assert!(bits.contains(18));

        bits.set_write_distance_target_contribution(1, true);
        assert!(!bits.contains(18));

        bits.set_write_distance_target_contribution(0, false);
        assert!(!bits.contains(18));
    }

    #[test]
    fn light_shadow_parallel_properties_emit_exact_selector_parameters() {
        assert_eq!(
            CeylonShadowParallelParameters::from_light_shadow_parallel(0, 4, true, 2),
            CeylonShadowParallelParameters {
                cascade_count: 1,
                edge_hide: 1,
                color_shadow: 2,
                cascade_modes: [4, 0, 0],
            }
        );
        assert_eq!(
            CeylonShadowParallelParameters::from_light_shadow_parallel(2, 4, false, 3),
            CeylonShadowParallelParameters {
                cascade_count: 2,
                edge_hide: 0,
                color_shadow: 3,
                cascade_modes: [4, 1, 0],
            }
        );
        assert_eq!(
            CeylonShadowParallelParameters::from_light_shadow_parallel(9, 1, false, 0),
            CeylonShadowParallelParameters {
                cascade_count: 3,
                edge_hide: 0,
                color_shadow: 0,
                cascade_modes: [1, 0, 0],
            }
        );
    }

    #[test]
    fn shadow_parallel_selector_branch_matches_gate_and_clamps() {
        let parameters = CeylonShadowParallelParameters {
            cascade_count: 4,
            edge_hide: 7,
            color_shadow: 4,
            cascade_modes: [8, 2, 1],
        };

        let mut ungated = CeylonSimpleShaderBits::default();
        ungated.set_shadow_parallel_contribution(parameters, false);
        assert!(ungated.contains(54));
        assert!((55..=68).all(|position| !ungated.contains(position)));

        let mut gated = CeylonSimpleShaderBits::default();
        gated.set_shadow_parallel_contribution(parameters, true);
        assert!(gated.contains(54));
        assert!(gated.contains(55));
        assert!(gated.contains(56));
        assert!(gated.contains(57));
        assert!(gated.contains(58));
        assert!(gated.contains(59));
        assert!((60..=62).all(|position| gated.contains(position)));
        assert!(!gated.contains(63));
        assert!(gated.contains(64));
        assert!(!gated.contains(65));
        assert!(gated.contains(66));
        assert!(!gated.contains(67));
        assert!(!gated.contains(68));

        gated.set_shadow_parallel_contribution(CeylonShadowParallelParameters::default(), true);
        assert!((54..=68).all(|position| !gated.contains(position)));
    }

    #[test]
    fn srd_vertex_format_and_enabled_texture_count_set_only_proven_positions() {
        let mut bits = CeylonSimpleShaderBits::default();
        bits.apply_srd_vertex_format_14();
        bits.apply_enabled_texture_count(2);
        assert!(!bits.contains(9));
        assert!(bits.contains(10));
        assert!(!bits.contains(11));
        assert!(bits.contains(12));
        assert!(bits.contains(36));
        assert!(bits.contains(37));
        assert!(!bits.contains(38));
    }

    #[test]
    fn shadercollect_variant_nine_key_decodes_and_emits_exact_define() {
        let key = *b"AAEBABBAADIIEAAAAA";
        let bits = CeylonSimpleShaderBits::from_compact_key(key);
        assert_eq!(bits.compact_key(), key);
        let prefix = bits.define_prefix();
        assert!(prefix.contains("#define SSF_MULTITEX0BLENDMODE 9\n"));
        assert!(prefix.contains("#define SSF_MULTITEX1BLENDMODE 0\n"));
        assert!(prefix.contains("#define SSF_VERTEX_COLOR 2\n"));
        assert!(prefix.contains("#define SSF_VERTEX_TEXCOORD 2\n"));
    }
}
