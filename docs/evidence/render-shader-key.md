# Ceylon ShapeEnv shader key 证据

本页记录 SRD 绘制包如何选择 Ceylon `sea::ShapeEnv*` 模块组合。它闭环的是 shader cache key 和模块索引，不把尚未还原的生成源码或像素公式写成结论。

分析对象：`chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；保存后的 IDB SHA-256 `4B54743085DC6EA79F10DFDA84A37A9B400FC9A204AB0635A0619473B441757E`。

## 绘制包默认值与 SRD vertex format

`ceylon_construct_draw_packet` (`0x6B8B10`) 先调用 `0x6CD8A0` 建立 304 字节绘制包。与 shader key 直接相关的默认写入为：

- packet `+0x28` = `31`；
- packet `+0x2C` = `0`；
- packet `+0x30/+0x34/+0x38` 三个纹理槽清零；
- packet `+0x60` 清零后设置 bit `0x4000`；
- render preset setter 后续会更新 packet `+0x00` 低六位及 `+0x60` 的派生位。

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

## ShapeEnv 模块索引

cache miss 进入 `sub_670680`，建立 328 字节组合资源并按 key 选择：

- `this + 3 + ((low >> 15) & 0x1F)`：内建 vertex-format/environment 资源；SRD 对应 format 14；
- manager `+0x150` vector：`low & 7`；
- manager `+0x180` 模块：low bit 3 非零时附加；
- manager `+0x15C` 的 62 项 `sea::ShapeEnvBlendMode`：`(low >> 20) & 0x3F`；
- manager `+0x168` 的 13 项 `sea::ShapeEnvMultiTex0BlendMode`：`(low >> 26) & 0x0F`；
- manager `+0x174` 的 8 项 `sea::ShapeEnvMultiTex1BlendMode`：`high & 7`。

构造函数 `sub_66E4D0` 的循环数量、各对象 RTTI/vtable 和每个对象保存的 variant index 共同证明上述模块类型及数量。`ShapeEnvBlendMode`、`MultiTex0`、`MultiTex1` 分别通过参数描述符 `0x1CAF86C/0x1CAF87C/0x1CAF88C` 把 variant 请求送入组合器。

## 原游戏 shader 编译链与编辑器约束

原游戏的 `sub_131A080` 使用 NVIDIA Cg 生成 D3D assembly 字符串，再调用导入的 `D3DXAssembleShader`，最后通过 D3D9 device 创建 vertex/pixel shader。这个事实只用于追踪原始 shader 的生成来源。

编辑器按用户约束不链接、不加载也不调用 D3DX，并且不绑定原游戏的 x86 指令集。最终后端必须在精确 bytecode/公式完成证明后，采用宿主机架构无关的 D3D9 shader 资源方案；在证据完成前不选择或虚构替代编译结果。

## 证据边界

已经闭环：所有 shader-key 输入位、纹理槽计数、SRD vertex format、cache 查找以及 ShapeEnv 模块族/variant 索引。

仍需闭环：SRD 的 CREF/CRE1 两个选择结果最终分别进入哪几个 packet 纹理槽、每个实际 preset/纹理数量对应的完整 key 集合、模块化 Cg 源码拼接、最终 D3D9 bytecode，以及 COLOR0/COLOR1、TEXCOORD0/TEXCOORD1 在像素阶段的具体公式。
