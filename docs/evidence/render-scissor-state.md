# SRD 绘制链中的 Scissor 状态证据

本页记录 SRD draw packet 提交过程中 scissor 状态的来源选择、RenderState 保留以及最终 D3D9 调用。该状态来自 Ceylon material 同步链；当前没有证据表明它由 SRD 文件中的某个属性直接提供。

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`AAFE2FE1FCCCC5E4BD44993EEE1B8FD45E4F89D7ABC323C485560E967D57CE4E`

关键函数：

- `sub_671350` (`0x671350`)：选择 draw packet 并取得其缓存的 draw/material 上下文
- `sub_671480` (`0x671480`)：由 packet 字段构造 material cache key
- `ceylon_apply_draw_packet_state` (`0x6CEE30`)
- `sea_material_sync_render_commands` (`0x659810`)
- `ceylon_scissor_state_command_construct` (`0xE931F0`)
- `ceylon_apply_scissor_state_command` (`0xE935C0`)
- `ceylon_copy_render_state` (`0x6CE390`)
- `ceylon_assign_render_state` (`0x6CE550`)
- `ceylon_reset_render_state_defaults` (`0xE5BE10`)
- `d3d9_flush_raster_and_scissor_state` (`0xE5D900`)

## Draw 前的 material 同步顺序

`sub_671350` 以 304 字节步长找到 draw packet，并通过 `sub_671480` 获得缓存上下文。`ceylon_apply_draw_packet_state` 在复制基础 RenderState 之前先执行：

```text
context+0x64 -> sea::Material object
virtual method +0x14 -> sea_material_sync_render_commands
```

该顺序由 `0x6CEE60..0x6CEE6B` 和随后 `0x6CEECD..0x6CEEDD` 的汇编直接证明。material 命令先更新全局 RenderState，函数之后才把它复制到栈上的临时 RenderState、套用 blend/depth/stencil packet 覆盖并写回。因此 draw packet 没有 scissor 专用覆盖字段时，material 选出的 scissor 状态仍会被完整保留。

## ScissorStateCommand 布局

`ceylon_scissor_state_command_construct` 将 24 字节对象初始化为：

| 偏移 | 类型 | 初值 | 已证明用途 |
|---:|---|---:|---|
| `+0x00` | pointer | vftable | `ceylon::command::ScissorStateCommand` |
| `+0x04` | u8 | `0` | scissor enable |
| `+0x08` | i32 | `0` | RECT left |
| `+0x0C` | i32 | `0` | RECT top |
| `+0x10` | i32 | `0` | RECT right |
| `+0x14` | i32 | `0` | RECT bottom |

四个值在命令应用和 D3D9 backend 中都按原始 16 字节整体复制，不发生 clamp、排序或无符号转换。

## Material 的两个独立来源选择

`sea_material_sync_render_commands` 在 material 同步版本改变或 `material+0x138` override mask 非零时建立两个来源：

```text
source[0] = 当前基础 StateParam + 0x3C
source[1] = material + 0x178
```

scissor 的 enable 与 rectangle 分别选择来源，不能合并为一个“是否整体 override”的判断：

| override mask | 选择行为 |
|---:|---|
| `0x02000000` | 选择 enable 来源；取该来源 `+0x00 bit 0x20` |
| `0x04000000` | 选择 rectangle 来源；复制该来源 `+0x24..+0x33` |

例如仅设置 `0x02000000` 时，enable 来自 material override，而 rectangle 仍来自基础 StateParam；仅设置 `0x04000000` 时正好相反。`0x659AFA` 与 `0x659B18` 的两个独立条件选择以及 `0x659B80` 的命令应用调用证明了这一点。

Rust 因而分别表示基础/override source，并按两个位独立组装 `CeylonScissorStateCommand`。没有将 rectangle 与 enable 强行绑定到同一来源。

## 命令到 RenderState

`ceylon_apply_scissor_state_command` 的完整函数只有以下有效写入：

```text
global RenderState+0x64 = command+0x04
global RenderState+0x68 = command+0x08 的 16 字节 RECT
```

`ceylon_copy_render_state` 和 `ceylon_assign_render_state` 都分别复制 `+0x64` 字节、`+0x65` 字节以及 `+0x68` 的 16 字节块，所以 scissor 状态会穿过临时 draw RenderState，不受当前已知 packet alpha/depth/stencil 覆盖影响。

`ceylon_reset_render_state_defaults` 的精确初值是：

```text
RenderState+0x64 enable = 0
RenderState+0x68 RECT   = { 5, 0, 0, 0 }
```

虽然 disabled 时 D3D9 不使用 rectangle，Rust 仍保存 `left = 5`，没有用更“自然”的全零矩形替换游戏构造值。独立的 ScissorStateCommand 构造器矩形则确实是全零；两种默认不能混淆。

## 最终 D3D9 调用

`d3d9_flush_raster_and_scissor_state` 对当前值与 backend cache 逐项比较：

- `RenderState+0x64` 变化或强制刷新时，调用 `SetRenderState(D3DRS_SCISSORTESTENABLE, value)`；枚举值为 `174`。
- `RenderState+0x68..+0x77` 任一 LONG 变化或强制刷新时，把四个 LONG 组成的 RECT 指针传给 D3D9 device vtable `+0x12C`，即 `IDirect3DDevice9::SetScissorRect`。

函数即使在 enable 为零时也会独立比较和刷新 rectangle；Rust 状态模型同样不以 enable 为条件丢弃矩形。

## 当前证据边界

本页闭环的是 draw/material scissor source selection、命令布局、RenderState 传递和 D3D9 消费。基础 StateParam 与 material override 数据更上游由谁设置、SRD player 是否在特定宿主场景中收到非默认 scissor，以及 draw 前后跨系统的完整状态恢复仍需继续追踪。当前代码不会虚构 SRD 标签或属性来控制 scissor；编辑器后端应把它作为外部 material/render context 状态输入。
