# CNUM 与 SrNumberCast 证据

本页只记录已经由 CNUM 解析器、运行时对象初始化、数字格式化和 glyph 记录生成代码共同闭环的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`B874FB682B5F963DB72E0660A6D9D7CFA061A787851C652F8A09056AB1779077`。

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
| `0x78` | `+0x3C` u32 | bits `0x0C` 选择数字串左/中/右横向放置 |
| `0x81` | `+0x40` i32 | 初始整数部分 |
| `0x82` | `+0x44` f32 | 初始小数部分 |
| `0x83/0x84` | `+0x68/+0x6A` i16 | 普通数字横向 advance / 高度 |
| `0x85/0x86` | `+0x6C/+0x6E` i16 | 逗号与小数点共用的 advance / 高度 |
| `0x87` | `+0x70` i16 | 逗号纵向偏移 |
| `0x88` | `+0x72` i16 | 分组分隔符的字符间隔 |
| `0x89` | `+0x74` i16 | 整数位数限制及补零宽度 |
| `0x8A` | `+0x76` i16 | 普通字符间距 |
| `0x8B` | `+0x78` i16 | 小数位数 |
| `0x8C` | `+0x7C/+0x80` f32 | 小数位数字的 X/Y 缩放 |
| `0x8D` | `+0x84` i16 | 小数位字符间距 |
| `0x8E` | `+0x86` i16 | 小数位数字纵向偏移 |
| `0x8F` | `+0x88` i16 | 小数点纵向偏移 |
| `0x90` | `+0x8A` i16 | 数值变化时的 glyph 动画模式 |
| `0x91..0x94` | `+0x8C..+0x92` i16 | `+`、`-`、`,`、`.` 的 CREF 下标 |
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

## 整数与小数字符串格式

`srd_number_format_integer_digits` (`0xAE0280`) 使用 `0x89` 计算 `10^count`，对当前整数部分取 wrapping abs，再把它限制到 `10^count - 1`。format flags 已闭环的位为：

| 位 | 行为 |
| --- | --- |
| `0x01` | 总值非负时添加 `+`；负值始终添加 `-` |
| `0x02` | 调用 `srd_number_insert_group_separators` (`0xAE0B40`)，每 `0x88` 个字符从右向左插入 `,` |
| `0x04` | 按 `0x89` 指定宽度对整数补零 |
| `0x08` | 生成小数字符串 |
| `0x10` | 小数位不足 `0x8B` 时在右侧补零 |
| `0x20` | 小数点后的字符间距改用 `0x8A`，否则使用 `0x8D` |

`srd_number_format_fraction_digits` (`0xADE250`) 对当前 double 小数部分清除符号位，以 `%lf` 生成六位小数文本，把所有 `0.` 替换为空串，再把末尾 `0` 删除到至少剩一个字符。之后按 `0x8B` 处理：过长且 `0x8B >= 0` 时截断；过短且 flags `0x10` 有效时补零。只有最终小数字符串非空，组合串才插入 `.`。

`srd_number_rebuild_glyph_geometry` (`0xADD6C0`) 依次生成 sign、integer、decimal-point、fraction 四组 glyph 记录，同时把四段连接成完整字符串交给位置生成器。总值的符号判断使用 `double(integer) + fraction`，但整数和小数字符串分别对各自分量取绝对值。

## 字符宽度与四顶点位置

`srd_number_measure_string_width` (`0xADE140`) 以 f32 顺序累加字符 advance 与间距，最后调用 `ceil`。`srd_number_build_glyph_positions` (`0xADDD60`) 使用该宽度和 `0x78 & 0x0C` 放置字符串：`0` 从 `-origin.x` 开始，`4` 在 CNUM 宽度中居中，`8` 右对齐。

每个字符生成四个 `[x,y,0]`，顺序与 SrImage quad 一致：左上、左下、右上、右下。普通整数数字使用 `0x83/0x84`；逗号与小数点使用 `0x85/0x86`，并分别用 `0x87/0x8F` 调整基线；小数位数字把 `0x83/0x84` 分别乘以 `0x8C[0]/[1]`，再用 `0x8E` 调整基线。CAST 的轴模式为零时，top 与 height 都取反，精确复现二维/三维分支的 Y 顺序。

位置生成器和宽度测量器对标点附近间距使用不同控制流，但对已生成字符串得到相同总宽度：测量器在普通字符后加间距，位置生成器在下一字符不是标点时加间距。Rust 分别保留两条原始控制流，没有合并成推测性的排版规则。

glyph 记录只在 `mapped_index < CREF count` 的 signed 比较成立时建立；负下标因此会建立记录，但 `srd_render_number_glyph` (`0xADF860`) 在记录 `+0x30 < 0` 时跳过绘制。位置数组则为完整字符串的每个字符生成；随后游戏只把位置数组前 N 项顺序复制到 N 个已建立记录，而不是按被过滤字符的原下标配对。Rust 保留了这一非直观行为。

绘制单个 glyph 前，`srd_set_image_reference_index` (`0xAD4730`) 只把该 glyph 的映射下标写入通道 `0` 的坐标描述符；随后 CREF 使用 glyph 下标解析，CRE1 仍使用 NumberCast 初始化时的选择器 `0`。因此静态 glyph 的两张引用表消费路径也已闭环到 TEXL/CROP。

## Rust 对应与样本验证

`NumberDefinition` 复现上述默认值、属性布局、CREF/CRE1 表、初始数值、格式化、字段访问、字符串宽度、逐字符 quad、glyph 过滤顺序和每 glyph 的 CREF/CRE1 坐标状态。`Layer::from_block` 根据 CNUM `0x51` 把定义连接到 NODE，并验证对应 NODE 的 cast type 为 `4`。

首次 history 提交现已闭环。SrNumberCast 构造器把 `+0x228/+0x22C/+0x230` 的 56-byte history vector 清空；`srd_render_number_cast` 每次先调用 `srd_number_rebuild_glyph_geometry`，按 `0x90` 清理旧记录，再用 `srd_append_number_glyph_record` 追加当前记录。`srd_render_number_glyph_history` 在 history count `<= 1` 时不进入 pair blend，而以 alpha `1.0` 依次提交 sign 正序、integer 逆序、decimal-point 正序、fraction 正序。`srd_render_number_glyph` 对负 glyph 下标直接返回；其余每 glyph 各调用一次 `srd_begin_quad_draw`。

纹理绑定与 UV 的两个时点也已区分：`srd_render_number_cast` 在进入 history 前，以当前 CREF/CRE1 descriptor 各解析一次并调用 `srd_select_render_texture_pair`，因此整次 NumberCast 共用这两个 packet texture slot；每 glyph 随后只用 `srd_set_image_reference_index` 替换 CREF selector 并清除该 descriptor 的显式矩形标志，再重建两组 UV，不重新绑定 texture。Rust 的 `NumberGlyph` runtime draw 保留该公共 binding 与逐 glyph UV 的差异，并保存 glyph/segment 身份、每次 packet 与 renderer layer key。

本地 53 个 SRD 的回归测试解析并链接了 552 个 CNUM、5852 条 CREF 和 12 条 CRE1；初始格式化共生成 2192 个可绘制 glyph，2192 个 glyph 的 CREF 都能解析到 TEXL/CROP。接入特殊 CAST matrix 后，完整 91 文件统一 runtime 语料得到 2696 个实际 NumberGlyph draw，其中 2223 个来自 reference copy、6 个为 3D；实际可达的 value animation mode 为 `0/1`，精确出现 `AAEBABBAABGAAAAAAA`、`EAEBABBAABGAAAAAAA` 与 `EAEBABBAADGAAAAAAA` 三个已经验证的单贴图 shader key。mode 0 在每次 append 前清空旧 history；mode 1 的新增可达实例由完整语料回归固定，不用未证明的历史插值代替。

固定 D3D9Ex 回归使用 `common/time/CHU_UI_Time_00_v10.srd`、scene 0、`AnimationSet1`、frame 0 与 CommonBackground/MainScene 宿主。runtime stream 为 10 个 draw、4 个 target group，其中 3 个 NumberGlyph source；关闭 NumberGlyph 后 Composition 有 2369 个 RGB 像素变化，强制 `ResetEx` 前后相同。

## 动画边界与仍未闭环

- `srd_apply_animation_motion_set` 的完整调用边界和 `srd_apply_cast_animation_channels` 的穷尽 switch 已证明：ANIM/TRK 不存在另一组 Number 专用目标，`11..17/20` 只修改内嵌 SrImage，`23` 对 NumberCast 是虚表空操作。CNUM 数值、格式和布局字段不会由这条 ANIM 路径绑定；数值变化来自 `srd_number_cast_set_value_parts` 等外部调用路径；
- shipped corpus 未出现的 `0x90 = 1..8` 历史 glyph pair 插值/裁剪；实现不会把 mode 0 的单记录行为外推到这些模式；
- 外部宿主调用 `srd_number_cast_set_value_parts` 后的编辑器输入接口；当前 fresh runtime 使用 CNUM 初值，ANIM/TRK 已证明不会修改数值字段；
- packet stencil sequence 与 special-depth 的未完成公共边界；普通 mode-0 NumberGlyph 的混合、alpha、深度、裁剪、shader、纹理、36 字节顶点和 D3D9Ex 提交已经接入。
