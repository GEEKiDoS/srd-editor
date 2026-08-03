# CNUM 与 SrNumberCast 证据

本页只记录已经由 CNUM 解析器、运行时对象初始化、数字格式化和 glyph 记录生成代码共同闭环的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`C90A6D76104FB1462F3DF8049D269BEF03EF3EAEAE506EA82693CCB3D1A9D80C`。

## CNUM 解析布局

`srd_parse_cnum` (`0xAA2440`) 分配并清零 `0x94` 字节，随后把宽高设为 `128.0`，四个 packed color 设为 `0xFFFFFFFF`，并把解析结构 `+0x18/+0x1C/+0x20/+0x24` 设为 `-1`。已确认字段如下：

| 属性 | 解析结构偏移 | 已证明的表示或用途 |
| --- | --- | --- |
| `0x49` | `+0x00` u32 | SrImage flags |
| `0x40/0x41` | `+0x04/+0x08` f32 | 宽、高 |
| `0x42/0x43` | `+0x0C/+0x10` f32 | 自定义 origin |
| `0x4B` | `+0x14` u8 | 3x3 origin mode |
| 重复 `0x44` | `+0x18..+0x27` | 四个重排后的 packed vertex color |
| `0x45` | `+0x28` u16 | CREF 声明数量 |
| `CREF` | `+0x2C` pointer | CREF 记录数组 |
| `0x4C` | `+0x30` u16 | 复制到 SrImage 的保留字段 |
| `0x4D` | `+0x32` u16 | CRE1 声明数量 |
| `CRE1` | `+0x34` pointer | CRE1 记录数组 |
| `0x80` | `+0x38` u32 | 数字格式 flags |
| `0x78` | `+0x3C` u32 | 语义尚未闭环，保留原值 |
| `0x81` | `+0x40` i32 | 初始整数部分 |
| `0x82` | `+0x44` f32 | 初始小数部分 |
| `0x83..0x8B` | `+0x68..+0x78` | 九个 signed i16 |
| `0x8C` | `+0x7C/+0x80` | 两个 f32 |
| `0x8D..0x94` | `+0x84..+0x92` | 八个 signed i16 |
| `0x51` | 临时 i32 | NODE 索引；非负时写入 NODE `+0x50` |

`CREF` 与 `CRE1` 都使用和 CIMG 相同的记录格式：每个 `0x4A` 连续读取两个 signed i16，分别是 TEXL 图像下标和该 TEX 的矩形下标。CNUM 对 `TEXT` 只比较标签，不调用 CIMG 的 TEXT 解析器。

## SrNumberCast 建立与初始值

NODE type `4` 由 `srd_create_number_cast` (`0xAC9D70`) 建立 `surfride::SrNumberCast`。`srd_number_cast_construct` (`0xADC2C0`) 构造 `0x270` 字节对象，随后 `srd_init_number_cast_from_cnum` (`0xAE0AF0`) 调用 `srd_init_srimage_from_cnum` (`0xAD2A10`) 初始化内嵌 SrImage。

CREF/CRE1 指针和数量被同时复制到 SrImage，与 CIMG 一样是两个独立表。两份坐标描述符的选择器都被强制设为 `0`，所以初始引用均为各自表的第零项，而不是读取 CIMG 的 `0x46/0x4E` 属性。

初始化函数把 CNUM `0x81` 的 signed i32 和 `0x82` 的 f32 传给 `srd_number_cast_set_value_parts` (`0xAE09B0`)。该函数分别保存两部分，并以 double 精度计算二者之和。CNUM 属性 `0x90` 只有在 signed 值位于 `[5, 9)` 时才允许调用方传入的插值标志生效；其余值会把该标志强制清零。

底层通用图像坐标代码会把解析结构 `+0x40/+0x44` 当作两个 UV 通道的 U 偏移读取；其中 `+0x40` 按 f32 bit pattern 重解释。但是 NumberCast 构造和初始化路径没有给 CAST `+0x210` 写入非零倍率，其构造后初值为 `0.0`。因此初始坐标偏移乘积为零。Rust 保留这条原始读取和零倍率，不把两个数值字段另行解释成真正的 UV 动画参数。

## 字符到 CREF 的映射

`srd_number_build_glyph_records` (`0xAE00F0`) 使用 `character & 0x3F` 查表。数字字符的静态映射为：

```text
'0' -> 0, '1' -> 1, ... '9' -> 9
```

函数每次调用都会从 CNUM 写入四个特殊字符映射：

| 字符 | 属性 | 解析结构偏移 |
| --- | --- | --- |
| `+` | `0x91` | `+0x8C` |
| `-` | `0x92` | `+0x8E` |
| `,` | `0x93` | `+0x90` |
| `.` | `0x94` | `+0x92` |

映射结果是 CREF 下标。只有当该 signed 下标小于 CREF 声明数量时，函数才追加一个 56 字节 glyph 记录；记录 `+0x30` 保存该下标，`+0x34` 的字节把 `+`、`-`、`,`、`.` 四类特殊符号标为 `0`，普通数字标为 `1`。样本中确实存在超出 CREF 数量的特殊字符下标，游戏会跳过对应 glyph；Rust 不把这种情况擅自判为格式错误。

## 小数格式化与已知宽度字段

`srd_number_format_fractional_digits` (`0xAE0280`) 使用属性 `0x89` 作为小数位数，计算 `10^digits`，并把小数绝对值限制到 `10^digits - 1`。格式 flags `0x4` 选择按位数补零的整数格式；未设置时使用普通十进制整数格式。flags `0x2` 会进一步调用字符串填充辅助函数，并把属性 `0x88` 的 signed 值作为该辅助函数的参数。Rust 暂时只公开原始 `0x88` 值，不把辅助函数尚未完全闭环的行为扩展成编辑器语义。

`srd_number_measure_string_width` (`0xADE140`) 还证明：普通数字基础宽度来自 `0x83`，逗号或小数点宽度来自 `0x85`；小数点后的普通数字宽度乘以 `0x8C[0]`。间距路径会使用 `0x8A` 和 `0x8D`，并受 format flags `0x20` 影响。由于完整定位、对齐、缩放和 glyph quad 生成链尚未闭环，这些字段目前仍以原始数组保存，未暴露成可编辑的最终布局模型。

## Rust 对应与样本验证

`NumberDefinition` 复现上述默认值、属性布局、CREF/CRE1 表、初始数值、已证明的格式字段和特殊 glyph 映射。`Layer::from_block` 根据 CNUM `0x51` 把定义连接到 NODE，并验证对应 NODE 的 cast type 为 `4`。复用 `ImageDefinition` 的部分只包含二进制证明由 SrImage 共用的尺寸、origin、颜色、纹理表和初始坐标状态。

本地 53 个 SRD 的回归测试解析并链接了 552 个 CNUM、5852 条 CREF 和 12 条 CRE1；其中 555 个具有非负实际引用的初始通道能够解析到 TEXL/CROP。测试也单独统计落在声明 CREF 范围内的特殊 glyph，而不拒绝游戏会跳过的越界映射。

## 仍未闭环

- `srd_number_rebuild_glyph_geometry` (`0xADD6C0`) 的完整字符串分段、对齐和每个 glyph 的位置公式；
- `0x83..0x90` 其余字段的完整语义，以及 format flags 除已列位之外的含义；
- NumberCast 的动画通道到数值、格式和布局字段的全部绑定；
- `srd_render_number_cast` (`0xADE6C0`) 下游纹理槽、混合、深度/裁剪状态和最终 D3D9 draw call。
