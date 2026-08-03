# CSLI、SLIC 与父级单元偏移证据

本页只记录由游戏二进制读取、运行时消费和本地样本三方闭环的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`EF9E8FE7957CD8055B3AD9CB91982FB70C2B2B8D7B9DBB660B4B925404BC7F3C`

## DATA、CSLI 与 NODE 的连接

`srd_parse_cast` (`0xAA0130`) 把 `DATA` 交给 `0xAA0570`；后者把 `CSLI` 交给 `srd_parse_csli` (`0xAA3320`)。这证明 CSLI 位于 `CAST → DATA → CSLI` 分派路径，而不是依据标签名推测。

`srd_parse_csli` 把属性 `0x51` 通过无符号标量读取器读入一个初始为 `-1` 的 32 位局部变量。若最终按有符号比较仍不小于零，则执行：

```text
NODE_base + 92 * node_index + 80 = parsed_csli_pointer
```

`srd_parse_cimg` (`0xAA1410`) 也会向同一个 NODE `+80` 槽写入其解析结果，因此该槽应保持“类型专属数据指针”的中性含义。`srd_init_cast_factory_registry` (`0xAC8E70`) 依次注册 key `0..5`；`srd_create_runtime_cast_for_node` (`0xACA030`) 对 NODE 类型低字节 `2` 选择 key `3`，对应 `srd_create_slice_cast` (`0xAC9EF0`)。该类 vtable 的 MSVC RTTI 名称为 `surfride::SrSliceCast`，因此 type 2 与 CSLI sliced CAST 的关系已经闭环。NODE 属性 `0x30` 的其余高位仍不命名，Rust 仅提供低字节 `cast_type()`。

## CSLI 原始布局

`srd_parse_csli` 先清零一份 104 字节模板，再把模板 `+20..+35` 的 16 字节设为 `0xFF`。已证明字段如下：

| 属性 | 偏移 | 行为 |
| --- | ---: | --- |
| `0x80` | `+0` | unsigned u32 |
| `0x40` | `+4` | 标量转 f32；SrImage 宽度 |
| `0x41` | `+8` | 标量转 f32；SrImage 高度 |
| `0x42` | `+12` | 标量转 f32；origin mode 超出 `0..8` 时的自定义 X |
| `0x43` | `+16` | 标量转 f32；origin mode 超出 `0..8` 时的自定义 Y |
| `0x44` | `+20` 起 | 每次写四字节 `[source1, source2, source3, source0]`，目标前移 4 字节 |
| `0x4B` | `+36` | unsigned 后保留低 8 位；origin mode |
| `0x81` | `+48` | unsigned 后保留低 16 位；列数 |
| `0x82` | `+50` | unsigned 后保留低 16 位；行数 |
| `0x84` | `+52` | unsigned 后保留低 16 位；首行显式宽度单元数 |
| `0x85` | `+54` | unsigned 后保留低 16 位；首列显式高度单元数 |
| `0x45` | `+56` | unsigned 后保留低 16 位；CREF 记录数 |
| `0x51` | 临时变量 | unsigned NODE 下标，用于写 NODE `+80` |

解析器计算 `cell_count = columns * rows`。仅当结果大于零时分配 `64 + 40 * cell_count` 字节，并从模板复制 104 字节。`CREF` 子块按 `0x45 * 4` 字节另行分配并调用 `srd_parse_cref_like`；其记录内容和运行时选择仍未闭环。`SLIC` 子块从 CSLI `+64` 开始写 40 字节记录。

## SLIC 记录

`srd_parse_slic` (`0xAA37A0`) 的 40 字节记录：

| 属性 | 偏移 | 行为 |
| --- | ---: | --- |
| `0x83` | `+0` | unsigned u32 flags |
| `0x40` | `+4` | 标量转 f32；flags 位 0 置位时作为显式宽度 |
| `0x41` | `+8` | 标量转 f32；flags 位 1 置位时作为显式行高 |
| `0x3A` | `+12` | 四字节重排为 `[1,2,3,0]` |
| `0x33` | `+16` | 四字节重排为 `[1,2,3,0]` |
| `0x44` | `+20` 起 | 重复四字节重排，目标每次前移 4 字节 |
| `0x46` | `+36` | signed 后保留低 16 位 |

每遇到属性 `0xFE`，解析器先检查当前 flags：若 `0x200` 清零，则设置 `0x100`；随后目标推进 40 字节。属性循环结束后还会对当前记录执行一次相同的 flags 收尾。这里的 `0x100` 业务名称来自它在下游生成记录时被直接转成 active 字节，而不是从旧编辑器继承。

## SrSliceCast 的尺寸、原点与显式单元累计

`srd_init_slice_cast_image` (`0xADB290`) 把内嵌 `surfride::SrImage`（CAST `+0xF8`）和 CSLI 指针传给 `srd_init_srimage_from_csli` (`0xAD2B40`)。后者建立下列运行时字段：

```text
CAST +0x178/+0x17C = origin X/Y
CAST +0x184/+0x188 = width/height（CSLI 0x40/0x41）
CAST +0x18C         = 首行所有 flags bit0 单元的显式宽度之和
CAST +0x190         = 首列所有 flags bit1 单元的显式高度之和
```

宽度和高度累计都按 SLIC 行优先顺序使用 f32 加法。`srd_set_srimage_size_and_origin` (`0xAD2EB0`) 对 origin mode `0..8` 使用 EXE 中的固定系数表：

```text
0:(0,0)   1:(0.5,0)   2:(1,0)
3:(0,0.5) 4:(0.5,0.5) 5:(1,0.5)
6:(0,1)   7:(0.5,1)   8:(1,1)
```

origin 等于系数逐分量乘 width/height。mode 大于等于 `9` 时不覆盖先前从 CSLI `0x42/0x43` 复制的自定义值。

## 20 字节运行时单元记录

`srd_generate_csli_cell_rects` (`0xAD26B0`) 从上述 SrImage 字段和 CSLI 计算默认尺寸：

```text
default_width  = (width - first_row_explicit_width_sum)
               / max(float(columns - explicit_width_cell_count), 1.0)
default_height = (height - first_column_explicit_height_sum)
               / max(float(rows - explicit_height_cell_count), 1.0)
```

生成器按行优先遍历 SLIC。每行只用该行第一个 SLIC 的 flags 位 1 和 `+8` 选择整行高度；每个单元分别用 flags 位 0 和 `+4` 选择宽度。输出记录固定为 20 字节：

```text
+0  u8  active = (SLIC.flags >> 8) & 1
+4  f32 当前行内累计 X
+8  f32 当前累计 Y
+12 f32 当前单元宽度
+16 f32 当前行高度
```

写完单元后累计 X，写完一行后累计 Y。Rust 保留了这些 f32 运算的先后顺序。

`srd_get_cast_csli_cell_rects` (`0xAD3520`) 只在原始 NODE `+72` 的低字节等于 `2` 时调用上述生成器。它把 CAST `+0x178` 作为 SrImage 字段基址，并传入 CAST `+0x0C` 的 CSLI 指针；其他类型返回空记录表。

## NODE 0x32 与父级偏移

NODE 属性 `0x32` 从原始 NODE `+0x4C` 复制到运行时 CAST `+0x194`。`srd_compute_parent_csli_cell_offset` (`0xABF140`) 证明它的限定语义：

- 下标小于零时输出 `[0,0]`。
- 向父 CAST 请求上述 20 字节记录表；下标越界或目标 active 字节为零时输出 `[0,0]`。
- 父 CAST `+0x178/+0x17C` 的 SrImage origin 作为减数。
- X 输出严格为：

```text
first  = cell.x - parent_size.x
second = cell.width + first
x      = (second + first) * 0.5
```

- Y 先计算：

```text
first  = cell.y - parent_size.y
second = (cell.height - parent_size.y) + cell.y
```

父 CAST 虚表槽 `+0x30` 的实现 `srd_cast_is_2d` (`0xACA350`) 返回 `(CAST.flags+0x4C & 2) != 0`。`srd_build_runtime_layer` 把 `is_2d = (LAYR.flags & 1) == 0` 传给每个 CAST 初始化器，后者把该值写入 flags 位 1。因此 2D layer 使用 `y = (first + second) * 0.5`；3D layer 按原汇编顺序改写：

```text
second = -second
first  = cell.height + second
y      = (first + second) * 0.5
```

这个二维结果随后作为已有局部矩阵构建器的 offset 输入。Rust 的 `compute_parent_csli_offsets` 和 `compose_world_matrices_with_csli_layout` 现在完全由 LAYR、NODE、CSLI 和 SLIC 数据复现该链，不再要求调用方提供推测性的布局输入。

## 样本验证

全部 53 个本地 SRD 回归得到：

- 799 个 CSLI 均通过 `0x51` 链接到同一 LAYR 的 NODE，且目标 NODE 类型低字节均为 `2`。
- 每个 CSLI 的 `columns * rows` 都与实际 SLIC 记录数一致。
- 每个 CSLI 的 `0x84` 都等于首行 flags 位 0 置位的单元数，`0x85` 都等于首列 flags 位 1 置位的单元数。
- 23 个非负 NODE `0x32` 均有层级父节点；父节点类型低字节均为 `2`，均有已链接 CSLI。
- 23 个下标全部在父级网格范围内，且对应 active 位全部为 1。

## 尚未闭环

- CSLI/CREF 的纹理资源选择、每个网格单元如何生成最终 D3D9 顶点与 UV。
- SLIC packed 字段、`0x46` 和 CSLI 其余字段的最终渲染语义。
