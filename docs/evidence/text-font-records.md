# TEXT、FONT/CHAR 与外部 RFZ 字体资源

状态：SRD 内 TEXT、FONT、CHAR 的记录布局，TEXT 到项目 FONT 下标解析，SrTextCast 建立/初始化，外部 RFZ/YABX/Ruhuna/AVTS/DDS 字体资源，Ruhuna Database/Glyph 到游戏 128 字节 runtime glyph 的转换，RFZ `TextBox` 路径与旧式 FONT/TEX/CROP 路径的运行时分流，实际游戏使用的 UTF-8 输入模式和当前完整语料所需的 Fennel token 子集，以及 format 13 字形 batch 的顶点声明、shader、DrawPacket、atlas sampler 和 D3D9 提交参数均已闭环。静态 mode-zero `sub_7C1F90` 的 auto-fit、自动断行、固定字符表、空格候选、对齐和垂直 `-254` 截止，`sub_7C0D40` 的记录过滤、atlas 分组与初始 hash 前向链顺序，以及 `sub_7C7F90` normal glyph 的 origin/effective-scale/2D CPU matrix 链也已实现，并通过 1292/1292 条完整 RFZ TEXT 审计。SrTextCast 初始 world/color、零颜色门控、ShapeEnv material cull 和真实 D3D9Ex Composition 像素回归现已闭环。当前剩余主线是内部行元数据与效果/裁剪分支。

## TEXT 记录

`srd_parse_cimg` (`0xAA1410`) 遇到直接 `TEXT` 子块时分配 `0x24` 字节并调用 `srd_parse_text` (`0xAA18F0`)。后者的直接写入为：

| 属性 | TEXT 偏移 | 读取行为 |
| --- | --- | --- |
| `0x78` | `+0x00`, u32 | unsigned scalar |
| `0x79` | `+0x04`, i32 | FONT 下标 |
| `0x7A` | `+0x08`, `char*` | 分配 2048 字节，最多复制 2047 字节 |
| `0x36` | `+0x0C/+0x10`, f32[2] | `sub_1298730(..., count=2)` |
| `0x7B` | `+0x14..+0x1A`, i16[4] | 四个 scalar 读为 i32 后截断写入 |
| `0x7C` | `+0x1C`, i16 | signed scalar 截断 |
| `0x41` | `+0x1E`, i16 | signed scalar 截断 |
| — | `+0x20` | 已解析 FONT 指针 |

解析器先把 `+0x08` 和 `+0x20` 清零。若 `0x79 >= 0` 且小于 PROJ 的 FONT 数，就按 `FONT_record_size=0x54` 把对应记录地址写入 `+0x20`。因此 TEXT 的字体关系是当前同一 PROJ 内的下标，不是由字符串名在运行时任意搜索。

Rust 的 `TextDefinition` 保留已证明字段；二进制没有显式初始化且样本可能省略的标量保存为 `Option`，没有虚构默认值。

## PROJ FONT 表

`srd_parse_srck` (`0xA9F370`) 对 PROJ：

- 属性 `0x05` 截断为 u16 写入 PROJ `+0x48`，表示直接 FONT 数；
- 按 `count * 0x54` 分配 FONT 数组并写入 PROJ `+0x54`；
- 直接 `FONT` 子块按顺序交给 `srd_parse_font` (`0xAA1110`)。

FONT 记录：

| 属性 | FONT 偏移 | 行为 |
| --- | --- | --- |
| `0x03` | `+0x00`, `char[0x40]` | 字体资源名 |
| `0x70` | `+0x44`, u32 | flags；低三位任一非零时 lookup 容量为 65536，否则为 256 |
| `0x71` | `+0x48`, u16 | unsigned scalar 截断 |
| — | `+0x50` | lookup 表，每项两个 i16，初始均为 `-1` |

`srd_parse_char` (`0xAA12D0`) 对每个直接 CHAR：

- `0x72` 读取 lookup 下标；
- `0x4A` 连续读取两个 signed scalar，截断为 i16 后写入该下标的两个值。

Rust 用稀疏 `FontCharacterMapping` 保存实际 CHAR 写入，并以文件顺序最后一次写入优先查询；不会为每个 FONT 强制分配 65536 项。当前本地 53 个 SRD 的 249 个 FONT 均没有内联 CHAR，说明这批样本实际依赖外部字体资源。

## SrTextCast 初始化

`srd_create_runtime_cast_for_node` (`0xACA030`) 只在 NODE type 1、CIMG 存在 TEXT 且 CIMG flags `0x100` 非零时选择工厂 key 2。工厂分配 `0x350` 字节并调用 `SrTextCast` 构造函数 `0xAD8270`；构造函数先建立 SrImageCast 基础部分，再写主虚表 `0x190E210`。

公共 CAST 初始化器调用虚表 `+0x08`，SrTextCast 在该槽进入 `sub_ADA4B0`：

1. 以 CIMG 初始化内嵌 SrImage；
2. 以 CIMG `+0x30` 的 TEXT 指针初始化 TextCast `+0x1F4` 的播放/文本状态；
3. 读取 TEXT `+0x08` 字符串；
4. 按 SrPlayer 的外部属性分支调用 TextCast 字符串 setter；
5. setter 最终更新 TextCast `+0x1F8` 起始的字符串状态并触发虚表 `+0xBC` 的重建路径。

这证明 TextCast 不是把 CIMG quad 直接换一张纹理：它拥有独立的字符串状态、字体资源和重建过程。

## RFZ 外部资源

AdvertiseLogo 的两个 FONT 名为：

```text
RFO_SEGAKAKUGOTHIC_DB_24pt.rfz
RFO_SEGAKAKUGOTHIC_DB_32pt.rfz
```

本地完整 data 根共有 6 个 RFZ，位于 `A000/font`。全部实文件开头为：

```text
59 53 02 00
Y  S  version=2
```

资源入口 `sub_F478A0` 明确：

1. 要求至少 4 字节且前两字节为 `Y`,`S`；
2. 要求第三字节为 `2`；
3. 建立序列化 reader，并把扩展名 `.rfz` 交给对应 handler；
4. 解析结果必须具有根类型名 `RHFONTDB`，否则释放并返回失败。

初始化函数 `sub_F531F0` 把普通 handler 注册为 `rfo`，把 RTTI 明确为 `yabukita::SerializeHandlerBinaryLZW` 的 handler 注册为 `rfz`。所以 RFZ 是游戏自己的 BinaryLZW 序列化容器；不能按 ZIP、zlib、DDS 或裸字体猜测解析，也不能用 Windows/ImGui 字体替代来声称复现游戏输出。

## 语料回归

本地 53 个 SRD：

```text
FONT records=249
inline CHAR mappings=0
TEXT records=1237
nonempty TEXT strings=1190
invalid nonnegative FONT indices=0
```

AdvertiseLogo 实际解析得到 2 个 FONT、多个英文/日文 TEXT；例如 warning 页的 `WARNING` 与版权说明均解析到 FONT `[1]` 的 32pt RFZ。

当前 `D:\sdhd\assets\data\surfboard` 完整目录的重新审计结果为：

```text
FONT records=365
TEXT records=1292
RFZ texts=1292
显式具有 0x78/0x36/0x7C/0x41 的 RFZ texts=1292
缺失上述静态排版输入的 RFZ texts=0
有效游戏模式 UTF-8=1292
当前已实现 token 子集=1292
```

因此 Rust 的静态 RFZ 输入转换器要求这四项属性显式存在；缺项直接返回错误，不需要也不允许为当前语料发明默认值。

## SrTextCast 排版与 glyph quad 路径

主虚表 `0x190E210` 的 `+0xBC` 进入 `sub_AD8D50`，字符串/样式状态更新后触发重建。实际 draw 入口 `sub_AD9160` 已继续闭环出以下调用链：

- `sub_AD8780` 按字节读取文本：低于 `0x80` 的字节直接成为 u16 code，高位字节与下一字节组成 big-endian u16 code；只有 `CR LF` 组合切分新行。它不是 UTF-8 解码器。
- `sub_AD8480` 对一行逐 code 查询 FONT `+0x50` 的两个 i16 映射，使用第一个值选择 540 字节 TEX 记录、第二个值选择 16 字节 CROP，并以 CROP 宽高和 TEX 像素尺寸累计行宽/行高；TEXT `+0x1C` 的字符间距会乘以 `count - 1` 加入宽度。
- `sub_AD86D0` 对所有 12 字节行记录调用上述测量并累计总高度。
- `sub_AD8620` 使用 TEXT flags `0x10/0x20` 选择垂直居中或底对齐，否则使用 TEXT `+0x18` 的原始 Y 起点。
- `sub_AD9160` 为每个 12 字节行记录调用虚表 `+0xC4`；SrTextCast 在该槽进入 `sub_AD9490`。
- `sub_AD9490` 使用 TEXT flags `0x04/0x08` 选择水平居中或右对齐，逐 code 再查 FONT/TEX/CROP，生成与当前 SrImage 路径相同的四顶点和两个相同 UV 通道。映射不存在时走明确的 16x16 fallback quad，而不是系统字体。

这里的 FONT/TEX/CROP 路径不是 RFZ 的前置转换结果。`sub_AD9160` 在进入上述逐字路径前先调用 `sub_46C4BD -> sub_AC5740`：

1. `sub_AC5740` 从 TextCast 状态读取字体资源名和 FONT 下标；
2. `sub_AC6440` 在 `SrRenderer +0x26C` 的字体资源树中按名字查找，并从该名字对应的 `shared_ptr<font::TextBox>` 数组按 FONT 下标取对象；
3. `sub_7BC820` 通过全局 `font::FontManager` 和 TextBox `+0x80` 的注册下标取得 `font::TextBoxObject`；
4. `sub_AC5740` 把 SrTextCast 的矩阵、颜色、对齐、裁剪和字符串状态写给 TextBoxObject；成功时返回 `1`；
5. `sub_AD9160` 对该返回值取反，只有 TextBox 路径返回 `0` 时才执行 `sub_AD8780/sub_AD8480/sub_AD9490` 的 FONT/TEX/CROP fallback。

`sub_AC6F50` 把 `TEXT.flags & 0x0C` 和 `TEXT.flags & 0x30` 传给 `sub_AC6BF0`，并把返回值写到 `TextBoxObject+0x2D0`。反编译与逐指令结果一致：水平组 `0/0x04/0x08` 分别贡献 `0/1/2`，垂直组 `0/0x10/0x20` 分别选择 `+0/+3/+6`，因此 9 个合法组合映射为 `0..8`。若任一组含同时置位的非法组合，函数回退为 `0`。Rust 的 `fennel_alignment_code_from_text_flags` 精确保留这张映射表和非法组合回退，没有把它简化成对所有位模式都成立的算式。

同一函数先调用 `sub_7C8DA0` 清除 `TextBoxObject+0x2E0` 的 `0x04/0x08`，再调用 `sub_7C8E80` 清除 `0x20/0x80/0x400/0x800/0x1000/0x2000/0x4000`。后续 `0..6` 模式 switch 会通过这两个 setter 重建对应位；已确认模式 `2..4` 会置位 `0x20`。`sub_7C90A0` 正是按 `0x20`、其次 `0x40`、否则默认路径选择三套大布局函数。当前尚未在这条初始化链找到 `0x40` 的写入来源，也尚未证明各模式的排版语义，因此 Rust 只实现已经闭合的 alignment code，不命名或选择这些布局分支。

加载端与此完全对应。`srd_player_impl_load_project` 对每个 PROJ FONT 调用 `sub_AC4A80`。该函数为普通字体名建立并注册 `font::TextBox`，保存到同一个 `SrRenderer +0x26C` 资源树；但字体名包含 `.sbfont` 时明确跳过建立 TextBox。因此 `.sbfont` 必然进入旧式路径，而 RFZ 在 TextBox 可用时直接走 Fennel/Ruhuna；外部资源加载失败也会回退到旧式路径。二进制中不需要、也没有证据支持一个 RFZ runtime glyph 到 FONT/TEX/CROP 的写表桥。

`sub_7CB9B0` 已证明 RFZ Database/Glyph 到 128 字节 runtime glyph 的全部 box、bearing、advance、旋转和带一像素边框 UV 算法，细节见 [`ruhuna-font-archives.md`](ruhuna-font-archives.md)。`sub_F323B0` 再按 font slot 调用具体 FontResource 虚表 `+0x08` 取得该记录，并把 slot ID 写入 glyph `+0x04`。

`font::TextBoxObject::setTextByWideString` (`sub_7C90A0`) 随后把每个正常 glyph 转换为 116 字节布局记录，已证明的直接写入为：

| 布局偏移 | 来源 |
| ---: | --- |
| `+0x00` | 正常 glyph 为 `0`；runtime width 为零时为 `-2` |
| `+0x04` | runtime glyph 指针/不透明 token |
| `+0x08` | runtime glyph `+0x18` 的 atlas texture handle |
| `+0x0C` | token iterator 输出字段 |
| `+0x10/+0x14` | token iterator 的整数位置转 f32 |
| `+0x18/+0x1C` | 正常 glyph 为 runtime width/height 加两侧 `+0x24` 字段；零宽 glyph 使用 iterator 基准加 advance/line height |
| `+0x20` | token iterator 基准值转 f32 |
| `+0x24..+0x30` | 清零 |
| `+0x34..+0x40` | 当前四个 packed colors |
| `+0x44/+0x48` | `1.0/1.0` |
| `+0x4C..+0x68` | runtime glyph 四组 UV 原样复制 |

Rust 的 `FennelGlyphLayoutRecord` 固定为精确 116 字节，并已实现上述逐字段转换；仍未命名的 iterator 字段保留偏移名，不用猜测语义。

### 游戏实际字符串编码与 token 子集

FontManager 构造路径 `0x7B719C..0x7B71A4` 依次压入 `0x20` 和 `0`，调用 `sub_46B446 -> sub_F320F0 -> sub_F32B40`。后者把第一个参数 `0` 写到实现对象 `+0x04`，把第二个参数 `0x20` 用作最大 font slot 数，并把 `0x24` 写到 `+0x08`。因此本游戏的已执行配置是：

- 字符串编码模式 `0`；
- 最多 `0x20` 个 font slots；
- Fennel 控制前缀为 UTF-16 `U+0024`，即 `$`。

`font::TextBoxObject::setTextByString` (`sub_7C8F40`) 通过 `sub_F321C0` 读取模式。模式 `0` 明确调用 `sub_F525D0`：该函数按 UTF-8 continuation byte 解码，BMP 字符直接写一个 UTF-16 code unit，补充平面字符写 surrogate pair。转换成功后通过虚表 `+0x18` 进入 `sub_7C9070 -> sub_7C90A0`。所以 RFZ TextBox 路径不是旧 fallback 的“两字节 big-endian code”输入；当前游戏配置明确是 UTF-8。

`sub_F3BB80` 建立 token iterator，`sub_F3BD40` 逐 token 输出。当前完整 SRD 语料实际需要且已在 Rust 复现的分支为：

| 输入 | iterator 行为 |
| --- | --- |
| 普通 UTF-16 unit | `sub_F3DA10(..., type=0, code)`；code 写到 token `+0x08` |
| CRLF、CR、LF | type `3` 换行；CRLF 只产生一次换行 |
| `$$` | type `0` 的字面 `$` glyph |
| `$N` / `$n` | type `3` 换行 |
| `$t[x:y]` / `$T[x:y]` | T/t switch 的十进制有符号参数分支；把 x/y 写到 iterator 与 token `+0x20/+0x24`，不单独产生 glyph record |
| NUL、`U+001A` | iterator 结束 |

其他 `$` 命令仍明确返回 unsupported，不会被当作普通文字。当前完整 `surfboard` 语料的 1292 条 RFZ TEXT 全部通过游戏模式的 UTF-8 解码与上述 token 子集，因此当前实际输入覆盖为 1292/1292；未出现的控制命令仍不会被默认为普通文本。

`sub_7C90A0` 的 record stream 边界也已复现：普通 glyph 每个一条 116 字节记录；显式换行写 kind `-1`，超过 128 项 line-start 表时写 `-254`；iterator 结束后再写一个 kind `-1` 和最终 kind `-255`。缺字时游戏会尝试名为 `fennel_npc` 的 EmbeddedSprite；该 fallback 尚未闭环，因此 Rust 当前明确报缺字，不伪造替代 glyph。

### 默认静态排版输入、完整 mode-zero 路径与 fitting guard

TextBoxObject 构造函数 `sub_7BEB80` 先写 `+0x2D0=0`、`+0x2E0=3`、`+0x2E4/+0x2E8=200.0`。`sub_AC6F50` 清除模式位后，静态 SrTextCast 内部状态的 mode 初值为 `0`：

- TEXT `0x78 bit 0` 置位时，默认布局 flags 精确为 `3`；
- TEXT `0x78 bit 0` 清零时，`sub_7C8DA0` 再置 `0x04`，默认布局 flags 精确为 `7`；
- 两者都由 `sub_7C90A0` 选择 `sub_7C1F90`；bit `0x04` 只控制首行横向 auto-fit，不改变布局器选择。

TextBoxObject 基类构造路径把 token iterator 的初始 x/y 状态 `+0x68/+0x6C` 清零。`sub_F2C670` 把 TEXT `+0x1C`（属性 `0x7C`）写到 TextBoxObject `+0x100`，`sub_F3BD40` 的普通 token 分支再把它复制到输出 `+0x6C`；`sub_7C90A0` 最终写入 layout record `+0x20`。同理，`sub_F2C620` 把 TEXT `+0x1E`（属性 `0x41`）写到 TextBoxObject `+0xFC`，作为行距；`sub_F2C5B0` 从 TEXT `0x36` 写入横纵缩放。因此 `FennelStaticTextProperties` 的来源为：

| Rust 输入 | 游戏来源 |
| --- | --- |
| `initial_x/initial_y` | TextBoxObject 构造初值 `0/0`；之后可被 `$t[x:y]` 改写 |
| `glyph_spacing` | TEXT `0x7C` → TextBoxObject `+0x100` → token `+0x6C` → record `+0x20` |
| `scale_x/scale_y` | TEXT `0x36` → TextBoxObject `+0xC4/+0xC8` |
| `line_spacing` | TEXT `0x41` → TextBoxObject `+0xFC` |
| `box_width/box_height` | 所属 CIMG 当前几何尺寸 → TextBoxObject `+0x2E4/+0x2E8` |

`sub_7C1F90` 的实现精确保留：

- 普通模式的视觉右边界为 runtime glyph `bearing_x + width`；水平 advance 为 `record.+0x20 + runtime advance_x`；行高取最大 `em_pixels_y`；
- 首行 auto-fit 使用 `f32::from_bits(0x3F7FBE77)` 的缩放偏置；静态 mode 0 不做统一 Y 缩放；
- 横向/纵向居中均使用 SSE `cvttss2si` 截断到整数后再转回 f32；
- 行距只插在第二行及后续行之前，既不加在第一行之前，也不追加到最后一行之后；
- 每个逻辑行维护当前 advance、视觉右边界、最近空格后的候选指针，以及候选保存时的“空格前” advance/视觉宽度；
- 当前 glyph 首次越宽且属于 FontManager `+0xE4` 集合时，仍把该 glyph 纳入当前逻辑行并只允许一次该状态；
- 其他越宽情况先检查前一个 glyph 是否属于 `+0xE0` 集合，命中时把断点回退一条记录；若存在空格候选，则候选断点最后覆盖该结果；
- 首 glyph 自身越宽且没有可回退断点时，游戏会产生不推进 record 指针的空逻辑行；后续由垂直边界终止，而不是强制把 glyph 塞入一行；
- 垂直检查使用 `abs(current_y) + line_height > box_height`，不包含当前行即将插入的行距；命中后把该逻辑行首记录 kind 改为 `-254` 并停止定位；
- 中/下对齐的测量 pass 若先命中垂直边界，会不写纵向 offset，随后定位出来的前缀因此保持顶对齐；
- 中/下对齐量写入 TextBoxObject `+0x108`，不会折进 layout record `+0x14`；`sub_7C7F90` 随后把它加到 TextBox 的 Y 平移。Rust 因此把 `textbox_vertical_offset` 与 record `y` 分开保存；
- `sub_7C90A0` 尾部扫描所有记录：没有 `-254` 时返回 glyph count；有标记时返回最后一个标记的记录下标，标记位于下标零时返回 `-1`。Rust 的 `record_limit` 保留这一结果。

`layout_fennel_static_fitting_lines` 仍作为原子 guard 保留：它在完整默认布局结果需要自动断行或垂直截止时返回明确错误且不修改输入，便于调用方只接受完整可见矩形；实际游戏路径由 `layout_fennel_static_default` 复现。

真实 1292 条 RFZ TEXT 与对应六套 RFZ runtime font 的逐条审计为：

```text
完整默认静态布局成功=1292
发生自动断行的 TEXT=9
自动生成的逻辑断行总数=16
最终含垂直 -254 标记的 TEXT=19
成功建立 texture batch 的 TEXT=1292
单条 TEXT 最大 texture batch 数=6
因 -254 停止 batch 扫描的 TEXT=19
成功建立 normal-glyph vertex batch 的 TEXT=1292
单条 TEXT 最大 normal-glyph 顶点数=1620
fitting-only guard 成功=1270
UTF-8/控制符/缺字/字体记录错误=0
```

此前 fitting guard 报告的 9 条断行与 13 条直接垂直边界现已全部进入默认游戏状态机；断行后又有部分文本触发垂直截止，因此最终 `-254` 文本数为 19。没有使用通用 Unicode line breaking、自动 fit-to-view 或简单裁切替代。

### layout record 到 texture batch

`sub_7C0D40` 清空已有 batch node 的两个计数和 record 指针数组后，按 TextBoxObject `+0xB0` 保存的完整 record 数顺序扫描：

- kind 为普通非负值时才进入 atlas 分组；`-1`、`-255` 等其他负值直接跳过；
- kind `-254` 立即返回，后面的记录不会进入任何 batch；
- TextBoxObject `+0x2C0` 为非负时，在 `processed + 1 >= maximum` 的当前记录之前立即返回；静态 SrTextCast 构造状态经 `sub_AE36C0 -> sub_AC5740` 明确把该值写为 `-1`，因此当前静态路径不启用该门限；
- 以 record `+0x08` 的 atlas texture token 为 key；同 texture 的 record 指针按出现顺序追加；
- node `+0x0C` 对每个普通 glyph 加一；record `+0x0C & 0x40000` 时 node `+0x10` 的 effect glyph 计数也加一；
- hash 为 wrapping `texture_token + (texture_token >> 3)`。

TextBoxObject 构造函数 `sub_7BEB80` 请求至少 11 个桶，prime table 首项为 `0x11`，所以初始桶数精确为 17。新 key 落入空桶时插入全局前向链表头；落入已有桶时插入该桶连续 node 组的最前端；已有 key 只追加 record，不改变 node 顺序。`sub_7C7F90` 从全局头开始沿 node `+0x00` 遍历，因此 Rust 保存的是这一原始遍历顺序，而不是 texture token 排序。

`sub_7C8BE0` 的 rehash 尚未移植；Rust 在第 18 个唯一 texture token 到达当前未覆盖域时明确报错。本地完整六套 RFZ 字体最多 7 个 atlas 页，1292 条真实文本每条最多产生 6 个 batch，全部严格落在不触发 rehash 的已证明域内。批次审计还确认全部 19 个垂直截止文本都在布局写入的同一 `-254` record 下标停止。

### FontManager 固定断行字符表

`sub_F33C30` 先清空 FontManager implementation `+0xE0/+0xE4` 的两个集合，然后：

- 把 `word_1940AB8` 的 45 个 UTF-16 值插入 `+0xE0`；`sub_F38DB0` 查询该集合；
- 把 `word_1940A9C` 的 14 个 UTF-16 值插入 `+0xE4`；`sub_F38D20` 查询该集合。

Rust 已按原始偏移名保存为 `FENNEL_FONT_MANAGER_SET_E0/E4`，并精确复现两个 membership 查询。断行实现直接遵循 `sub_7C1F90` 对当前/前一 glyph、一次越界状态和空格候选的组合顺序；没有把这两张表扩展成通用 Unicode 规则。

### 116 字节记录到 glyph triangles

`sub_7C10B0` 的无裁剪分支（TextBoxObject flags `& 0x400 == 0`）已经复现为 host-independent Rust 顶点生成器：

- 输出顶点精确为 28 字节：`float3 position`、4 字节 primary color、4 字节 secondary color、`float2 UV`；
- 四角索引表 `dword_18E8084` 的实值为 `[1, 0, 2, 3, 1, 2]`，即直接生成两个 triangle-list 三角形；
- 每个 UV 分量提交前加 `f32::from_bits(0x3727C5AC)`，即约 `0.00001`；
- primary color 取记录 `+0x34` 起对应角的 packed color，secondary color 取 TextBoxObject `+0x32C`；两个颜色都经过 `sub_F25B40 -> sub_5FF4A0`，在顶点内存中写为 BGRA byte 顺序；
- 局部四角由记录 `width/height`、独立调用参数 effective scale、`+0x24..+0x30` 和调用方 x/y 组成；随后按函数中的 4x4 row dot product 做 perspective divide。effective scale 不是重写后的 record 字段，而是 `sub_7C7F90` 计算的 `record.+0x44/+0x48 × TextBox.+0xC4/+0xC8`；
- `RuhunaD3d9AtlasSet` 已复用现有无 D3DX DDS 上传器，可把 RFZ/AVTS 中按 page 排序的 DDS 建为可 device-reset 重建的 D3D9 纹理。

`flags & 0x400 != 0` 的裁剪与 UV 重映射分支还未并入 Rust；因此当前顶点生成器名称和接口明确限定为 `unclipped`。

静态 mode-zero SrTextCast 的 normal-glyph 调用输入也已闭合：

- `sub_AE3580` 把 text state `+0x100` 的 correction mode、effect enable 和 effect offsets 清零；`sub_AC6F50 -> sub_F2C160` 因此把 TextBox byte `+0x98` 写为 0；
- FontObject 构造函数 `sub_F2BB60` 把 TextBox byte `+0x10C` 清零，静态初始化链不调用唯一的 `sub_F2C9C0` setter；
- 默认 flags 为 `3/7`，所以 `sub_7C04F0` 传给 `sub_7C7F90` 的 draw offset 精确为 `(0,0)`，并且不进入 `flags & 0x400` 裁剪；
- correction mode 0 的 glyph 修正为 `x=bearing_x`、`y=bearing_y-flag_mode-line_height+3`，最终两轴再减 glyph `enabled` 并乘 effective scale；
- TextBox position 由 `sub_AC6F50` 写为 `(-SrImage.origin_x,-SrImage.origin_y,0)`，Y 再加布局返回的 `+0x108`；
- 2D 分支通过 `sub_604010` 计算 `TextBox.+0x2EC * local_translation` 后交给 `sub_7C10B0`；3D 分支给 CPU 顶点生成器的只有 local translation，`+0x2EC` 另交 renderer 状态。

Rust 的 `build_fennel_static_unclipped_vertex_batches` 按已证明的 texture 前向链顺序执行上述 normal-glyph 路径，并校验 record/runtime glyph texture token 一致。完整 1292 条 RFZ TEXT 均成功建立批次，单条最多 1620 个顶点；当前语料构造输入未进入 effect。任何 record `+0x0C & 0x40000` 仍明确报错，不生成半套第二 glyph。

### teaFontRenderer batch 与 format 13

`teaFontRenderer` 的 RTTI/vtable 从 `0x18E7EC8` 闭环；虚表 `+0x34` 是上述 `sub_7C10B0`。`sub_7C04F0` 最终直接调用实际绘制函数 `sub_7C7F90`。后者按 texture batch node 迭代，在 `0x7C8284..0x7C82C6` 完成以下步骤：

- 把 batch texture 写到内嵌 DrawPacket texture slot 0；slot 1/2 清零；
- 以 vertex format `13`、primitive `3`、`glyph_count * 6` 个顶点和调用方 `is_2d` 配置 renderer `+0x150` 的动态 batch；
- 每个普通 glyph 消耗 `0xA8 = 6 * 28` 字节；record flag `0x40000` 时再保留一组 `0xA8` 并第二次调用 `sub_7C10B0`；
- 虚表 `+0x10` 进入 `sub_6DF020`，把 `is_2d` 复制到 DrawPacket `+0x60 bit 7` 后提交。

`sub_671D30` 的 vertex declaration switch 中 case 13 精确注册：

1. POSITION float3；
2. COLOR0 D3DCOLOR；
3. COLOR1 D3DCOLOR；
4. TEXCOORD0 float2。

case 14 在这四项之后才增加 TEXCOORD1。因此 Fennel format 13 精确是 SRD format 14 的前 28 字节，而不是根据 `sub_7C10B0` 输出形状反推的自定义声明。Rust 的 `FENNEL_D3D9_VERTEX_DECLARATION` 和 `FennelRenderVertex::STRIDE` 已固定并测试该布局。

### DrawPacket、ShapeEnv 与 shader bytecode

字体 renderer 构造后默认选择 render preset 3。DrawPacket 构造、renderer 初始化和 `sub_7C7F90` 默认模式的最终值为：

```text
draw_flags_00 = 0x02AFE003
flags_60      = 0x00004020（2D 时再置 bit 7，成为 0x000040A0）
field_28      = 31
field_2c      = 0
vertex_format = 13
textures      = [present, null, null]
```

由已经复现的 ShapeEnv key 与 Simple selector 得到：

| batch | ShapeEnv low:high | compact key |
| --- | --- | --- |
| 3D | `0x0036BFB0:0` | `AAMAAABAABGAAAAAAA` |
| 2D | `0x0036BFB8:0` | `EAMAAABAABGAAAAAAA` |

使用原游戏 `cg.dll` 和 `chusanApp.exe` 内的精确 Cg source 只做离线编译，两个 key 均通过 D3D assemble 和 HAL `CreateVertexShader/CreatePixelShader`。输出为：

| shader | bytes | SHA-256 |
| --- | ---: | --- |
| 3D VS | 332 | `86669F24505A70D6DB560C6B2838EBA7D262B0206825BA3927658AB5A7112D61` |
| 2D VS | 384 | `A3E0CA2EFA3452A529DDE7E92EB638FAAD4A230DF5A5537D2D50BE53BC045BD4` |
| shared textured PS | 248 | `066761E3FE149084A9526FDD1A091138B9DC894EAC29FA707D71992E4ED4E23F` |

这些 bytecode 分别与项目中已有的首个 3D VS、2D VS 和单贴图 PS 逐字节相等。`shader_bytecode.rs` 因此把两个 Fennel key 映射到相同的已验证数组，不嵌入重复副本。编辑器运行时不加载 Cg，也不使用 D3DX。

### RFZ atlas sampler

每个 RFZ 的 AVTS entry 0 是一个 Stevia YABX 数据库；其中每个 DDS atlas page 都有一个 `stevia::Texture` 对象。该类序列化字段前缀明确是 `_wrapU/_wrapV/_minFilter/_magFilter/_mipFilter/_anisoNumber/_lodBias`。本地完整六套字体的每一页均为：

```text
[2, 2, 1, 1, 0, 1, 0]
```

这组值也与 `ceylon::resource::Texture` 构造函数 `sub_E7E1E0` 在 `+0x18..+0x30` 写入的默认值逐项相同。`ceylon_texture_sync_sampler_state` (`sub_E7E8B0`) 把它们写入 backing sampler，`d3d9_flush_texture_sampler_state` (`sub_E59410`) 再明确提交：

- type 1/2：ADDRESSU/V，经地址表把内部 2 映射为 D3D9 Clamp；
- type 6/5/7：MIN/MAG/MIPFILTER，经 filter 表把内部 1/1/0 映射为 Linear/Linear/Point；
- type 9/10/8/4：MAXMIPLEVEL、MAXANISOTROPY、MIPMAPLODBIAS、BORDERCOLOR。

字体 DDS 当前均为一层 mip；`ceylon_image_initialize_from_stevia_request` 在 `0xE7FF5F..0xE7FF6E` 把零 mip count 至少提升到 1 并写到 image `+0x64/+0x68`，随后 `sub_E7E8B0` 将其用于 MAXMIPLEVEL。因此当前 atlas 的完整 D3D9 sampler 是 Clamp/Clamp、Linear/Linear/Point、max mip level 1、max anisotropy 1、LOD bias 0、border color 0。Rust 会解析并验证每个 Stevia texture object；页数不匹配、页间 sampler 不同或出现未证明值时直接报错。

`FennelDx9Renderer` 已实现可 ResetEx 失效/重建的 format 13 declaration、精确 shader 对、按需增长的动态 DEFAULT-pool vertex buffer、上述完整 stage-0 sampler、preset 3 的 blend/raster/depth 状态，以及 `D3DPT_TRIANGLELIST(vertices / 3)` 提交。默认静态 layout record、`sub_7C0D40` texture batch membership、normal-glyph CPU vertices 和初始 SrTextCast world/color 均已连接到 Composition。

### ShapeEnv material cull 与像素闭环

Fennel packet 的 `draw_flags_00 = 0x02AFE003` 设置了 `0x00800000`，因此 `ceylon_apply_draw_packet_state` 不会强制 `CULL_NONE`，而是保留 ShapeEnv material 同步出的基础 cull。该基础值的完整来源为：

1. `ceylon_create_shape_environment` (`0x670680`) 创建 `PrimitiveDummyShape`，并以 `sub_E87890` 新建 `ceylon::resource::State` 后交给其 `sea::Material`；
2. State 内的两个 52 字节 `StateParam` 均由 `sub_E859B0` 构造，packed `+0x08` 的低三位为 `0`；
3. Material override mask 初始为零，`sea_material_sync_render_commands` (`0x659810`) 因而从该默认 StateParam 提取低三位到 material command `+0x54`；
4. `sub_E93530` 把它写入全局 RenderState `+0x60`；内部值 `0` 经原版表映射为 `D3DCULL_CW`。

此前直接用全局 RenderState reset 默认值 `1`（`D3DCULL_CCW`）会把 Fennel 的右上→左上→左下三角形全部剔除，导致顶点、alpha 和 viewport 均正常但 Composition 没有任何 changed pixel。这个失败只用于定位，最终实现使用上述二进制闭环得到的 `D3DCULL_CW`，没有保留诊断性的 `CULL_NONE`。

真实语料初始帧审计得到 550 个可见 2D Fennel draw、32694 个顶点。`CHU_UI_Advertise_00_v10.srd` 的 smoke 为 1 draw、834 vertices，使用两个 atlas batch；D3D9Ex Composition 在强制 `ResetEx` 前后均得到 40920 个 changed pixels、`white_pixels=0`、bbox `(651,396)..(1271,683)` 和 FNV-1a `7B466FC4B4E0EC9A`。哈希仅作为当前设备上的稳定诊断值，不定义为跨 GPU 像素规范。

外部 Ruhuna 字形仍不是一条可以随意替换的 ImGui 文本路径。编辑器将上传 RFZ 内嵌 DDS atlas 并提交游戏布局记录对应的 glyph quad；不会使用系统字体冒充。

## 下一证据目标

- `sub_7C1F90` 写入 TextBoxObject `+0x12C/+0x34C` 的内部行元数据及其后续消费者；
- `sub_7C4070/sub_7C5A20` 两个非默认排版器及其 mode 来源；
- Fennel 其余控制 token（颜色、font slot、EmbeddedSprite 等）及 `fennel_npc` 缺字 fallback；
- `sub_7C10B0` 的 `flags & 0x400` 裁剪/UV 重映射分支和 `0x40000` 第二 glyph/effect 的具体来源；
