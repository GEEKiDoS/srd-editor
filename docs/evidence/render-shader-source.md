# Ceylon ShaderSelector source、公式与编译链证据

本页记录 Ceylon 如何把紧凑 shader 选择键变成 Cg `#define` 前缀、取得并展开原始 source、组合 vertex/pixel Shader 输入，以及 SRD 双 UV/双顶点色在 Simple shader 中的真实消费顺序。

分析对象：`chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；保存后的 IDB SHA-256 `63CFC0D28318D936CF0F134B0C7328047D21C3488565C0E97F6CFED1DEFEA187`。

## ShaderSelector 对象与双阶段缓存

`sea_shader_selector_construct` (`0x6BDC60`) 的 RTTI 指向 `sea::ShaderSelector`。同一 virtual slot 的派生 RTTI 还证明存在 `SimpleShaderSelector`、`DefaultShaderSelector`、`DefaultPostEffectSelector` 和 `PostEffectSelector`。

`sea_shader_selector_get_or_create_shader_pair` (`0x6BE2A0`) 对两个阶段分别查找缓存。单阶段 cache miss 时：

1. 调用当前 selector 的 virtual `+0x10`，以紧凑键建立 source 前缀；
2. 使用 selector 内保存的阶段 source body、资源名前缀和 owner；
3. 以资源名前缀与紧凑键组成资源/cache 名；
4. 调用 `ceylon_create_shader_from_memory` (`0xE9D900`)；
5. 把两个阶段分别写入各自缓存和调用者输出位置。

两个阶段共享紧凑键和派生 selector 的前缀规则，但 source、资源名、owner 与缓存相互独立。

## SRD 使用 Simple selector 槽位 9

`ceylon_initialize_shader_selectors_and_post_effects` (`0x6217F0`) 对 `g_sea_shader_manager` 的首个 selector vector 连续插入 9 个 Default，再插入一个 Simple。vector 初始为空，insert (`0x61D350`) 在无空洞时返回追加前长度，所以槽位严格为 Default `0..8`、Simple `9`。同一 manager 的后续字段还保存嵌入式 shader source 文件表，因此该全局对象不是单独的 registry。

`ceylon_create_shape_environment` (`0x670680`) 在 cache miss 时构造 `sea::PrimitiveDummyShape`，随后执行 `push 1; push 9; mov ecx, shape; call sea_shape_set_shader_selector`。setter (`0x6AFC70`) 用 selector 字节查上述 vector。cache hit 返回同一 shape，绘制入口再通过其 virtual `+0x2C` 执行。因此 SRD/ShapeEnv 使用 Simple 是对象与调用链结论，不是文件名推断。

## 前缀与 source body 的拼接

`ceylon_shader_initialize_source` (`0xE9D000`) 保存 stage byte 和 resource id，再进入 `ceylon_shader_set_source_with_optional_prefix` (`0xE9D2B0`)。无前缀时原样复制 source；有前缀时：

```text
prefix_size = strlen(prefix) + 1
allocate(prefix_size + source_size)
copy prefix including trailing NUL
replace that NUL with '\n'
copy source body immediately after it
```

最终输入是“全部 uppercase `#define` 前缀 + 一个换行 + 原始 Cg source”，不是 compiler macro 参数数组。Default 的 46 字节键机制和 Simple 的 18 字节键/71 项表分别见本页关联文档。

## 可执行文件内的原始 source 与精确解码

`sea_register_embedded_shader_sources` (`0x61FD70`) 逐项调用 `sea_shader_manager_register_embedded_source` (`0x623B40`)，按文件名保存 `(resource_hash, encoded_pointer, byte_count)`。`sea_decode_embedded_shader_source` (`0x61F030`) 对每个 little-endian dword 执行：

```text
decoded_dword = encoded_dword XOR resource_hash XOR 0x59634649
```

循环步长严格为 4；全部已注册记录的 byte count 都是 4 的倍数。Rust 的 `decode_embedded_shader_source` 保存该变换，对非整 dword 输入显式报错。

Simple 主 source 与专属 include 的记录为：

| 文件 | hash | encoded VA | bytes |
| --- | ---: | ---: | ---: |
| `SimpleShaderPS.cg` | `0x527375CC` | `0x17C3C60` | `0x2884` |
| `SimpleShaderVS.cg` | `0x68D98052` | `0x17C64F0` | `0x1D98` |
| `PhotoShopLayerBlendPS.h` | `0x4A65FBF5` | `0x17FBD10` | `0x2C3C` |
| `SimpleCommonFunc.h` | `0x267431F4` | `0x17FE958` | `0x04F4` |
| `SimplePSFunc.h` | `0xC16317DF` | `0x17FEE50` | `0x09A8` |
| `SimplePSUniform.h` | `0xE47776FD` | `0x17FF800` | `0x0998` |
| `SimpleShaderDefine.h` | `0xF415D5A2` | `0x18001A0` | `0x0BB4` |
| `SimpleVSUniform.h` | `0x876A4C71` | `0x1800D60` | `0x0858` |

解码后的 VS/PS 均为可读 Cg，入口名均为 `main`，尾部由 NUL 对齐。`sea_load_and_expand_shader_source` (`0x6BF4C0`) 优先从 manager 文件表读取，失败才尝试外部文件，并递归展开 `#include`。Simple 闭包还精确包含 `DefaultColorCompress.h`、`DefaultShadow.h`、`FixedPSUniform.h`、`FixedVSUniform.h` 和 `ParticleSystem.h`。source 来源、字节、include 依赖与预处理入口均已闭环。

## SRD 双 UV 与双顶点色公式

`SimpleShaderDefine.h` 的输入与 format 14 一致：POSITION、COLOR0、COLOR1、TEXCOORD0、TEXCOORD1。`SimpleShaderVS.cg` 在 `SSF_VERTEX_TEXCOORD == 2` 时把 TEXCOORD0 写到 `_texCoord01.xy`，把 TEXCOORD1 写到 `_texCoord01.zw`，并在启用 MultiTex1 时写到 `_texCoord23.xy`。

`SimpleShaderPS.cg` 的顺序为：

1. `SSF_BASEMAP`：`textureBase` 采样 `_texCoord01.xy`；
2. `SSF_MULTITEXMAP0`：`textureMultiTex0` 采样 `_texCoord01.zw`，调用 `multiTexBlned(multi, current, SSF_MULTITEX0BLENDMODE)`；
3. `SSF_MULTITEXMAP1`：`textureMultiTex1` 采样 `_texCoord23.xy`，以同一函数继续组合；
4. `SSF_VERTEX_COLOR`：结果乘 COLOR0；值大于 1 时再把 COLOR1.rgb 加到结果 rgb；
5. 随后才执行 blend-mode 特殊分支、深度/soft-edge/shadow/refraction/fog 与 `calcCompressColor`。

`DefaultColorCompress.h` 证明 `multiTexBlned` 的 SRD 相关 mode：`9` 用 source 覆盖 rgb，并写 `src.a + dest.a * 0.00001`；`10` 用 source alpha 在 dest/src rgb 间插值；`11` 把 source.r 写入 dest alpha；`12` 用 source.r 乘 dest alpha。完整 data 的 Simple collection 实际包含 MultiTex0 mode `9/10/11/12`，使键值、宏值和像素公式形成独立交叉校验。

## Vertex/pixel 阶段与原游戏编译链

`ceylon_create_pixel_shader_resource` (`0xE957C0`) 写 stage `0`；`ceylon_create_vertex_shader_resource` (`0xE96220`) 写 stage `1`。`tea_cg_create_program` (`0x1319E30`) 以 `CG_SOURCE (0x1010)`、入口 `main` 创建 Cg program。`tea_d3d9_compile_cg_shader` (`0x131A080`) 再以 `CG_COMPILED_PROGRAM (0x100A)` 取得 D3D assembly，调用 `D3DXAssembleShader`，然后：

- stage `1` -> `IDirect3DDevice9::CreateVertexShader`，vtable `+0x16C`；
- stage `0` -> `IDirect3DDevice9::CreatePixelShader`，vtable `+0x1A8`。

这条 D3DX 路径只用于还原原游戏。编辑器不链接、加载或调用 D3DX。离线工具现已用原版 Cg 参数生成完整 Simple collection 的 assembly，并由系统 `D3DCompiler_47!D3DAssemble` 无失败地产生确定性 D3D9 bytecode，详见 [`render-shader-bytecode.md`](render-shader-bytecode.md)。

## 当前边界

已经闭环：selector、前缀格式与拼接、嵌入 source 解码、全部 Simple include、双 UV/双顶点色消费顺序、MultiTex 公式、Cg 到 D3D assembly 的原游戏链，以及最终 D3D9 stage 创建。

仍需闭环：64 位 ShapeEnv key 的其余 base/context 输入如何生成每一种完整 18 字节 Simple 键，以及 bytecode 表的发布期封装与编辑器 runtime draw submission 接入。完整 bytecode 已由独立 D3D9 HAL device 实际创建，见 [`render-shader-bytecode.md`](render-shader-bytecode.md)。
