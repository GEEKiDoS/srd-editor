# Ceylon ShaderSelector 源码前缀与 Shader 资源证据

本页记录 Ceylon 如何把紧凑 shader 选择键变成 Cg `#define` 前缀，并把它与原始源码缓冲组合成 vertex/pixel Shader 资源。它不把尚未闭环的 ShapeEnv 参数名、像素公式或最终 bytecode 写成结论。

分析对象：`chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；保存后的 IDB SHA-256 `736C88EC383DC51EE559AEA5B19E69D7A10751D1EDE39A6628BEC502DC8BCC00`。

## ShaderSelector 对象与双阶段缓存

`sea_shader_selector_construct` (`0x6BDC60`) 安装的 vtable complete-object locator 指向 RTTI `sea::ShaderSelector`。同一 common virtual slot 的派生 vtable RTTI 还证明存在：

- `sea::SimpleShaderSelector`；
- `sea::DefaultShaderSelector`；
- `sea::DefaultPostEffectSelector`；
- `sea::PostEffectSelector`。

`sea_shader_selector_get_or_create_shader_pair` (`0x6BE2A0`) 对两个阶段分别查找缓存。单阶段 cache miss 时，它执行以下已由参数和字段写入证明的步骤：

1. 调用当前 selector 的 virtual `+0x10`，以紧凑选择键建立可选源码前缀字符串；
2. 以 selector 内保存的原始源码指针和长度作为 source body；
3. 以 selector 的阶段资源名前缀与紧凑键组成资源/cache 名；
4. 调用 `ceylon_create_shader_from_memory` (`0xE9D900`) 建立对应 Shader resource；
5. 将得到的资源保存到该阶段缓存，并把两个阶段的结果分别写回调用者提供的输出位置。

两个阶段使用独立的源码缓冲、资源名、owner 和缓存，但共享同一个紧凑选择键和由派生 selector 生成的前缀逻辑。

## DefaultShaderSelector 的 46 字节紧凑键

`sea_default_shader_selector_build_define_prefix` (`0x662EE0`) 先调用 `sea_decode_compact_shader_define_bits` (`0x661350`)。后者清零 24 字节 bitset，然后读取恰好 46 个键字节：

```text
value = key_byte - ASCII('A')
feature_position = byte_index * 4 + bit_index
bit_index = 0..3
```

只保留 `feature_position < 180` 的位置。因此前 45 个字节覆盖 180 个 feature，函数仍读取的第 46 个字节对应位置 `180..183`，不会设置任何有效位；函数不会把字符当十六进制文本解析，也不会读取高四位。

`sea_default_shader_selector_append_define_prefix` (`0x6612D0`) 依次扫描 180 个 feature position。每个置位位置交给 `0x6C50F0` 查找已注册的参数描述符，并把该描述符对应的位值累加到参数记录。随后 `sea_append_shader_defines` (`0x6C5160`) 按参数 map 顺序写入：

```text
#define <registered_name> <signed_decimal_value>\n
```

字符串片段、空格、`%d` 十进制格式和换行均来自调用点常量。这里可以确认生成格式和 feature-position 机制，但尚不能在没有上游描述符表证据时给 180 个位置自行命名。

## 前缀与原始 Cg source 的组合

`ceylon_shader_construct` (`0xE9C910`) 建立 64 字节 `ceylon::resource::Shader`。`ceylon_shader_initialize_source` (`0xE9D000`) 写入：

- Shader `+0x24`：stage byte；
- Shader `+0x3C`：12-bit resource slot/id；
- 随后进入 `ceylon_shader_set_source_with_optional_prefix` (`0xE9D2B0`)。

没有前缀时，后者分配 `source_size` 字节并原样复制 source body。存在前缀时，它执行：

```text
prefix_size = strlen(prefix) + 1
allocate(prefix_size + source_size)
copy prefix including trailing NUL
replace that trailing NUL with '\n'
copy source body immediately after it
```

最终缓冲指针和长度分别写入 Shader `+0x14/+0x18`，额外字段写入 `+0x1C`。因此最终传给编译器的输入是“生成的 `#define` 前缀 + 一个换行 + 原始 Cg source”，不是多个独立 compiler macro 参数。

## Vertex/pixel 阶段证明

`ceylon_create_pixel_shader_resource` (`0xE957C0`) 以 stage byte `0` 初始化 Shader；`ceylon_create_vertex_shader_resource` (`0xE96220`) 以 stage byte `1` 初始化 Shader。`ceylon_shader_compile` (`0xE9D030`) 把该字节和最终 source 缓冲提交到后端。

原游戏后端 `tea_d3d9_compile_cg_shader` (`0x131A080`) 在 Cg 成功后取得 D3D assembly 字符串并调用 `D3DXAssembleShader`。最终 device vtable 调用直接证明：

- stage `1` -> `IDirect3DDevice9::CreateVertexShader`，vtable `+0x16C`；
- stage `0` -> `IDirect3DDevice9::CreatePixelShader`，vtable `+0x1A8`。

这条 D3DX 路径只用于还原原游戏行为。编辑器按约束不链接、加载或调用 D3DX；最终实现必须在 bytecode 或等价公式获得完整证据后，采用独立于 D3DX 且不受原游戏 x86 指令集限制的方案。

## 与 ShapeEnv 的当前闭环边界

`ceylon_create_shape_environment` 建立 RTTI 为 `sea::PrimitiveDummyShape` 的组合对象，并按 64 位 ShapeEnv key 注册 vertex-format、base、blend、MultiTex0 和 MultiTex1 模块。该对象的专用 shader-cache virtual (`0x6CF3D0`) 会进入全局 ShaderSelector registry，因此 ShapeEnv 与上述 selector 系统已在对象和调用层连接。

仍需闭环：

- SRD/ShapeEnv 实际选择的是哪个 concrete ShaderSelector 实例；
- 64 位 ShapeEnv key 和模块参数如何生成那 46 字节紧凑键；
- 180 个 feature position 对应的完整 registered define name/value 表；
- selector 内两份原始 Cg source 的来源和精确字节；
- 完整 vertex/pixel Cg 公式以及不依赖 D3DX 的最终 D3D9 bytecode。

在这些链路闭环之前，Rust 只保存已经证明的 ShapeEnv key、模块 variant 和纹理槽状态，不生成推测性的 shader 宏或像素公式。
