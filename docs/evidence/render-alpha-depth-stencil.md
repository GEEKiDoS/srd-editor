# SRD Alpha、深度与模板 D3D9 状态证据

本页记录从 `SrImage` 运行时字段、Ceylon draw packet、临时 RenderState 到 D3D9 `SetRenderState` 的已闭环链路。没有证据支持的业务字段名仍保留为原始偏移名。

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`4B57E934C0ADBFEEC21349F4AC64AFE67879A8AA95A6205C91BEBE2667F2C412`

关键函数和表：

- `srd_srimage_construct` (`0xAD2150`)
- `srd_begin_quad_draw` (`0xAC5320`)
- `ceylon_set_stencil_always_replace_packet` (`0x6CDAF0`)
- `ceylon_set_stencil_compare_packet` (`0x6CDB70`)
- `ceylon_reset_stencil_packet` (`0x6CD840`)
- `ceylon_apply_draw_packet_state` (`0x6CEE30`)
- `d3d9_map_comparison_function` (`0xE5AF80`)
- `d3d9_map_stencil_operation` (`0xE5B4B0`)
- `ceylon::scene::element::Material` 构造器 `sub_E8C7F0`
- Material command 构造器 `sub_E93070`
- stencil command 构造器 `sub_E93230`
- `d3d9_flush_blend_and_alpha_state` (`0xE5D2D0`)
- `d3d9_flush_depth_state` (`0xE5D740`)
- `d3d9_flush_stencil_state` (`0xE5DB20`)
- `d3d9_comparison_function_table` (`0x19306B8`)
- `d3d9_stencil_operation_table` (`0x1930708`)

## SrImage 原始字段与构造初值

`srd_srimage_construct` 将下列字段置零，CIMG、CNUM 和 CSLI 的三个已定位初始化器都不覆盖它们：

| SrImage 偏移 | 初值 | 已证明的低层用途 |
|---:|---:|---|
| `+0x10` | `0` | `srd_begin_quad_draw` 的 alpha/stencil packet 分支选择 |
| `+0x14` | `0` | 作为 x86 `SHL DL,CL` 的移位计数生成 8 位 stencil mask |
| `+0x18` | `0` | 可阻止 `+0x10 == 3/4` 的专用分支并转入 fallback |

Rust 对每个独立 `RuntimeImageState` 保存这三个构造初值。当前没有证据说明它们来自哪个 SRD 属性或具有何种上层业务含义，所以没有将其命名为“mask mode”等推测名称。

## SrImage 到 draw packet

`srd_begin_quad_draw` 首先执行与 x86 完全相同的 8 位移位。有效计数是 `SrImage+0x14 & 0x1F`；结果写入 `DL`，因此计数 `8..31` 的结果为零，而不是 Rust 宽整数的非零位。

设该结果为 `image_mask`，renderer `+0x274` 的字节为 `renderer_mask`：

| `SrImage+0x10` 与条件 | packet `+0x08` 低 16 位 | ref/mask/write-mask 字节 | `flags+0x0C bit 0x100` | `flags+0x00 & 0x1E000` | 其他副作用 |
|---|---:|---|---:|---:|---|
| `1` 或 `2` | `0x2001` | 均为 `image_mask` | 设置 | 清除 | renderer `+0x198` 的低字节原生递减并允许回绕 |
| `3` 且 `+0x18 == 0` | comparison `2` | 均为 `image_mask | renderer_mask` | 设置 | 设置 | 无 |
| `4` 且 `+0x18 == 0` | comparison `3` | 均为 `image_mask | renderer_mask` | 设置 | 设置 | 无 |
| 其他且 `renderer_mask != 0` | comparison `2` | 均为 `renderer_mask` | 设置 | 设置 | 无 |
| 其余 | `0` | `0` | 清除 | 设置 | 无 |

表中的三个字节位于 packet `+0x0A`、`+0x0B` 和 `+0x0C` 低字节。这里只陈述写入事实；`flags+0x00 & 0x1E000` 的更高层含义尚未独立闭环。

renderer `+0x25D` 非零时，同一函数还根据 `SrImage+0x1C` 修改深度 packet：负值清除 `flags+0x00 bit 0x20000`，非负值设置该位；随后总是设置 `bit 0x40000` 并令 packet `+0x2C = 4`。该分支还会修改 renderer 私有的排序字段，当前 Rust helper 明确只覆盖已经表示的 draw-packet 部分，未伪造其余状态。

## draw packet 到临时 RenderState

`ceylon_apply_draw_packet_state` 先复制调用者提供的基础 RenderState，再套用预设和 packet 覆盖。深度字段直接来自 `packet.flags+0x00`：

| packet 位 | RenderState 字段 |
|---:|---|
| bit 18 (`0x40000`) | `+0x30` Z enable |
| bit 17 (`0x20000`) | `+0x31` Z write enable |
| bits 19..22 | `+0x34` internal Z comparison |

packet `flags+0x0C bit 0x100` 总是决定 RenderState `+0x40` stencil enable。该位未设置时不会覆写其余 alpha/stencil 基础字段；设置时，`packet+0x08` 和 `+0x0C` 解码为：

| packet 位 | RenderState 字段 |
|---:|---|
| `+0x08 bits 0..3` | `+0x44` stencil comparison |
| `+0x08 bits 4..7` | `+0x48` stencil fail op |
| `+0x08 bits 8..11` | `+0x4C` stencil Z-fail op |
| `+0x08 bits 12..15` | `+0x50` stencil pass op |
| `+0x08 bits 16..23` | `+0x54` stencil reference |
| `+0x08 bits 24..31` | `+0x58` stencil mask |
| `+0x0C bits 0..7` | `+0x5C` stencil write mask |

当低四位 comparison 为 internal `1` 时，函数另外强制 `RenderState+0x24 = 1` 和 `+0x28 = 128`，即开启 alpha test 并设置 alpha reference；它不改写 `+0x2C` 的基础 alpha comparison。

draw packet 构造器写入的基线 flags 是 `0x00AFE000`。这可解码为 Z enable、Z write enable 和 internal comparison `5`，但它是该构造路径的 packet 初值，不应被扩张成所有调用上下文的全局默认状态。

默认 Material 的基础 alpha/stencil 输入也已由构造链闭环。`sub_E8C7F0` 对 Material `+0x58` 调用 `sub_E93070`；后者写入 alpha test disabled、reference `0`、internal comparison `6`。comparison 表证明 internal `6` 为 D3D9 `GREATER`，所以默认 Material 上 alpha-test preset 的最终条件是 source alpha `> 0`。`sub_E93230` 则把默认 stencil 写为 disabled、comparison/op 字段 `1`、reference/mask/write-mask 为零。Rust 的 `CeylonAlphaStencilState::default_material()` 固化这些构造值；独立编辑器宿主在没有外部自定义 Material 时使用它们，packet stencil override 仍因 sequence 生命周期未闭合而显式拒绝。

## Internal 枚举到 D3D9

`d3d9_comparison_function_table` 的完整映射为：

```text
internal 0 -> NEVER(1)       internal 1 -> ALWAYS(8)
internal 2 -> EQUAL(3)       internal 3 -> NOTEQUAL(6)
internal 4 -> LESS(2)        internal 5 -> LESSEQUAL(4)
internal 6 -> GREATER(5)     internal 7 -> GREATEREQUAL(7)
```

`d3d9_stencil_operation_table` 的 internal `0..7` 依次映射为 D3D9 `KEEP(1)`、`ZERO(2)`、`REPLACE(3)`、`INCRSAT(4)`、`DECRSAT(5)`、`INVERT(6)`、`INCR(7)`、`DECR(8)`。两个 accessor 都只接受 `0..7`；Rust 返回 `Option`，不会为越界值猜测 fallback。

## 最终 D3D9 RenderState

三个 backend flush 函数逐字段缓存比较，并在值变化或强制刷新时调用 `IDirect3DDevice9::SetRenderState`：

| RenderState 偏移 | 转换 | D3D9 state |
|---:|---|---|
| `+0x24` | u8 直传 | `ALPHATESTENABLE` (`15`) |
| `+0x28` | u32 直传 | `ALPHAREF` (`24`) |
| `+0x2C` | comparison table | `ALPHAFUNC` (`25`) |
| `+0x30` | u8 转布尔 | `ZENABLE` (`7`) |
| `+0x31` | u8 直传 | `ZWRITEENABLE` (`14`) |
| `+0x34` | comparison table | `ZFUNC` (`23`) |
| `+0x38` | signed i32 转 f32 后乘常量，按位传递 | `DEPTHBIAS` (`195`) |
| `+0x3C` | f32 符号位 XOR，按位传递 | `SLOPESCALEDEPTHBIAS` (`175`) |
| `+0x40` | u8 直传 | `STENCILENABLE` (`52`) |
| `+0x44` | comparison table | `STENCILFUNC` (`56`) |
| `+0x48` | stencil-op table | `STENCILFAIL` (`53`) |
| `+0x4C` | stencil-op table | `STENCILZFAIL` (`54`) |
| `+0x50` | stencil-op table | `STENCILPASS` (`55`) |
| `+0x54` | u32 直传 | `STENCILREF` (`57`) |
| `+0x58` | u32 直传 | `STENCILMASK` (`58`) |
| `+0x5C` | u32 直传 | `STENCILWRITEMASK` (`59`) |

### Depth bias 的位级行为

`d3d9_flush_depth_state` 在 `0xE5D812..0xE5D834` 执行 `movd`、`cvtdq2ps`、`mulss`，再把结果 DWORD 传给 state 195。乘数位于 `0x1914A80`，原始字节为 `BD 37 86 B5`，按 little-endian u32 读取为 `3045472189`。Rust 使用 `f32::from_bits(3045472189)`，没有用十进制近似值替换它；输入零因此产生负零位模式。

`0xE5D868..0xE5D885` 对 slope 值执行 `xorps xmm0, xmmword_1796060`。该 16 字节常量是四个重复的 f32 符号位 mask。Rust 直接 XOR `to_bits()`，从而与游戏一样翻转正负零并保留 NaN 的指数、尾数和载荷。

## 本地语料与测试边界

53 个本地 SRD 共构造 15,124 个 CIMG/CSLI/CNUM 图像型运行时对象；其 `SrImage+0x10/+0x14/+0x18` 构造初值全部为零。样本没有覆盖这些字段的非零来源，因此非零分支由二进制控制流、helper 写入和最终 D3D9 消费证明，不能声称已找到对应 SRD 属性。

Rust 单元测试覆盖两个完整映射表、移位计数 `8` 的 x86 低字节结果、counter 回绕、所有 packet 分支、alpha reference 128 的条件覆盖、模板字段解码、深度 flags、特殊深度分支，以及 depth/slope bias 的有符号零和 NaN 位行为。

## 当前证据边界

本页闭环的是上述 alpha/depth/stencil packet 字段及其最终 D3D9 状态。后续已经闭环的 draw/material scissor 状态见 [`render-scissor-state.md`](render-scissor-state.md)。基础 RenderState 中 alpha reference/function 和 depth-bias 数值的更上游来源、renderer 私有排序字段、纹理创建、shader 与 draw 前后完整状态恢复仍需分别追踪。Rust API 因而要求调用者提供基础 alpha/stencil 状态，并继续用原始偏移名保存未命名字段。
