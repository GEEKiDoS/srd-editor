# Ceylon ShapeEnv shader key 证据

本页记录 SRD 绘制包如何选择 Ceylon `sea::ShapeEnv*` 模块组合。它闭环的是 shader cache key 和模块索引，不把尚未还原的生成源码或像素公式写成结论。

分析对象：`chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；保存后的 IDB SHA-256 `893CD6866DAE3C958988FAA9F3D2AA32405CD6E5927819D8310BC5AE080C11CE`。

## 绘制包默认值与 SRD vertex format

`ceylon_construct_draw_packet` (`0x6B8B10`) 先调用 `0x6CD8A0` 建立 304 字节绘制包。与 shader key 直接相关的默认写入为：

- packet `+0x28` = `31`；
- packet `+0x2C` = `0`；
- packet `+0x30/+0x34/+0x38` 三个纹理槽清零；
- packet `+0x60` 清零后设置 bit `0x4000`；
- render preset setter 后续会更新 packet `+0x00` 低六位及 `+0x60` 的派生位。

该构造函数写入 draw flags `0x00AFE000`，`srd_construct_renderer` 随后以 `0xFF79FFFF` 清除 SRD 不保留的位，因此 SRD renderer 的初始 packet `+0x00` 为 `0x0029E000`。packet `+0x58` 低字节为 `0xFF`，`+0x60` 为 `0x4000`。Rust 的 `CeylonDrawPacketPresetState::srd_renderer_initial` 保存这组已经由构造链闭环的初值。

`srd_begin_quad_draw` 通过 builder 虚表 `+8` 请求 vertex format `14`、primitive type `4`、vertex count `4`。落点 `ceylon_begin_vertex_batch` (`0x6DE970`) 把 format 写到 packet `+0x70`，把 primitive type 写到 packet `+0x74`。因此 SRD quad 进入 shader-key 计算时的 vertex-format 字段已由写入点证明为 `14`。

## 64 位 cache key

`sub_671480` 生成 `(low, high)` 两个 u32。Rust `CeylonShaderKeyInput::shader_key` 逐条保留原表达式和回绕行为。按位来源为：

| key 字段 | 绘制包来源 |
| --- | --- |
| low `0..2` | packet `+0x2C` low 3 bits |
| low `3` | packet `+0x60` bit 7 |
| low `4` | 当前 render-preset 32 字节记录的首位，即 alpha-blend enable |
| low `5` | packet `+0x00` bit 17 |
| low `6` | packet `+0x60` bit 3 |
| low `7` | packet `+0x60` bit 14 |
| low `8..12` | packet `+0x28` low 5 bits |
| low `13..14` | 三个纹理槽中非空槽的数量，按槽逐次加一并以两位回绕 |
| low `15..19` | packet `+0x70` low 5 bits；SRD quad 为 14 |
| low `20..29` | packet `+0x00` low 10 bits |
| high `0..2` | packet `+0x00` bits `10..12` |

函数先以 packet preset 低六位调用 `ceylon_get_render_preset_record`，因此这里使用的是经过该 accessor 边界规则选择的真实表项，不是根据 preset 编号猜测 alpha 行为。

## SRD 的两个纹理槽

renderer `+0xF0` 是 vertex builder，builder `+0x20` 是绘制包，所以 renderer `+0x140/+0x144` 分别就是 packet `+0x30/+0x34`。`srd_select_render_texture_pair` 在这两个地址分别保存 CREF 和 CRE1 的显式覆盖或 TEXL pair 选择；提交函数从 packet `+0x30` 起遍历三槽，packet copy 又原位复制三槽。因此：

```text
CREF -> packet texture slot 0 -> D3D9 stage 0
CRE1 -> packet texture slot 1 -> D3D9 stage 1
SRD packet texture slot 2 remains null
```

这也闭环了 key 的纹理数量来源：它是上述两个独立结果中非空项的数量，范围为 0..2；不是根据是否存在 CREF/CRE1 子块推测。

## CIMG/CNUM 0x4C 对 key 的影响

`srd_init_srimage_from_cimg` 和 `srd_init_srimage_from_cnum` 把解析字段 `0x4C` 复制到 `SrImage+0x0C`；CSLI 初始化不覆盖构造函数写入的零。`srd_begin_quad_draw` 将该值按 signed 规则夹到 `0..4`，从表 `[0, 9, 0xA00, 0xB0000, 0x0C000000]` 取值，左移 6，然后只替换 packet draw flags 的 `0x3C0` 位。

在这个掩码下，只有值 `1` 产生 bits `6..9 = 9`；`0/2/3/4` 都产生零。它们进入 key low `26..29`，因而分别选择 `ShapeEnvMultiTex0BlendMode` variant `9` 或 `0`。Rust 的 `apply_srd_image_field_0c_shader_bits` 保留 clamp、位移和掩码顺序。

## ShapeEnv 模块索引

cache miss 进入 `sub_670680`，建立 328 字节组合资源并按 key 选择：

- `this + 3 + ((low >> 15) & 0x1F)`：内建 vertex-format/environment 资源；SRD 对应 format 14；
- manager `+0x150` vector：`low & 7`，实际只构造 5 项：`0 = null`、`1 = sea::ShapeEnvSoftEdge`、`2 = sea::ShapeEnvRefraction`、`3 = sea::ShapeEnvRefraction2`、`4 = sea::ShapeEnvDepthWrite`；
- manager `+0x180` 模块：RTTI 为 `sea::ShapeEnv2D`，low bit 3 非零时附加；
- manager `+0x15C` 的 62 项 `sea::ShapeEnvBlendMode`：`(low >> 20) & 0x3F`；
- manager `+0x168` 的 13 项 `sea::ShapeEnvMultiTex0BlendMode`：`(low >> 26) & 0x0F`；
- manager `+0x174` 的 8 项 `sea::ShapeEnvMultiTex1BlendMode`：`high & 7`。

构造函数 `ceylon_environment_manager_construct` (`0x66E4D0`) 的 push 顺序、各对象 RTTI/vtable 和每个对象保存的 variant index 共同证明上述模块类型及数量。`ceylon_create_shape_environment` 对 base vector 使用未经取模或边界检查的 `low & 7` 原始索引；因为构造器只建立 5 项，所以 `5..7` 不是可接受的已构造 variant，Rust 明确拒绝它们，而不虚构映射。`ShapeEnvBlendMode`、`MultiTex0`、`MultiTex1` 分别通过参数描述符 `0x1CAF86C/0x1CAF87C/0x1CAF88C` 把 variant 请求送入组合器。

## 完整游戏数据的初始 key 回归

回归测试同时覆盖旧 53 个 SRD 样本和 `D:\sdhd\assets\data\surfboard` 的完整 91 个 SRD。测试按 CIMG/CSLI/CNUM 初始化路径建立每个 image node 的运行时状态，应用 CATR `ExtParamData` preset 覆盖，解析 CREF/CRE1 槽位，再使用上述 packet 初值计算真实初始 key。

完整语料的 19,484 个 image node 得到：

- texture mask：无纹理 `2,387`，仅 slot 0 `16,357`，仅 slot 1 `22`，slot 0+1 `718`；
- `SrImage+0x0C`：`0` 为 `18,720`，`1` 为 `3`，`2` 为 `3`，`3` 为 `89`，`4` 为 `669`；
- 65 个不同的初始 `(low, high)` key；
- base environment (`low & 7`) 全部为 variant `0`，可选 `+0x180` 模块全部关闭；
- `MultiTex0` variant `0` 为 `19,481`，variant `9` 为 `3`；`MultiTex1` 全部为 variant `0`。

旧语料的 15,124 个 image node 得到 23 个不同初始 key，texture mask 分布为 `2,125/12,433/12/554`，`MultiTex0` 同样只有三个 variant `9`。这些断言是语料回归的一部分，不是手工观察日志。

这里的边界是“资源初始化后的首帧状态”。动画可以改变 CREF/CRE1 selector 或显式矩形，从而改变纹理槽是否非空；不能把上述 65 个 key 宣称为所有动画帧的全集。

## 原游戏 shader 编译链与编辑器约束

原游戏的 `tea_d3d9_compile_cg_shader` (`0x131A080`) 使用 NVIDIA Cg 生成 D3D assembly 字符串，再调用导入的 `D3DXAssembleShader`，最后通过 D3D9 device 创建 vertex/pixel shader。这个事实只用于追踪原始 shader 的生成来源。

ShaderSelector 的注册顺序和 SRD 对 Simple 槽位 9 的选择已经闭环。Simple 使用 18 字节键和 71 项 descriptor；Default 的 46 字节键属于其他游戏路径。两者的前缀格式、前缀与原始 Cg source 的精确拼接，以及 stage 0/1 到 pixel/vertex 的映射见 [`render-shader-source.md`](render-shader-source.md)，SRD 使用的 Simple 表见 [`render-simple-selector.md`](render-simple-selector.md)。

编辑器按用户约束不链接、不加载也不调用 D3DX，并且不绑定原游戏的 x86 指令集。最终后端必须在精确 bytecode/公式完成证明后，采用宿主机架构无关的 D3D9 shader 资源方案；在证据完成前不选择或虚构替代编译结果。

## 证据边界

已经闭环：所有 shader-key 输入位、CREF/CRE1 到 slot 0/1 的映射、纹理槽计数、SRD vertex format、cache 查找、ShapeEnv 模块族/variant 索引，以及完整语料初始化状态的实际 key 集合统计。

仍需闭环：动画遍历后的完整运行时 ShapeEnv key 集合，以及其余 context 输入到完整 18 字节 Simple key 的映射。base vector `0..4` 与可选 `ShapeEnv2D` 模块已经闭环。原始 Cg source、像素公式和完整 collection 的无 D3DX bytecode 已闭环，见 [`render-shader-source.md`](render-shader-source.md) 与 [`render-shader-bytecode.md`](render-shader-bytecode.md)。
