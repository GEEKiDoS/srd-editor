# CAST CATR 与 ExtParamData 证据

本页记录 `CAST/CATL/CATR` 的节点挂接、通用属性记录以及 `ExtParamData` 到图像 render preset 和层级键的完整运行时链。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`4B54743085DC6EA79F10DFDA84A37A9B400FC9A204AB0635A0619473B441757E`
- 完整本地语料：`D:\sdhd\assets\data\surfboard`

## CATL/CATR 记录

`srd_parse_catl` (`0xAA0380`) 遍历 `CATL` 的直接 `CATR` 子块。每个 `CATR` 先分配一个 8 字节列表头：

| 字段 | 含义 |
| --- | --- |
| `+0x00` | 属性记录声明数量 |
| `+0x04` | 72 字节属性记录数组 |

`CATR` 属性 `0x0E` 提供声明数量并触发 `count * 72` 分配。随后的记录由 `sub_AA0EC0` (`0xAA0EC0`) 按原顺序读取：

- 属性 `0x03` 写入当前记录 `+0x00` 的 64 字节名称；
- 属性 `0x0F` 写入 `+0x40` 的源 type code 和 `+0x44` 的值指针，然后推进到下一条 72 字节记录；
- type `1/8` 分配并保存 signed scalar；
- type `2` 分配并保存字符串；
- type `10` 分配并保存 f32；
- 其他 type 不进入上述已证明的分配分支。

`srd_parse_catl` 还读取属性 `0x51` 作为 NODE 下标。非负值把该 8 字节列表头写到 `CAST` 解析后 NODE 记录的 `+0x44`；同一 NODE 后出现的列表会覆盖先前指针。Rust 同时保留全部列表，并用单独的最后挂接索引复现该运行时选择，避免丢失被覆盖的原始结构。

## ExtParamData 的特殊转换

每个 type `2` 字符串记录都会进入 `sub_AA4890` (`0xAA4890`)。只有名称与 `ExtParamData` 完全相等时，它才把字符串按逗号原地分词，分配 12 字节结构，将属性运行时 type 改为 `30`，并用该结构替换原字符串指针。

结构布局和构造默认值为：

| 偏移 | 默认值 | 作用 |
| --- | --- | --- |
| `+0x00` i32 | `-1` | SrImage render-preset override |
| `+0x04` u8 | `0x80` | layer level 编码 |
| `+0x05` u8 | `5` | layer 编码 |
| `+0x06` u16 | `0` | 本链未发现单独写入 |
| `+0x08` u32 | `1` | value/enable flags |

识别的 token 与机器级变换为：

| token | 精确结果 |
| --- | --- |
| `blendMode#N` | `N <= 0` 写 `-1`；否则写 `N + 33` |
| `layerLevel#N` | 只取 `AL`，再减 `0x80`，按 u8 回绕 |
| `layer#N` | 只保存 `AL` |
| `layerKind#N` | flags bit 0 写成 `N == 0` |
| `enableKind#N` | flags bit 1 写成 `N != 0` |
| `enableLayer#N` | flags bit 2 写成 `N != 0` |
| `enableLevel#N` | flags bit 3 写成 `N != 0` |

比较是区分大小写的固定前缀 `strncmp`，数值由 CRT `atoi` 读取。Rust 覆盖完整语料使用的有符号 32 位十进制域；超出该域的畸形字符串行为尚未作为格式保证。

## NODE 到运行时 CAST

`srd_init_runtime_cast_from_node` (`0xAD3F10`) 遍历 NODE `+0x44` 的每个 72 字节属性记录。`sub_AB8F40` (`0xAB8F40`) 检查运行时 type 是否为 `30`；匹配时调用 `sub_ABB590` (`0xABB590`) 取得 12 字节结构并原样复制到运行时 CAST `+0x198`。

图像型 CAST 的 `SrImage` 子对象从运行时 CAST `+0xF8` 开始，因此这次复制具有可验证的字段重叠：

```text
runtime CAST + 0x198 == SrImage(+0xF8) + 0xA0
```

`srd_render_slice_cast` 在 `0xADA9ED` 明确计算 `CAST+0xF8` 并传给 `srd_select_image_render_preset`；后者在 `0xAC6D58` 读取参数 `+0xA0`，非负时原样提交给 `ceylon_set_draw_render_preset_id`。相同 selector 也由 CIMG、CNUM 等图像型路径调用。因此 `blendMode#N -> N+33` 不是名称推断，而是 `CATR -> type 30 -> CAST+0x198 -> SrImage+0xA0 -> draw packet preset` 的完整闭环。

## FontParamData 与 SrTextCast

`FontParamData` 不会像 `ExtParamData` 一样在通用 CATR 解析阶段转换为 type 30。SrTextCast 的虚方法 `sub_AD9BF0` 遍历 NODE `+0x44` 的原始 72 字节记录，固定把目标设为 `SrTextCast+0x2F4`，并按记录顺序调用 `sub_AB8720`。后者只有在名称与 `FontParamData` 完全相等时才把字符串先按逗号、再按 `#` 分词；每项至少需要名称和值两个字段，额外字段被忽略。

扩展对象由 `sub_AE3580` 初始化。已闭合的字段、默认值和 token 为：

| 扩展偏移 | 默认值 | token / 写入规则 |
| --- | ---: | --- |
| `+0x00..+0x03` | false | `vertical/prohibition/wordWrap/monospaced`，值仅在精确等于 `True` 时为真 |
| `+0x04` | false | 游戏字符串拼写为 `diplayShadow` |
| `+0x08` | 0 | `autoScalingHeight` 写 0/1；`noWrapPutMode` 用 `atoi` 后夹到 `0..6`；按 token 顺序后写覆盖前写 |
| `+0x0C/+0x10` | 0/0 | `shadowX/shadowY` |
| `+0x14/+0x18` | 32/32 | `pointX/pointY` |
| `+0x1C/+0x20/+0x24/+0x28` | 0 | `outline/italic/bold/faceId` |
| `+0x2C/+0x30/+0x34` | 40/2/2 | `scrollSpeed/scrollWait`；`+0x34` 当前没有命名 token |
| `+0x38/+0x3C` | 0/0 | `shadowColor/outlineColor`；`atoi` 的 32 位结果按原字节保存 |

`sub_AC6F50` 先根据 `+0x08` 执行 mode switch，随后总是把 `prohibition/wordWrap/monospaced` 分别写入 TextBox flags `0x01/0x02/0x200`，所以它们可以清掉构造器的低位。`vertical` 写 FontObject correction byte；`diplayShadow=True` 设置 record flag `0x40000`，把 `shadowX/Y` 转为 f32 写入 effect offset，并把 `shadowColor` 的源字节按 `2,1,0,3` 排列后复制到四个 effect color。Rust 按同样顺序解析记录并已把 mode、最终 flags、裁剪和 shadow effect 接到初始 Fennel draw；完整语料的 text `vertical=True` 为 0，因此尚未泛化未使用的 correction 分支。

同函数把当前 FontObject 24 字节 style 复制到栈上，把 `pointX/pointY` 分别写入 style `+0x02/+0x04`，随后 `sub_F2C100` 清除两者高字节；因此精确结果是 signed `max(value,1)` 的低 8 位。outline/italic/bold 分别控制 style flags `0x08/0x04/0x02`，数值落在 `+0x10/+0x14/+0x0C` 并按 `0xF/0x3/正数时 0xF` 掩码。token iterator 将这 24 字节 style 复制到 glyph 请求；RFZ 的 `FontDriverRFO` 虚方法 `sub_F3B780` 只用请求 `+0x00` 的字符码调用 `sub_F41C50`，并只把请求 `+0x08` 的 flags 复制到 runtime glyph `+0x08`。它不读取两个 point 值或三个数值 style 字段；`faceId` 在整个 `sub_AC6F50` 中没有读取。`sub_7C90A0` 再把请求 flags 写到 layout record `+0x0C`，当前 record batch/render 消费者只检测其中的 shadow bit `0x40000`。

## 层级键合成

`srd_update_cast_tree` (`0xAC0F80`) 从父键开始，只在对应 enable bit 开启时替换字段：

```text
if flags & 2: key bit 15      = flags bit 0
if flags & 4: key bits 8..14 = sign_extend(layer) & 0x7F
if flags & 8: key bits 0..7  = layer_level
```

结果写入运行时 CAST `+0x50`。`srd_render_cast` 随后把该值复制到 renderer `+0x198`，并把解析 NODE `+0x58` 的低字节偏移加到最低字节；`srd_render_runtime_layer` 在层遍历结束后恢复 renderer 原值。这证明三个 layer token 控制的是继承式绘制层级键，而不是纹理坐标或像素着色公式。

## 完整语料回归

91 个完整 SRD 得到：

- 29,138 个 `CATR` 列表，全部通过 `0x51` 挂接到合法 NODE；
- 68,511 条通用属性记录；
- 每个列表恰有一条 `ExtParamData`，共 29,138 条；
- 17,825 个列表含 `FontParamData`；最终 mode 分布为 `0:17689, 1:1, 2:16, 4:119`；
- 1,292 个 RFZ TextCast 的实际初始 mode 为 `0:1173, 2:12, 4:107`，其中 272 个 monospaced、110 个 shadow、0 个 vertical；record style flags 精确为 `1:1182, 0x40001:110`；
- point size 为 `32x32:1278, 28x28:12, 21x21:2`；14 个非默认值全部处于 `sub_7C04F0` 的 `flags&0x20` 路径，不进入 pointY/2 margin 分支；outline/italic/bold/faceId 非零数均为 0；
- 28,863 条 render-preset override 为 `-1`；其余覆盖值分布在 `34..58` 和 `60`，没有把缺失的 `59` 自行补成合法样本值；
- 图像型 CIMG/CSLI/CNUM 中，19,210 条 override 为 `-1`，274 条为非负覆盖；覆盖后有效 preset 除默认 `3/4/5/9` 外，确实出现 `34..58` 和 `60`。

Rust 语料测试同时保留旧 53 文件集合的独立精确统计，防止把两个版本的文件差异误判为解析回归。

## 证据边界

已经闭环：CATR 列表和 72 字节记录布局、NODE 挂接、四种已处理源 type、`ExtParamData` token、12 字节结构、SrImage preset 覆盖、继承式层级键，以及 SrTextCast 对原始 `FontParamData` 的顺序解析与初始 RFZ mode/TextBox flags/style record/shadow 输入。

尚未闭环：其余命名 CATR 数据各自的业务结构、未处理源 type 的运行时对象、畸形/溢出 `atoi` 输入、FontParam vertical/scroll 的实际消费与 mode 6 显式 API 调用点，以及 preset `34..60` 对最终 ShapeEnv shader 模块的逐项像素公式。后者必须继续由 Ceylon shader-key 与生成代码证明，不能仅凭属性名称猜测。
