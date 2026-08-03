# CSLI、SLIC 与父级单元偏移证据

本页只记录由游戏二进制读取、运行时消费和本地样本三方闭环的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`B4C9D857618B44DF808F84D3590CC5A004193D4AD83B4C3B5A9A51F2A0D4495E`

## DATA、CSLI 与 NODE 的连接

`srd_parse_cast` (`0xAA0130`) 把 `DATA` 交给 `0xAA0570`；后者把 `CSLI` 交给 `srd_parse_csli` (`0xAA3320`)。这证明 CSLI 位于 `CAST → DATA → CSLI` 分派路径，而不是依据标签名推测。

`srd_parse_csli` 把属性 `0x51` 通过无符号标量读取器读入一个初始为 `-1` 的 32 位局部变量。若最终按有符号比较仍不小于零，则执行：

```text
NODE_base + 92 * node_index + 80 = parsed_csli_pointer
```

`srd_parse_cimg` (`0xAA1410`) 也会向同一个 NODE `+80` 槽写入其解析结果，因此该槽应保持“类型专属数据指针”的中性含义。运行时工厂 `0xACA030` 读取 NODE `+72` 的低字节进行类型分派；低字节为 `2` 时选择会消费 CSLI 数据的运行时类型。NODE 属性 `0x30` 的其余高位仍不命名，Rust 仅提供低字节 `cast_type()`。

## CSLI 原始布局

`srd_parse_csli` 先清零一份 104 字节模板，再把模板 `+20..+35` 的 16 字节设为 `0xFF`。已证明字段如下：

| 属性 | 偏移 | 行为 |
| --- | ---: | --- |
| `0x80` | `+0` | unsigned u32 |
| `0x40` | `+4` | 标量转 f32 |
| `0x41` | `+8` | 标量转 f32 |
| `0x42` | `+12` | 标量转 f32 |
| `0x43` | `+16` | 标量转 f32 |
| `0x44` | `+20` 起 | 每次写四字节 `[source1, source2, source3, source0]`，目标前移 4 字节 |
| `0x4B` | `+36` | unsigned 后保留低 8 位 |
| `0x81` | `+48` | unsigned 后保留低 16 位；列数 |
| `0x82` | `+50` | unsigned 后保留低 16 位；行数 |
| `0x84` | `+52` | unsigned 后保留低 16 位；X 默认尺寸除数的减数 |
| `0x85` | `+54` | unsigned 后保留低 16 位；Y 默认尺寸除数的减数 |
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

## 20 字节运行时单元记录

`srd_generate_csli_cell_rects` (`0xAD26B0`) 接收四个尚未完成上游语义命名的 f32 输入，以及 CSLI 指针。Rust 因此保留中性名称 `extent_inputs[4]`。默认尺寸按汇编顺序计算：

```text
default_width  = (extent_inputs[0] - extent_inputs[2])
               / max(float(columns - divisor_subtract_x), 1.0)
default_height = (extent_inputs[1] - extent_inputs[3])
               / max(float(rows - divisor_subtract_y), 1.0)
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

`srd_get_cast_csli_cell_rects` (`0xAD3520`) 只在原始 NODE `+72` 的低字节等于 `2` 时调用上述生成器。它传入运行时 CAST `+0x178` 开始的四个 f32 和 CAST `+0x0C` 的类型专属数据指针；其他类型返回空记录表。

## NODE 0x32 与父级偏移

NODE 属性 `0x32` 从原始 NODE `+0x4C` 复制到运行时 CAST `+0x194`。`srd_compute_parent_csli_cell_offset` (`0xABF140`) 证明它的限定语义：

- 下标小于零时输出 `[0,0]`。
- 向父 CAST 请求上述 20 字节记录表；下标越界或目标 active 字节为零时输出 `[0,0]`。
- 父 CAST `+0x178/+0x17C` 两个 f32 作为尺寸输入。
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

父 CAST 虚表槽 `+0x30` 返回非零时，`y = (first + second) * 0.5`。返回零时按原汇编顺序改写：

```text
second = -second
first  = cell.height + second
y      = (first + second) * 0.5
```

该虚函数的最终坐标系含义尚未证明，Rust 参数只命名为 `axis_mode`。同理，`+0x178` 四个输入的生产链尚未闭环，因此不能先命名为 left/top/right/bottom。

这个二维结果随后作为已有局部矩阵构建器的 offset 输入，因而 Rust 已提供 `compute_parent_csli_offsets` 和 `compose_world_matrices_with_csli_layout` 完成相同连接，同时要求调用方显式提供尚未证明来源的运行时输入。

## 样本验证

全部 53 个本地 SRD 回归得到：

- 799 个 CSLI 均通过 `0x51` 链接到同一 LAYR 的 NODE，且目标 NODE 类型低字节均为 `2`。
- 每个 CSLI 的 `columns * rows` 都与实际 SLIC 记录数一致。
- 23 个非负 NODE `0x32` 均有层级父节点；父节点类型低字节均为 `2`，均有已链接 CSLI。
- 23 个下标全部在父级网格范围内，且对应 active 位全部为 1。

## 尚未闭环

- CAST `+0x178` 四个 f32 的上游计算及最终几何命名。
- 父 CAST 虚表槽 `+0x30` 的具体实现与业务名称。
- CSLI/CREF 的纹理资源选择、每个网格单元如何生成最终 D3D9 顶点与 UV。
- SLIC packed 字段、`0x46` 和 CSLI 其余字段的最终渲染语义。
