# TEXT、FONT/CHAR 与外部 RFZ 字体资源

状态：SRD 内 TEXT、FONT、CHAR 的记录布局，TEXT 到项目 FONT 下标解析，SrTextCast 建立/初始化，外部 RFZ/YABX/Ruhuna/AVTS/DDS 字体资源，Ruhuna Database/Glyph 到游戏 128 字节 runtime glyph 的转换，以及 RFZ `TextBox` 路径与旧式 FONT/TEX/CROP 路径的运行时分流均已闭环。二进制证明 RFZ glyph 不会先被改写成 FONT 两个 i16；当前继续复现 `TextBoxObject` 的排版和最终 glyph draw。

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

外部 Ruhuna 字形仍不是一条可以随意替换的 ImGui 文本路径。编辑器将上传 RFZ 内嵌 DDS atlas 并提交游戏布局记录对应的 glyph quad；不会使用系统字体冒充。

## 下一证据目标

- `sub_7C1F90/sub_7C4070/sub_7C5A20` 三个 116 字节记录排版器的对齐、换行、裁剪和方向分支；
- Fennel token iterator 的字符串编码、控制 token 和位置输出；
- `sub_7C10B0` 的 116 字节 glyph item 到 28 字节六顶点输出，以及 atlas texture 的最终提交状态；
- 外部字体 glyph quad 进入 D3D9Ex composition 的真实 SRD/RFZ 端到端回归。
