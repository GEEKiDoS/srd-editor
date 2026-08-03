# CIMG、CREF/CRE1 与 SrImageCast 证据

本页只记录已经由解析器、运行时对象初始化和最终 `SrImageCast` 绘制入口共同闭环的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`BF6B8219DAE9D352483F47D49D9472B466B42E4A3EDF5F346937E4D8DBAD9D9C`。

## CIMG 解析布局

`srd_parse_cimg` (`0xAA1410`) 分配并清零 84 字节，随后设置宽高为 `128.0`、四个 packed color 为 `0xFFFFFFFF`，并把两个引用选择器设为 `-1`。已确认的字段为：

| 属性 | 解析结构偏移 | 运行时用途 |
| --- | --- | --- |
| `0x49` | `+0x00` u32 | SrImage flags；完整值参与 UV flip/order，最高字节 bit 0 选择 Point 采样 |
| `0x40/0x41` | `+0x04/+0x08` f32 | 宽、高 |
| `0x42/0x43` | `+0x0C/+0x10` f32 | origin mode 超出 0..8 时的自定义 origin |
| `0x4B` | `+0x14` u8 | 3x3 origin mode |
| 重复 `0x44` | `+0x18..+0x27` | 四个重排后的 packed vertex color |
| `0x46` | `+0x28` i16 | CREF 选择器 |
| `0x45` | `+0x2A` u16 | CREF 声明数量 |
| `0x4C` | `+0x34` u16 | 保留字段，复制到 SrImage `+0x0C` |
| `0x4E` | `+0x36` i16 | CRE1 选择器 |
| `0x4D` | `+0x38` u16 | CRE1 声明数量 |
| `0x83/0x85` | `+0x40/+0x48` f32 | CREF 通道 U/V 后加偏移 |
| `0x84/0x86` | `+0x44/+0x4C` f32 | CRE1 通道 U/V 后加偏移 |
| `0xA1` | `+0x50` u32 | 保留字段 |
| `0x51` | 临时 i32 | NODE 索引；非负时写入 NODE `+0x50` |

`CREF` 与 `CRE1` 子块都由 `srd_parse_cref_like` (`0xAA1390`) 解析。每个 `0x4A` 连续读取两个 signed i16，分别是 TEXL 图像下标和该 TEX 的矩形下标。两种子块使用相同记录格式，但写入完全不同的指针与数量字段。

## 两张表在运行时同时保留

`srd_init_srimage_from_cimg` (`0xAD28E0`) 把解析结果复制到 CAST `+0xF8` 的内嵌 `surfride::SrImage`：

| 数据 | SrImage 偏移 | CAST 总偏移 |
| --- | --- | --- |
| CREF 指针 | `+0xAC` | `+0x1A4` |
| CRE1 指针 | `+0xB0` | `+0x1A8` |
| CREF 数量 | `+0xB4` | `+0x1AC` |
| CRE1 数量 | `+0xB8` | `+0x1B0` |

因此二者不是加载时的优先级或替代表。`srd_build_image_texture_coordinates` (`0xAD7EE0`) 对参数执行 `selector & 1`：结果 `0` 取 `+0x1A4/+0x1AC`，结果 `1` 取 `+0x1A8/+0x1B0`。`srd_render_image_cast` (`0xAD77D0`) 明确调用两次，第一次传 `0` 生成 CREF UV 通道，第二次传 `1` 生成 CRE1 UV 通道。

## 坐标描述符与解析边界

`srd_srimage_construct` (`0xAD2150`) 清零两个 48 字节坐标描述符。CIMG 初始化先建立第一份描述符，再整块复制成第二份，只分别写入 CIMG `0x46` 和 `0x4E` 选择器。描述符中与坐标解析直接相关的字段为：

| 描述符偏移 | 语义 |
| --- | --- |
| `+0x18` i16 | CREF/CRE1 记录选择器 |
| `+0x1A` i16 | 显式矩形模式的图像下标 |
| `+0x1C` u8 | 非零时使用显式矩形 |
| `+0x20..+0x2C` | 四个 f32 矩形端点 |

`srd_resolve_cref_texture_coordinates` (`0x129B8D0`) 的第一项检查使用 unsigned compare。由于选择器先 sign-extend，负数与大于等于声明数量的值都会失败；输出保持图像下标 `-1`、矩形全零。显式矩形模式也位于这项边界检查之后，所以不能绕过无效选择器。

普通表模式先把记录的第一个 i16 写入输出图像下标；仅当图像下标和矩形下标都非负时，才按 TEX 运行时记录步长 `0x21C` 和矩形记录步长 `0x10` 读取四个 f32。之后 CIMG flags `0x10/0x20` 交换 X/Y 端点，`flags & 0xC0` 使用与 SrSliceCast 相同的四种顶点顺序。

## CIMG 坐标偏移与采样模式

底层解析完成后，`srd_build_image_texture_coordinates` 对四个顶点逐一执行：

```text
channel 0: u = CIMG.0x83 * CAST[+0x210] + u
           v = CIMG.0x85 * CAST[+0x210] + v

channel 1: u = CIMG.0x84 * CAST[+0x210] + u
           v = CIMG.0x86 * CAST[+0x210] + v
```

`srd_init_image_cast_from_cimg` (`0xAD7FB0`) 把 `CAST +0x210` 的初值写为 f32 bit pattern `0x4CBEBC20`。Rust 以 `f32::from_bits` 保留该精确初值和乘加顺序，不为该倍率附加未经证明的单位解释。

`srd_init_srimage_from_cimg` 从 CIMG flags 的第四个字节取 bit 0 写入 CAST `+0x100`。结合已闭环的纹理 pair 选择，这等价于 `flags & 0x01000000 != 0` 时选择 Point 包装对象，否则选择 Linear 包装对象。

`srd_select_render_texture_pair` (`0xAC6E90`) 先把两个显式覆盖分别写入渲染器 `+0x140/+0x144`。覆盖为空时，它分别对 CREF 和 CRE1 输出的 signed 图像下标执行 unsigned 范围检查，并从对应 TEX pair 选择 Linear 或 Point 包装对象写回这两个槽。两槽共享同一个 Point/Linear 标志，但彼此的图像下标和覆盖指针独立。当前证据足以称为 CREF/CRE1 渲染纹理槽；在闭合下游绘制包前，不把第二槽直接命名为某个 D3D9 stage。

## SrImageCast 顶点颜色

`srd_render_image_cast` 对四个 36 字节顶点逐一读取第一份 48 字节描述符的四个 packed color。它调用 `srd_unpack_packed_color` 后，以 `srd_multiply_color_u8` 和 CAST multiplicative tint 相乘，写入顶点 `+0x0C`；CAST additive tint 不经过额外运算，直接写入顶点 `+0x10`：

```text
primary   = descriptor.vertex_color[vertex] * CAST multiplicative tint
secondary = CAST additive tint
```

乘法仍是逐通道 `u32(a) * u32(b) / 255`。Rust 的 `ImageDefinition::vertex_colors` 要求调用方显式提供两种 CAST tint，不为尚未追完的动画/default 来源填入假定值。

## SrImageCast 与 SrTextCast 的建立条件

`srd_create_runtime_cast_for_node` (`0xACA030`) 对 NODE type `1` 检查其 CIMG：仅当存在 `TEXT` 子块且 CIMG flags 包含 `0x100` 时使用注册表 key `2`，其工厂 RTTI 为 `surfride::SrTextCast`；否则使用 key `1`，工厂 RTTI 为 `surfride::SrImageCast`。Rust 当前保存 `has_text_child` 并通过 `creates_text_cast` 复现这个已证明的分类条件；TEXT 内部字段仍等待独立闭环。

## Rust 对应与样本验证

`ImageDefinition` 复现上述 CIMG 默认值、属性布局、两张引用表、origin、采样标志和 cast 分类。`ImageCoordinateState` 对应 48 字节描述符中的已证明字段；`resolve_coordinates` 保留选择器边界、显式矩形、负下标、flip/order、偏移乘加顺序和每通道最终采样状态。`Layer::from_block` 依据 CIMG `0x51` 把定义连接到 NODE。

本地 53 个 SRD 的回归测试会验证所有已连接 CIMG 的 NODE type、两张表的声明长度，并对具有非负实际引用的初始 CREF/CRE1 通道解析 TEX/CROP。精确计数由测试运行输出记录，避免把样本统计手工固化进格式结论。

## 仍未闭环

- TEXT 子块的完整字段、字体资源、排版和 glyph 绘制；
- 动画轨道如何逐字段修改两个 48 字节坐标描述符；
- 两个 UV 通道进入 shader/固定管线后的精确组合；
- CAST tint 的动画/default 来源。
