# CAST special matrix flags

本页只记录游戏二进制中已经闭环的 `NODE type_flags & 0x00070000`
分支。旧实现曾在一个 layer 内发现任意特殊矩阵节点时拒绝整个 layer；完整语料证明
这会丢掉大量真实内容，因此不能继续作为保守边界。

## 二进制调用链

`srd_update_cast_tree` (`0xAC0F80`) 把 parent CAST world、local TRS、NODE
flags、runtime layer mode 和 `SrRenderer` 传给
`srd_compose_cast_world_state` (`0xABF350`)。后者首先仍按普通顺序计算：

```text
W = P * L
```

若 `flags & 0x00070000 != 0`，函数保存 `W` 的三个 translation 分量，随后把
`P` 扩展为 4x4，经 `ceylon_inverse_matrix4x4` (`0x6B4170`) 求逆，再取回前三行。
这条 parent inverse 路径不能与后述 `SrRenderer+0x88` 的 3x4 inverse 混用；两者
的浮点顺序和奇异矩阵回退不同。

runtime layer `+0x130 != 0` 时，矩阵分支严格为：

```text
W = inverse(P) * W

if flags & 0x01000000:
    sx = sqrt((P00*P00 + P10*P10) + P20*P20)
    sy = sqrt((P01*P01 + P11*P11) + P21*P21)
    W = diag(sx, sy, 1) * W

W = inverse_srd_camera_view * W
```

runtime layer `+0x130 == 0` 时，`flags & 0x00070000` 的 switch 为：

```text
0x20000: W = W * inverse(P)
0x30000: W 不再修改
0x40000: W 不再修改
0x50000: W 不再修改
0x10000,
0x60000,
0x70000: W = inverse_srd_camera_view * (inverse(P) * W)
```

上述分支结束后，三个 translation 分量全部恢复为最初 `P * L` 的结果。因此特殊
矩阵只替换 basis；位置继续沿普通父子组合传播。

CAST 子节点递归完成后，`srd_update_cast_tree` 还执行一个独立条件：

```text
(flags & 0x01000000) != 0
&& (flags & 0x00070000) == 0x00010000
```

命中时仅把当前 CAST world Z translation 清零。它发生在子节点更新之后，所以子
节点仍然继承清零前的 parent Z；不能提前修改 parent matrix。

## `inverse_srd_camera_view` 来源

`srd_construct_renderer` (`0xAC4010`) 把 `SrRenderer+0x88` 和 `+0xB8` 两个
3x4 matrix 分别初始化为 identity。`srd_renderer_configure_project_camera`
(`0xAC7400`) 在命名 `TargetScene` 成功解析后：

1. 从为 SRD CAM 配置的 Camera 虚表 `+0xE8` 取得 View 3x4；
2. 复制到 `SrRenderer+0x88`；
3. 原地调用 `sub_60B2A0` 的 3x4 cofactor inverse。

因此特殊矩阵读取的是 SRD Camera View 的逆矩阵。若 `TargetScene` lookup 为 null，
配置函数在写入前返回，`SrRenderer+0x88` 精确保留 constructor identity。Rust
host context 已按这两个已证明分支选择矩阵，没有用接收全局 packet 的 target
Camera 代替它。

## 完整语料覆盖

`srd-runtime-state-audit` 对本地 91 个 SRD 报告：

```text
special matrix layers: 101
flagged nodes: 1226
matrix kinds: {0x10000: 1226}
modifier 0x01000000: 134（全部位于 2D runtime layer）
affected layer modes: 2D=95, 3D=6
```

这些 layer 合计包含 10,369 个 CAST，其中 6,865 个是 Image。实现前，一处特殊
节点会导致整层被跳过。实现后的完整初始 runtime audit 产出 21,136 个
Image/Slice/Number draw；其中 5,124 个来自特殊矩阵 layer，1,155 个直接来自特殊
矩阵节点。

Fennel 的非引用初始语料基线也从 551 draw / 34,326 vertices 增至
557 draw / 35,466 vertices；包含 RefCast copied layer 后从 1,155 draw /
54,504 vertices 增至 1,228 draw / 58,488 vertices。旧的独立 Fennel builder
现已复用同一 runtime world composition，避免再次形成“引用路径支持、普通路径整层
跳过”的分叉。

首个命中样本
`avatarCutin/CHU_UI_Entry_AvatarCutIn_00.srd` 含一个 2D 和一个 3D 特殊 layer。
其初始 runtime 现在产出 64 个 draw，其中 55 个来自这两个过去被拒绝的 layer，
21 个直接来自 flagged node。

Rust 对应实现位于 `Affine3x4::inverse_game`（只用于 `SrRenderer+0x88` 的
3x4 inverse）、`inverse_affine_via_matrix4x4_game`（parent 4x4 inverse）和
`compose_cast_matrix_game`。递归函数在访问子节点后才执行当前节点的 Z 清零。
