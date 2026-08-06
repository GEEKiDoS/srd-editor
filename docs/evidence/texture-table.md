# TEXL/TEX/CROP 纹理矩形表证据

本页只记录已经由 SRD 解析器、`SrProject` 分配器、CREF 消费端和外部 DDS 路径构造共同闭环的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`B4C6931303CB6DE0202DFAF5D9E625D6F0C2DA30313AF180B8E7EB3915920756`

## TEXL 表

`srd_parse_texl` (`0xAA3E50`) 读取 TEXL 属性 `0x60`。`j_vtbf_read_unsigned_scalar` 的结果先经 `AX` 截断，再零扩展保存到上下文 `+0x44`，因此运行时声明数量是 `u16`。

解析器随后通过 `SrProject` 虚表槽 `+4` 分配：

```text
declared_count * 0x21C
```

该虚表槽已闭环到会清零整个分配区的 `srd_project_allocate_zeroed`，所以缺少 TEX 子块时，对应记录保持全零。表基址保存于上下文 `+0x48`。解析器只在子块标签恰为 `TEX ` 时调用 `srd_parse_tex` (`0xAA3A70`)，并将目标指针增加 `0x21C`；其他子块不会占用记录。

## 540 字节 TEX 记录

`srd_parse_tex` 对已证明字段的写入为：

| 记录偏移 | VTBF 属性 | 运行时表示 |
| --- | --- | --- |
| `+0x204` | `0x61` | 分配并复制后的文件名字节指针 |
| `+0x208` | `0x40` | 截断为 `u16` 的纹理宽度 |
| `+0x20A` | `0x41` | 截断为 `u16` 的纹理高度 |
| `+0x20C` | `0x62` | `u32`；`0x00F0` 控制 U 寻址，`0x0F00` 控制 V 寻址 |
| `+0x210` | `0x63` | `u32` CROP 记录数量 |
| `+0x214` | — | `crop_count * 0x10` 分配区的指针 |

属性 `0x61` 先由 `vtbf_copy_prefixed_bytes` 类读取逻辑复制到 256 字节、保证 NUL 结尾的栈缓冲区，再按实际 `strlen + 1` 分配和复制。因此游戏实际保留的文件名内容最多 255 字节；Rust 解析器执行相同截断，但以不含尾部 NUL 的字节向量保存语义内容。

`0x62` 的寻址和采样提交链已经闭环：`0x00F0` 中任一位非零时 U 使用 Clamp，否则 Wrap；`0x0F00` 中任一位非零时 V 使用 Clamp，否则 Wrap。每条 TEX 同时建立 Linear 与 Point 两个采样包装对象，具体证据见 [`texture-binding.md`](texture-binding.md)。Rust 仍保留原始 `field_62`，避免丢失尚未证明的其他位。

## CROP 归一化矩形

`srd_parse_crop` (`0xAA3CF0`) 对每个属性 `0x65` 建立一个 16 字节、四个 `f32` 的记录。局部四值缓冲区在读取前清零；读取循环使用属性描述符给出的标量数。设原始值为 `raw[0..4]`，TEX 记录尺寸为 `width/height`，写入结果为：

```text
rectangle[0] = (1.0f / float(width))  * raw[0]
rectangle[1] = (1.0f / float(height)) * raw[1]
rectangle[2] = (1.0f / float(width))  * raw[2]
rectangle[3] = (1.0f / float(height)) * raw[3]
```

二进制中 X 分量的写入顺序是 `2, 0`，Y 分量是 `3, 1`，但最终 16 字节布局确定为 `[x0, y0, x1, y1]`。每处理一个 `0x65`，目标指针增加 `0x10`。

这正是 `srd_resolve_cref_texture_coordinates` (`0x129B8D0`) 使用的表：它以 CREF 的 signed `image_index` 选择 `0x21C` 字节 TEX 记录，再从该记录 `+0x214` 取矩形表并以 signed `rectangle_index * 0x10` 选择矩形。因此 TEXL/CROP 到 SrSliceCast 最终 UV 的数据路径已闭环。

## 外部 DDS 路径

`sub_AA55C0` 在解析 SRFF 后取得同一 TEXL 上下文。`0xAA568C` 从每条 `0x21C` 字节记录的 `+0x204` 读取文件名，`0xAA56A0` 追加字面量 `.dds`，通过 `sub_419BEB` 检查路径存在，然后在 `0xAA570C` 把路径与对象 `+0xE0` 传给该对象虚表槽 `+0x4C`。循环在 `0xAA572B` 使用相同的 `0x21C` 步长。

这证明 TEX `0x61` 是不含 `.dds` 后缀的外部纹理路径基础名，也证明运行时按 TEXL 声明顺序逐项提交存在的 DDS。加载入口已经闭环到 `air::TextureResource`，DDS 描述符、D3D9 原生创建和 D3DX9_43 回退参数详见 [`dds-resource-loading.md`](dds-resource-loading.md)；已加载纹理进入包装对象和绘制绑定的后续路径见 [`texture-binding.md`](texture-binding.md)。

Chusan 的独立路径 provider `0x7D6360/0x7D61A0` 已证明专用根为 `surfboard/texture/`。编辑器给定完整 data 根时使用 `<data>/surfboard/texture/<base>.dds`；这是一条由 provider 与本地文件语料共同验证的编辑器映射，不把不同游戏资源对象内部的绝对路径管理方式混为同一个字符串拼接调用。首个实际 stage-0 draw 使用该映射加载 `CHU_UI_Movie_dummy.dds`，见 [`render-first-textured-draw.md`](render-first-textured-draw.md)。

## 样本回归

53 个本地 SRD 中共解析出：

- 1170 条 TEX 记录；
- 24167 条 CROP 矩形；
- 4994 个 image/rectangle 下标均非负的 active SLIC CREF 选择。

4994 个非负选择全部能在同文件 TEXL/CROP 表中解析到实际矩形。Rust 的 `TextureList` 保留声明数量造成的零记录、文件名 255 字节截断、尺寸、原始 `0x62`、声明 CROP 数量及归一化矩形，并把结果连接到 CSLI/SLIC 的最终 UV 顺序和已证明的双采样状态。

## 仍未闭环

- 无需格式转换的二维 DDS staging/`UpdateSurface`、独立 BC1/BC3 fallback 解码和编辑器 DEFAULT-pool `ResetEx` 重建已接入；完整语料未出现的内部格式转换与 cube request 仍待闭环。
- CIMG/CRE1 与 CNUM 每个 mode-0 glyph 的 CREF/CRE1 已闭环到 TEXL/CROP 和统一 D3D9Ex runtime submission；剩余的是 TEXT 旧式字体路径、未出现在 shipped corpus 的 CNUM history mode `1..8`，以及双 UV shader 公式的独立命名。
- CIMG/CNUM 动画通道 `17/20` 的显式矩形也已闭环到相同 TEXL/CROP 表。
- 单贴图 TEXCOORD0 与 Advertise 双纹理 TEXCOORD1 的 shader 消费均已实际提交；其余边界是显式 texture override 与未覆盖的外部 draw context。
