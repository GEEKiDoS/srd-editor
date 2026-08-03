# SRD 图像混合预设与 D3D9 状态证据

本页只记录已经由游戏二进制的 CAST 绘制入口、draw packet 编码、内建预设表和 D3D9 backend 状态刷新共同闭环的结论。分析对象：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`DA404C7D7A321F8D87A0190B3867EC3BDCC157B07AD124E9105AC345ED39C777`

关键函数：

- `srd_select_image_render_preset` (`0xAC6D50`)
- `srd_image_special_blend_mode` (`0xAD2880`)
- `ceylon_set_draw_render_preset_id` (`0x6CDAA0`)
- `ceylon_refresh_draw_preset_derived_flags` (`0x6CDA10`)
- `ceylon_get_render_preset_record` (`0x6CD870`)
- `ceylon_apply_draw_packet_state` (`0x6CEE30`)
- `d3d9_map_blend_factor` (`0xE5AF40`)
- `d3d9_map_blend_operation` (`0xE5AF60`)
- `d3d9_flush_blend_and_alpha_state` (`0xE5D2D0`)

## SrImage 运行时字段

`srd_srimage_construct` (`0xAD2150`) 明确初始化：

| SrImage 偏移 | 初值 | 已证明用途 |
|---|---:|---|
| `+0x1C` | `-1` | 特殊预设 20/21 的运行时序列值；其他具体业务语义仍不命名 |
| `+0xA0` | `-1` | 有符号 render-preset override |

`srd_init_srimage_from_cimg`、`srd_init_srimage_from_cnum` 和 `srd_init_srimage_from_csli` 均不覆盖这两个字段。因此 Rust 的每个独立顶层/引用副本 `RuntimeImageState` 都保存同样的两个构造器初值，而不是从 SRD 属性臆造来源。

## 图像预设选择

`srd_select_image_render_preset` 首先读取 `SrImage+0xA0`。非负覆盖值原样作为 32 位有符号整数传给 Ceylon setter；选择阶段不能提前缩窄为 `u8`。

覆盖值为负时，`SrImage+0x04` flags 的低四位按下表选择：

| `flags & 0x0F` | 预设 |
|---:|---:|
| `0` | 通常为 3；见下方特殊分支 |
| `1` | 4 |
| `2` | 5 |
| `3` | 9 |
| `4..15` | 不调用 setter，保持 draw packet 当前状态 |

低四位为零时，`srd_image_special_blend_mode` 对 `flags & 0x600` 返回：`0x200 -> 1`、`0x400 -> 2`、`0x600 -> 3`、其余 `-> 0`。只有 renderer `+0x25D` 非零时，模式 1/2 才分别选择预设 20/21；模式 0/3 或 renderer 特殊模式关闭时仍选择预设 3。

预设 20/21 分支还会把全局 `dword_1CA0B70+0x1C` 进行原生 32 位 `inc`，并把结果写到 `SrImage+0x1C`。Rust 使用 `wrapping_add(1)` 保存溢出行为。落入预设 3 的低四位零分支把 `SrImage+0x1C` 重置为 `-1`；覆盖分支和低四位 1/2/3 分支不修改它。五个实际调用者在调用后都直接继续，未读取 EAX，所以实现只暴露已证明的状态变化。

## draw packet 的有符号请求编码

`ceylon_set_draw_render_preset_id` 保留完整 `i32` 请求参与比较，但写入 packet 时只编码低六位：

```text
packet.flags_00[5:0] = ((u8)requested_id) & 0x3F
```

随后按精确顺序执行：

1. `requested_id > 32` 时设置 `flags_60 & 0x800`，否则清除；
2. 调用 derived-state refresh；
3. `requested_id > 32` 时再次强制设置 `flags_60 & 0x20`。

derived-state refresh 使用 packet 低六位作为无符号 ID，62/63 被钳制到表项 61。它从预设记录更新：

- `flags_60 bit 0x20 = alpha_blend_enabled`；
- `flags_60 bit 0x40 = alpha_test_enabled`；
- `flags_58 bit 0x08 = !alpha_blend_enabled`。

因此例如请求 `300` 不会失败：低六位编码为 44，先使用表项 44 刷新派生位，再因原始请求大于 32 强制设置 `flags_60 bit 0x20`。Rust 单元测试同时覆盖 `-1`、21、62、64 和 300，防止把此行为错误简化成 `u8` 转换或普通范围检查。

独立的 `ceylon_get_render_preset_record` 接收有符号 ID 时采用另一条规则：负数钳制到 0，大于 61 钳制到 61。Rust 将直接表访问和 packet 编码分开实现，避免混淆两种边界行为。

## 32 字节预设记录

`ceylon_render_preset_table` (`0x18A6908`) 含 62 个连续的 32 字节记录。`ceylon_apply_draw_packet_state` 的字段搬运与 `d3d9_flush_blend_and_alpha_state` 的最终 `SetRenderState` 调用共同证明：

| 记录偏移 | 类型 | D3D9 状态 |
|---:|---|---|
| `+0x00` | u8 | `ALPHABLENDENABLE` (`27`) |
| `+0x04` | u32 internal | `SRCBLEND` (`19`) |
| `+0x08` | u32 internal | `DESTBLEND` (`20`) |
| `+0x0C` | u32 internal | `BLENDOP` (`171`) |
| `+0x10` | u8 | `ALPHATESTENABLE` (`15`) |
| `+0x11` | u8 | `SEPARATEALPHABLENDENABLE` (`206`) |
| `+0x14` | u32 internal | `SRCBLENDALPHA` (`207`) |
| `+0x18` | u32 internal | `DESTBLENDALPHA` (`208`) |
| `+0x1C` | u32 internal | `BLENDOPALPHA` (`209`) |

完整 factor 映射来自 `d3d9_map_blend_factor`：

```text
0 ZERO(1)                 1 ONE(2)
2 SRCCOLOR(3)             3 INVSRCCOLOR(4)
4 SRCALPHA(5)             5 INVSRCALPHA(6)
6 DESTCOLOR(9)            7 INVDESTCOLOR(10)
8 DESTALPHA(7)            9 INVDESTALPHA(8)
10 SRCALPHASAT(11)        11 BLENDFACTOR(14)
12 INVBLENDFACTOR(15)
```

完整 operation 映射来自 `d3d9_map_blend_operation`：`0 ADD(1)`、`1 SUBTRACT(2)`、`2 REVSUBTRACT(3)`、`3 MIN(4)`、`4 MAX(5)`。

Rust 保存全部 62 个表项，而不仅是样本当前使用的几个 ID。表内相同记录仍保留为不同索引：0/20/22..60 相同，1/21 相同，3/61 相同，5/15、6/16、7/17、9/19 分别相同。12、13、14 的 separate-alpha 字段也完整保存，未因 SRD 默认路径暂时不用而丢弃。

SRD 直接选择的六个预设最终状态为：

| ID | Alpha blend | Src | Dst | Op | Alpha test |
|---:|---:|---|---|---|---:|
| 3 | 1 | SRCALPHA | INVSRCALPHA | ADD | 0 |
| 4 | 1 | SRCALPHA | ONE | ADD | 0 |
| 5 | 1 | SRCALPHA | ONE | REVSUBTRACT | 0 |
| 9 | 1 | ZERO | SRCCOLOR | ADD | 1 |
| 20 | 0 | SRCALPHA | INVSRCALPHA | ADD | 0 |
| 21 | 0 | SRCALPHA | INVSRCALPHA | ADD | 1 |

## 本地语料交叉验证

53 个本地 SRD 中共解析：

- CIMG：13,773
- CSLI：799
- CNUM：552

这 15,124 个图像型 CAST 的构造器 override/序列初值全部为 `-1`。flags 选择分布为：预设 3 共 10,659、预设 4 共 4,242、预设 5 共 43、预设 9 共 180；没有低四位 4..15 的保持状态样本，也没有触发 20/21 的样本。后两条运行时分支仍由二进制控制流、常量和字段写入完整证明，不能因当前语料未覆盖而删除。

## 当前证据边界

本页闭环的是预设选择、packet ID 编码、62 项 blend/alpha-enable 表以及最终 D3D9 blend render states。`ALPHAREF`、`ALPHAFUNC`、深度、scissor、shader 和双 UV 的像素阶段消费仍需各自调用链证明；这里没有把它们并入混合预设或赋予推测值。
