# SRD SimpleShaderSelector 选择、紧凑键与 feature 表

本页记录 SRD/Ceylon ShapeEnv 实际选择的 selector、Simple 键的字节编码、完整 71 项 descriptor，以及已经闭环的 ShapeEnv 模块参数映射。结论来自游戏二进制调用链，并以完整游戏 data 目录中的 shader collection 作独立语料校验。

分析对象：`chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；保存后的 IDB SHA-256 `893CD6866DAE3C958988FAA9F3D2AA32405CD6E5927819D8310BC5AE080C11CE`。

## selector 槽位 9

`ceylon_initialize_shader_selectors_and_post_effects` (`0x6217F0`) 使用同一个 `g_sea_shader_manager` 的首个 selector vector：

1. `0x621A60..0x621C54` 循环 9 次构造 Default selector，并在 `0x621C40` 调用 registry insert；
2. `0x621C5A..0x621D78` 构造 Simple selector；
3. `0x621DC2` 把 Simple 对象插入同一个 registry。

`sea_shader_selector_registry_insert` (`0x61D350`) 先检查重复对象和空槽；registry 初始为空且上述对象均为新分配，所以每次都追加到 vector 尾部，返回值是追加前的长度。由此严格得到 Default 槽位 `0..8`、Simple 槽位 `9`。lookup (`0x61F4A0`) 使用 `requested_index % current_count` 取 vector 元素。

`ceylon_create_shape_environment` (`0x670680`) 构造 `sea::PrimitiveDummyShape` 后，在 `0x6706D2..0x6706DF` 执行：

```text
push 1
push 9
mov  ecx, shape
call sea_shape_set_shader_selector
```

`sea_shape_set_shader_selector` (`0x6AFC70`) 用 selector 字节查 registry，并把它写入 `Shape+0xA8`；第二个字节写入 `Shape+0xB0`。cache hit 时 `ceylon_select_shape_environment` 返回此前保存的同一 shape，随后 Ceylon 绘制入口通过该 shape 的 virtual `+0x2C` 执行。因此 SRD ShapeEnv 使用 Simple selector 是实际对象与调用链结论，不是依据 shader 文件名推断。

## 18 字节紧凑键

Simple selector 的 virtual `+0x0C` 是 `sea_simple_shader_selector_build_compact_key` (`0x660120`)；完成 71 位 bitset 后调用 `sea_simple_shader_encode_compact_key` (`0x65EB40`)。编码循环每次读取连续四个 feature position，低位在前：

```text
nibble = bit[p+0] * 1
       | bit[p+1] * 2
       | bit[p+2] * 4
       | bit[p+3] * 8
key_byte = ASCII('A') + nibble
p += 4
```

循环覆盖到 `p == 72`，因此输出恰好 18 字节；有效位置只有 `0..70`，最后一个字符的 bit 3 对应 padding position 71，编码时恒为零。

Simple selector 的 virtual `+0x10` 先由 `sea_simple_shader_decode_compact_key` (`0x65EA80`) 读取恰好 18 个字符。每个字符执行 `byte - 'A'`，只检查其低四位，并且只写入 `< 71` 的位置；没有十六进制文本解析步骤。

## 完整 71 项 descriptor

`sea_simple_shader_selector_construct` (`0x65ED50`) 在 `0x65EE3A` 把 `0x189CD58` 的 71 个、每项 16 字节的记录注册到公共 define manager。记录包括 position、逐位名称、可选共享参数名称和共享值的 bit index。共享名称为空时，逐位名称本身就是 define 名称。注册器把实际 define 名称转为大写作为有序 map key，所以最终前缀输出的是下表 `define` 列的大写形式。

| pos | descriptor | define | value bit |
|---:|---|---|---:|
| 0 | `SSF_None` | `SSF_None` | 0 |
| 1 | `SSF_UserShader` | `SSF_UserShader` | 0 |
| 2 | `SSF_2DTransform` | `SSF_2DTransform` | 0 |
| 3 | `SSF_Vertex_Normal` | `SSF_Vertex_Normal` | 0 |
| 4 | `SSF_Vertex_NormalUByte4N` | `SSF_Vertex_NormalUByte4N` | 0 |
| 5 | `SSF_Vertex_Tangent` | `SSF_Vertex_Tangent` | 0 |
| 6 | `SSF_Vertex_TangentUByte4N` | `SSF_Vertex_TangentUByte4N` | 0 |
| 7 | `SSF_Vertex_Binormal` | `SSF_Vertex_Binormal` | 0 |
| 8 | `SSF_Vertex_BinormalUByte4N` | `SSF_Vertex_BinormalUByte4N` | 0 |
| 9 | `SSF_Vertex_ColorBit0` | `SSF_Vertex_Color` | 0 |
| 10 | `SSF_Vertex_ColorBit1` | `SSF_Vertex_Color` | 1 |
| 11 | `SSF_Vertex_TexcoordBit0` | `SSF_Vertex_Texcoord` | 0 |
| 12 | `SSF_Vertex_TexcoordBit1` | `SSF_Vertex_Texcoord` | 1 |
| 13 | `SSF_Vertex_BlendWeight` | `SSF_Vertex_BlendWeight` | 0 |
| 14 | `SSF_Vertex_BlendIndices` | `SSF_Vertex_BlendIndices` | 0 |
| 15 | `SSF_Vertex_PointSize` | `SSF_Vertex_PointSize` | 0 |
| 16 | `SSF_OutColor_ModeBit0` | `SSF_OutColor_Mode` | 0 |
| 17 | `SSF_OutColor_ModeBit1` | `SSF_OutColor_Mode` | 1 |
| 18 | `SSF_OutDistance_Color0A` | `SSF_OutDistance_Color0A` | 0 |
| 19 | `SSF_OutDistance_Color0RD2G` | `SSF_OutDistance_Color0RD2G` | 0 |
| 20 | `SSF_NoUpdateDistance` | `SSF_NoUpdateDistance` | 0 |
| 21 | `SSF_OutVelocityBit0` | `SSF_OutVelocity` | 0 |
| 22 | `SSF_OutVelocityBit1` | `SSF_OutVelocity` | 1 |
| 23 | `SSF_PixelAlphaTest` | `SSF_PixelAlphaTest` | 0 |
| 24 | `SSF_AlphaBlend` | `SSF_AlphaBlend` | 0 |
| 25 | `SSF_SoftEdge` | `SSF_SoftEdge` | 0 |
| 26 | `SSF_ParticleShader` | `SSF_ParticleShader` | 0 |
| 27 | `SSF_ReductionMode` | `SSF_ReductionMode` | 0 |
| 28 | `SSF_Fog_ModeBit0` | `SSF_Fog_Mode` | 0 |
| 29 | `SSF_Fog_ModeBit1` | `SSF_Fog_Mode` | 1 |
| 30 | `SSF_Vtf_Fog_ModeBit0` | `SSF_Vtf_Fog_Mode` | 0 |
| 31 | `SSF_Vtf_Fog_ModeBit1` | `SSF_Vtf_Fog_Mode` | 1 |
| 32 | `SSF_LightEffectModeBit0` | `SSF_LightEffectMode` | 0 |
| 33 | `SSF_LightEffectModeBit1` | `SSF_LightEffectMode` | 1 |
| 34 | `SSF_LightParallelBit0` | `SSF_LightParallel` | 0 |
| 35 | `SSF_LightParallelBit1` | `SSF_LightParallel` | 1 |
| 36 | `SSF_BaseMap` | `SSF_BaseMap` | 0 |
| 37 | `SSF_MultiTexMap0` | `SSF_MultiTexMap0` | 0 |
| 38 | `SSF_MultiTexMap1` | `SSF_MultiTexMap1` | 0 |
| 39 | `SSF_RefractionMapBit0` | `SSF_RefractionMap` | 0 |
| 40 | `SSF_RefractionMapBit1` | `SSF_RefractionMap` | 1 |
| 41 | `SSF_BlendModeBit0` | `SSF_BlendMode` | 0 |
| 42 | `SSF_BlendModeBit1` | `SSF_BlendMode` | 1 |
| 43 | `SSF_BlendModeBit2` | `SSF_BlendMode` | 2 |
| 44 | `SSF_BlendModeBit3` | `SSF_BlendMode` | 3 |
| 45 | `SSF_BlendModeBit4` | `SSF_BlendMode` | 4 |
| 46 | `SSF_BlendModeBit5` | `SSF_BlendMode` | 5 |
| 47 | `SSF_MultiTex0BlendModeBit0` | `SSF_MultiTex0BlendMode` | 0 |
| 48 | `SSF_MultiTex0BlendModeBit1` | `SSF_MultiTex0BlendMode` | 1 |
| 49 | `SSF_MultiTex0BlendModeBit2` | `SSF_MultiTex0BlendMode` | 2 |
| 50 | `SSF_MultiTex0BlendModeBit3` | `SSF_MultiTex0BlendMode` | 3 |
| 51 | `SSF_MultiTex1BlendModeBit0` | `SSF_MultiTex1BlendMode` | 0 |
| 52 | `SSF_MultiTex1BlendModeBit1` | `SSF_MultiTex1BlendMode` | 1 |
| 53 | `SSF_MultiTex1BlendModeBit2` | `SSF_MultiTex1BlendMode` | 2 |
| 54 | `SSF_ShadowParallel` | `SSF_ShadowParallel` | 0 |
| 55 | `SSF_ShadowMapBit0` | `SSF_ShadowMap` | 0 |
| 56 | `SSF_ShadowMapBit1` | `SSF_ShadowMap` | 1 |
| 57 | `SSF_ShadowMapEdgeHide` | `SSF_ShadowMapEdgeHide` | 0 |
| 58 | `SSF_ShadowMapColorShadowBit0` | `SSF_ShadowMapColorShadow` | 0 |
| 59 | `SSF_ShadowMapColorShadowBit1` | `SSF_ShadowMapColorShadow` | 1 |
| 60 | `SSF_ShadowMap0_ModeBit0` | `SSF_ShadowMap0_Mode` | 0 |
| 61 | `SSF_ShadowMap0_ModeBit1` | `SSF_ShadowMap0_Mode` | 1 |
| 62 | `SSF_ShadowMap0_ModeBit2` | `SSF_ShadowMap0_Mode` | 2 |
| 63 | `SSF_ShadowMap1_ModeBit0` | `SSF_ShadowMap1_Mode` | 0 |
| 64 | `SSF_ShadowMap1_ModeBit1` | `SSF_ShadowMap1_Mode` | 1 |
| 65 | `SSF_ShadowMap1_ModeBit2` | `SSF_ShadowMap1_Mode` | 2 |
| 66 | `SSF_ShadowMap2_ModeBit0` | `SSF_ShadowMap2_Mode` | 0 |
| 67 | `SSF_ShadowMap2_ModeBit1` | `SSF_ShadowMap2_Mode` | 1 |
| 68 | `SSF_ShadowMap2_ModeBit2` | `SSF_ShadowMap2_Mode` | 2 |
| 69 | `SSF_DepthWrite` | `SSF_DepthWrite` | 0 |
| 70 | `SSF_Debug` | `SSF_Debug` | 0 |

`sea_simple_shader_append_define_prefix` (`0x65EA10`) 先把所有已注册 define 的值清零，再扫描全部 71 个位置。每个置位 descriptor 向其 define 累加 `1 << value_bit`。`sea_append_shader_defines` (`0x6C5160`) 随后按大写 map key 的字典序输出所有 define，包括值仍为零的 define：

```text
#define <UPPERCASE_REGISTERED_NAME> <signed_decimal_value>\n
```

## ShapeEnv 参数 ID 6/7/8

Simple 构造函数建立 parameter ID 到 position vector 的精确映射：

| ShapeEnv parameter ID | positions | emitted parameter |
|---:|---|---|
| 6 | `41..46` | `SSF_BLENDMODE`，6 位 |
| 7 | `47..50` | `SSF_MULTITEX0BLENDMODE`，4 位 |
| 8 | `51..53` | `SSF_MULTITEX1BLENDMODE`，3 位 |

`sea_simple_shader_apply_parameter_positions` (`0x65E950`) 对每个 parameter value 从 bit 0 开始检查；置位的 value bit 设置对应 vector 元素指向的 feature position。此前 ShapeEnv 模块调用链已经证明 Blend、MultiTex0、MultiTex1 分别写 parameter ID `6/7/8`，所以三项 variant 到最终 Simple define 的映射现已闭环。

例如 MultiTex0 variant `9` 的二进制值为 `1001b`，因此设置 positions `47` 与 `50`。在紧凑键中 position 47 是第 12 个字符的 bit 3，position 50 是第 13 个字符的 bit 2，局部编码恰为 `I`、`E`。

## 两套参数表与 `ShapeEnv2D` 的 position 2 来源

`sea_simple_shader_selector_construct` (`0x65ED50`) 还精确注册了 parameter ID `1` 到 position `2`，即 `SSF_2DTransform`。这是 selector 自身的静态映射，不依赖 shader collection XML。

游戏的 parameter manager 与 parameter set 各自维护两套互不混用的表：

- named resource parameter registry 位于 manager `+0xB4`，parameter set 的资源指针 vector 位于 `+0xC0`。`sea_shader_parameter_registry_intern` (`0x630FD0`)、`sea_append_param_util_register_slot` 与 `sea_append_param_util_append_slot` 操作这套表；
- integer selector parameter registry 位于 manager `+0xC0`，parameter set 的整数 value vector 位于 `+0xCC`。`sub_65CAC0(local_slot, integer_id)` 把 utility local slot 映射到指定的固定整数 ID，`sea_shader_parameter_set_get` (`0x6BBF10`) 与 `sub_6BC540` 读取/写入这套表。

因此 named 参数的注册顺序不会产生 integer selector parameter ID。此前把 `AllEnvBasic` 的名称与 parameter ID `1` 联系起来的解释不成立；它的 7 个逻辑名称全部属于 named resource 表，不能直接设置 Simple feature position。

对 executable 中所有 `sub_65CAC0` 调用点的完整枚举显示，固定整数 ID `1` 只有一个注册点：`sea_shape_env_2d_register_selector_parameter` (`0x6CF4B0`) 将 `sea::ShapeEnv2D` 的 local slot `0` 映射到 ID `1`。其 RTTI、构造和应用链进一步闭环为：

1. `ceylon_environment_manager_construct` (`0x66E4D0`) 构造 RTTI 为 `sea::ShapeEnv2D` 的对象并保存到 manager `+0x180`；
2. `ceylon_create_shape_environment` (`0x670680`) 仅在 cache key low bit `3` 非零时把 `manager+0x180` 应用到 shape；
3. `sea_shape_apply_environment_module` (`0x6AFD10`) 调用模块 virtual `+0x18`，落到 `sea_shape_env_2d_apply` (`0x6CF560`)；
4. 该方法通过 `sea_append_param_util_raise_selector_value` (`0x65D320`) 写 local slot `0` 的值 `1`。后者先读取当前整数 selector value，再写入 `max(current, 1)`；最终目标就是 parameter set `+0xCC` 的 ID `1` 项；
5. Simple selector 将 ID `1` 的 value bit `0` 映射到 position `2`，得到 `SSF_2DTransform`。

base environment vector 是另一条独立路径。构造器只按顺序加入 `null/SoftEdge/Refraction/Refraction2/DepthWrite` 五项，对应 variants `0..4`；不存在 base variant `5 = ShapeEnv2D`。因此 Rust 已删除旧的 variant `5 -> position 2` 映射，并拒绝 base variants `5..7`。position `2` 的已证直接 key 来源只有 low bit `3` 的可选 `ShapeEnv2D` 模块。

SRD quad 的 ShapeEnv cache key 能直接证明的 Simple contributions 已单独实现：format 14 的 color/texcoord 位、实际 texture slot 数、alpha blend、反向的 `NoUpdateDistance`、base environment `0..4`、low bit `3` 的 `ShapeEnv2D`，以及 ShapeEnv parameter `6/7/8`。shader collection 中“只差 position 2”的键仍只作语料诊断，不能反向证明某次运行时 draw 启用了 `ShapeEnv2D`。

## 完整 data 目录的独立校验

完整游戏数据 `D:\sdhd\assets\data\A000\shader\shadercollect.xml` 包含 `SimpleShaderVSSimpleShaderPS_-382718031` 组。该组有 82 个键，全部严格为 18 字节；Rust 对每个键执行二进制同构解码再编码，82 个均逐字节回到原值。

按 positions `47..50` 解码，82 个 Simple shader key 的 MultiTex0 variant 分布为：`0:58`、`6:1`、`9:1`、`10:1`、`11:10`、`12:11`。其中唯一的 variant `9` 键是 `AAEBABBAADIIEAAAAA`，其 `I/E` 位型精确得到数值 `9`。这份 XML 只作为完整资源语料对二进制算法的独立校验；selector 选择、position 语义和编码规则仍由上述 executable 调用链决定。

## 当前实现边界

Rust 已实现：

- selector registry 的 9 个 Default + 1 个 Simple 类型映射；
- Simple 的完整 71 项 descriptor；
- 18 字节键的逐位编解码；
- 所有 uppercase define 的清零、累加和有序前缀输出；
- ShapeEnv parameter `6/7/8` 到 positions `41..53` 的映射；
- base environment `0..4` 与 low bit `3` 的 `ShapeEnv2D -> SSF_2DTransform` 映射，并拒绝未构造的 base `5..7`；
- 完整 data 中 82 个 Simple key 的回归。

`SimpleShaderVS.cg/SimpleShaderPS.cg` 的原始 source、include 闭包、双 UV/顶点色公式和完整 collection 的无 D3DX bytecode 已闭环，见 [`render-shader-source.md`](render-shader-source.md) 与 [`render-shader-bytecode.md`](render-shader-bytecode.md)。尚未闭环的是其余 shape/context feature 输入如何在 SRD 的每一种运行时状态下形成全部 positions，以及 bytecode 的 runtime 选择/设备接入。
