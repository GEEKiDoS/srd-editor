# SrRenderer 的 CAST 可见性矩形与空名称未决点

本文记录 `sub_AC6660 -> sub_1049940` 的完整 ImageCast 可见性测试，并明确区分“property 2 字符串为空”和“target 查找结果已证明为 null”。两者目前不能等同。

## 已闭合的矩形算法

`srd_renderer_configure_project_camera` (`0xAC7400`) 在 target pointer 与 SrProject 都非空时，从 target Width/Height 构造：

```text
SrRenderer+0x24C = width  * 0.5   // center X
SrRenderer+0x250 = height * 0.5   // center Y
SrRenderer+0x254 = width  * 0.5   // half width
SrRenderer+0x258 = height * 0.5   // half height
```

ImageCast、SliceCast、NumberCast 和 Fennel 路径均经 thunk `sub_469227` 调用 `sub_AC6660`。二维 CAST 直接取四个 world corner 的 X/Y；三维 CAST 使用 `SrRenderer+0x48` 的第 0、1、3 行做 homogeneous X/Y/W 与除法。四点以 SSE `MINSS/MAXSS` 的实际 operand 顺序得到 min/max，再构造中心与半尺寸。

`sub_1049940` (`0x1049940`) 是 inclusive center/half AABB 测试：

```text
abs(cx1-cx2) <= halfW1+halfW2
abs(cy1-cy2) <= halfH1+halfH2
```

`sub_AC6660` 对相交返回 0、对不相交返回 1；各 CAST renderer 因此在非零时跳过 draw。Rust 已在命名 target 明确解析成功的 ImageCast 路径复现四角投影、SSE 边界、NaN/有符号零比较和 inclusive AABB 剔除。

## 空名称为什么仍未闭环

`srd_construct_renderer` (`0xAC4010`) 只初始化 `+0x248` 和 `+0x25C/+0x25D`，没有初始化 `+0x24C..+0x258`。`surfride::SrPlayer::Impl` 的分配链最终是 `_aligned_malloc(size, 8)`，也没有清零这四个 f32。对 Surfride `0xAA0000..0xAF0000` 的完整 rendered-listing 搜索只发现 `0xAC7400` 写 renderer 的这组矩形字段。

与此同时，`sub_AC6660` 在每个 ImageCast 前无条件读取该矩形。若 property 2 的空字符串查找真的返回 null，稳定渲染将依赖未初始化堆内存；这与实际设计矛盾。因此“查找没有 fallback”只能证明它执行精确 map lookup，不能证明 map 中不存在空字符串 key。

注册链 `sub_43BB2E -> sub_602210 -> sub_46524E -> sub_6310E0 -> sub_673F10` 使用 target 虚表 `+0x38` 返回的运行时名称作为 key，且没有拒绝空名称。18 个注册调用点中，固定名称与字符串拼接名称已经排除；`sub_769D10` 属于调试 `Scene List` 的 Create 回调。剩余 `star::SglScene` (`0x12B68E0`) 从 AFB 运行时描述符取得名称并注册，描述符实例值仍需从 AFB loader/数据闭环。

因此当前严格结论是：

- 命名 target 成功解析时，矩阵与可见性剔除已闭环并实现；
- 若查找结果为 null，`+0x08/+0x48` 的 identity 短路是条件上精确的；
- Chusan Advertise/Common 的 property 2 字符串确实为空；
- 但该空字符串最终解析为 null 还是空键 target，尚无完整证据。

在空键注册闭环前，Rust 的 `renderer_project_target=None` 只能作为显式的 null-branch 诊断输入，不再宣称是 Advertise/Common 的最终原游戏查找结果；该路径也不会伪造 `+0x24C..+0x258` 可见性矩形。
