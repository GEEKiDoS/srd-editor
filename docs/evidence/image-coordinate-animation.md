# CIMG/CNUM 双坐标描述符动画证据

本页记录 CAST 专属动画通道到 SrImage 两份 48 字节坐标描述符的已闭环路径。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`CE6CC7A75EDE1FA4EE7830736486B7BF71C64C03A8366344BE9A5C194046C6C5`。

## CAST 专属通道分派

`srd_apply_animation_motion_set` (`0xAD52B0`) 在公共通道之后调用 `srd_apply_cast_animation_channels` (`0xAD5370`)。后者已经证明下列直接目标：

| 通道 | 运行时目标 |
| --- | --- |
| `11/12` | SrImage 当前宽/高，求值后重新计算 size/origin |
| `13/14/15/16` | 四个 packed vertex color，目标顺序为记录偏移 `+0/+8/+4/+12` |
| `17` | 坐标描述符通道 0，即 CREF |
| `20` | 坐标描述符通道 1，即 CRE1 |
| `23` | CAST 虚表槽 `+0x7C`；具体语义依派生 CAST 而异 |

Rust 本轮只实现已经继续闭环到 key、引用表和 TEXL/CROP 的 `17/20`。尺寸、颜色和通道 `23` 保留在后续任务中，不因定位到目标偏移就提前赋予完整编辑语义。

## 标量轨道

`srd_apply_image_coordinate_track` (`0xAD3620`) 检查 TRK format 的最低两位。结果不等于 `3` 时，它调用公共标量求值器，并把四个结果字节原样写到所选描述符：

```text
descriptor +0x18: signed i16 reference selector
descriptor +0x1A: signed i16 explicit image index
```

这与游戏的原始位写入规则一致，不执行数值类型转换。该路径不修改描述符 `+0x1C` 的显式矩形标志。

## 20 字节引用 key 轨道

format 最低两位等于 `3` 时，`srd_eval_image_reference_track` (`0xAD36A0`) 使用该通道的引用表指针和声明数量：CAST `+0x1A4/+0x1AC` 对应 CREF，`+0x1A8/+0x1B0` 对应 CRE1。表为空或 key 数量为零时返回 false，并清除显式矩形标志。

`srd_eval_image_reference_keys` (`0x129AC50`) 使用 20 字节 key：

```text
+0x00 i32 frame
+0x04 i32 reference selector（最终写入描述符时截断为 i16）
+0x08 u32 mode
+0x0C/+0x10 本路径不消费
```

时间首先经过已有的 TRK wrap 规则。单 key、时间位于端点外，或左 key mode 为 `0` 时，直接采用对应 key 的 selector。区间内且左 mode 非零时：

```text
t        = (frame - left.frame) / (right.frame - left.frame)
selector = cvtt_i32(left.selector * (1-t) + right.selector * t)
rectangle[i] = left.rectangle[i] * (1-t) + right.rectangle[i] * t
```

selector 最终写入描述符 `+0x18`。`srd_lookup_cref_rectangle` (`0x129B2C0`) 用 key selector 查询当前通道的 CREF/CRE1 记录，再经 TEX 记录步长 `0x21C` 和 CROP 步长 `0x10` 取得四个归一化 f32。有效查询还把记录的 image index 写入显式 image index。

区间插值会依次查询左、右 key，并共用同一个 image-index 输出。因此右 key 有效时，最终显式 image index 来自右 key；只有两端矩形都有效时才覆盖四个插值矩形。如果查询无效，游戏保留描述符中此前的矩形字节。Rust 复现这项有状态行为，但对负 selector 不执行游戏中未定义的数组前寻址。

求值函数返回 true 后，`srd_apply_image_coordinate_track` 把描述符 `+0x1C` 设为 true。后续 `srd_resolve_cref_texture_coordinates` 仍先检查插值后的 selector 是否落在声明数量内，然后才使用显式 image/rectangle；动画不能绕过 selector 边界。

## Rust 对应与样本验证

`ImageDefinition::apply_coordinate_track` 同时支持标量原始位写入和 20 字节引用 key。它直接修改 `ImageCoordinateState`，保留 wrap、端点、mode 0 hold、`cvtt` selector、左右查询顺序、显式 image 选择、矩形 f32 插值及无效查询时的旧矩形。

53 个本地 SRD 中，通道 `17` 只出现 format `0x23/0x123`，通道 `20` 同样只出现 `0x23/0x123`。对已连接到 CIMG/CNUM 的轨道，在每个 key 和相邻 key 中点共执行 183432 次求值，其中 183016 次得到能够继续通过 `resolve_coordinates` 的显式纹理引用。差额来自游戏允许返回显式状态、但 selector 或 image index 在该时刻不可绘制的情况。

## 仍未闭环

- 通道 `11/12` 的尺寸动画写回 Rust 运行时状态；
- 通道 `13..16` 的 packed color key 插值和四顶点绑定；
- 通道 `23` 在各派生 CAST 中的虚函数语义；
- 动画后的双 UV 在 shader/固定管线中的最终组合与 D3D9 draw 状态。
