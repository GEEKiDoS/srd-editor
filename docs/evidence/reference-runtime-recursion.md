# SrRefCast 独立运行时层与递归绘制证据

分析对象：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`CDADC82B2932CF09122A1C73E7753446F7AAA3F7176A2DEE4F43902BBE725E7F`。

## 独立层的构造和双向绑定

`srd_resolve_reference_cast_resource` 找到同文件 SCN/LAYR 后，经 `srd_create_and_bind_reference_layer` (`0xAC27F0`) 进入 `srd_create_independent_reference_layer` (`0xAC2210`)。后者重新分配 runtime layer，并以目标 runtime layer `+0x0C` 保存的 parsed LAYR 调用 `srd_build_runtime_layer`。因此每个 RefCast 都重新建立 CAST、变换和动画状态；它不复用目标 runtime layer 对象。

新层先取得 `_copy%d` 唯一名，并保存：

| runtime layer 偏移 | 已证明关系 |
| --- | --- |
| `+0x244` | 被匹配的原始 runtime layer |
| `+0x250` | 创建时由已有复制层数量得到的索引 |
| `+0x254` | 创建时取得的关系值，绑定阶段会更新 |

`srd_bind_runtime_layer_to_reference_cast` (`0xAC1890`) 随后写入：

| 对象与偏移 | 已证明关系 |
| --- | --- |
| copied runtime layer `+0x24C` | owning RefCast 反向指针 |
| copied runtime layer `+0x250/+0x254` | 传给该层所有 CAST 虚表 `+0x20` 的绑定值 |
| copied runtime layer `+0x130` | owning RefCast 虚表 `+0x30` 返回的 2D 模式 |
| RefCast `+0x1F4` | copied runtime layer 指针 |

绑定器还把同一 2D 模式传给复制层的每个 CAST 虚表 `+0x34`。所以嵌套引用沿最外层 owning CAST 继承模式，而不是重新采用每个目标 parsed LAYR 的 flags。若目标层原本为 2D、owning RefCast 却为 3D，`0xAC18D4..0xAC18D6` 还会把 copied layer `+0x131` 置一，使局部矩阵建立时翻转 Y；其他组合不设置该字节。Rust `ReferenceLayerInstance::is_2d/flip_y` 复现这两条传播链。

## 新复制层的迭代解析

`srd_resolve_reference_scene_links` (`0xAA9450`) 先遍历原始运行时场景表中的所有层和 CAST，调用 CAST 虚表 `+0x0C`。RefCast 解析成功时，新 copied layer 被加入 `ReferenceScene+0x24` 的队列。

随后函数反复：

1. 取出当前队列中的全部 copied layer；
2. 遍历每层 `+0x4C..+0x50` 的 CAST；
3. 再次调用每个 CAST 的虚表 `+0x0C`；
4. 直到新 copied layer 队列为空。

因此目标层内的 RefCast 会继续产生新的独立层实例。同一个目标被两个 RefCast 引用时也会产生两个对象，而不是合并缓存。该循环没有深度上限或目标层去重；若同一目标层沿一条引用链再次出现，新复制层会持续产生，队列不会收敛。Rust 对这种图返回明确错误，不伪造游戏中不存在的截断深度。

## 更新递归

`srd_update_cast_tree` (`0xAC0F80`) 完成普通子 CAST 递归后调用 CAST 虚表 `+0x74`。普通 CAST 返回空，SrRefCast 的 `srd_get_reference_cast_runtime_layer` (`0xADB810`) 返回 `+0x1F4`。存在 copied layer 时，更新器把当前 RefCast 的有效状态传给该层并调用 `srd_update_runtime_layer` (`0xABE710`)。

`srd_update_runtime_layer` 在存在 `+0x24C` owning RefCast 时：

- 以 RefCast `+0x8C` 世界矩阵为父矩阵；
- 将 copied layer `+0x13C` 局部仿射矩阵与父矩阵相乘，结果保存到 layer `+0x16C`；
- `+0x210 = RefCast +0xC4 && layer +0x168`；
- `+0x208 = multiply_color(RefCast +0xBC, layer +0x160)`；
- `+0x20C = saturating_add_color(RefCast +0xC0, layer +0x164)`；
- 再遍历 layer 根 CAST，调用 `srd_update_cast_tree`。

`srd_update_runtime_scene_layers` (`0xAC21A0`) 对普通项目层把 runtime scene `+0x75` 写入 layer `+0x1A4`。对 copied layer，`srd_update_cast_tree` 则把 owning RefCast 的完整 `srd_cast_passes_render_gate` 结果写入 `+0x1A4`。`srd_runtime_layer_passes_render_gate` 对 copied layer 要求 `+0x168`、`+0x1A4` 和 owning RefCast gate 三者都成立。因此 Rust 世界状态分别保存 transform visibility 与 render gate，不把二者合并成一个字节。

## 绘制递归

所有 CAST 的虚表 `+0x18` 进入 `srd_render_cast` (`0xAD45E0`)。该公共包装器检查 CAST gate、更新 renderer `+0x198`，再调用各类型虚表 `+0x1C`。SrRefCast 在该槽进入 `srd_render_reference_cast` (`0xADB7C0`)：

1. 要求 RefCast `+0x1F4` 非空；
2. 要求 `srd_runtime_layer_passes_render_gate` (`0xAC0A50`) 成立；
3. 要求 RefCast 自身公共 render gate 成立；
4. 调用 `srd_render_runtime_layer` (`0xABF840`)。

`srd_render_runtime_layer` 保存 renderer `+0x198`，按顺序遍历 copied layer `+0x4C..+0x50` 的 CAST 并调用各自虚表 `+0x18`，最后恢复 `+0x198`。因此嵌套 RefCast 在其 CAST 位置立即递归绘制目标层，而调用者的排序状态跨层保持。

## 当前实现边界

Rust 已实现：

- 原始项目层优先、复制层随后展开的运行时建立顺序；
- 每个 RefCast 一个独立 `ReferenceLayerInstance`；
- 父项目层或父引用实例、引用 NODE 下标和目标 SCN/LAYR 的对应关系；
- owning RefCast 2D 模式向所有嵌套复制层传播；
- 2D 目标嵌入 3D RefCast 时的 Y 翻转；
- copied layer 的父矩阵、乘色、加色、transform visibility 和 render gate 组合；
- 每个 copied layer 独立的 CAST transform、ANIM frame/duration/flags，以及首次同名动画的公共通道立即应用；
- 未解析引用不创建层，以及无截断猜测的循环诊断。

53 个本地 SRD 的 1090 个静态 CRFD 按上述过程展开为 2087 个独立 runtime reference layer；按文件统计共有 186 个目标 SCN/LAYR 被两个或更多实例引用。全部文件均收敛且没有未解析目标。这组结果也排除了“按目标层共享一个运行时对象”作为语料兼容实现。

后续仍需把 CAST 类型专属动画通道全部接入 `ReferenceLayerRuntimeState`，实现源目标层对根 CAST 的附加抑制条件，以及最终 D3D9 draw submission。本页不把已完成的公共动画和世界状态组合表述为已经完成像素渲染。
