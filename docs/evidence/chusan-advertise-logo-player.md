# Chusan `AdvertiseLogoObject` 的 SrPlayer 宿主状态

本文只记录 `chusanApp.exe` 中 `projView::AdvertiseLogoObject` 实际创建并初始化的嵌入式 `projView::SrPlayer`。结论来自当前游戏二进制，不使用旧编辑器文档推导。

## 对象与成员位置

外层 `AdvertiseLogoObject` 构造函数 `sub_C94580` 分配 `0x450` 字节的 Impl，并由 `sub_C94630` 构造。Impl 构造在 `0xC9466F..0xC9467F` 对 `Impl+0x68` 调用 `sub_BA5080`，所以 SrPlayer 的位置是精确的 `Impl+0x68`。

加载准备 `sub_C959D0` 把：

- 完整 SRD 路径写入 `Impl+0x1A8`，即 SrPlayer `+0x140`；
- surfboard texture 根路径写入 `Impl+0x1C0`，即 SrPlayer `+0x158`；
- 随后在 `0xC95A71..0xC95A76` 对 `Impl+0x68` 调用 `SrPlayer::load(0)`。

文件名由 `sub_112F0A0` 读取运行时枚举表 `0x1CD7740`。目前没有把本地 fixture 名称冒充成该表的已证明结果。

## common init 的属性写入

`srd_construct_player` (`0xAA68B0`) 注册的相关属性为：

| 编号 | 属性 | 构造默认值 |
| ---: | --- | ---: |
| 0 | `Enable` | true |
| 1 | `DrawTargetSceneOnly` | false |
| 2 | `TargetScene` | 空字符串 |
| 3 | `FirstCalcMatrix` | false |
| 4 | `DrawMask` | `0xFFFF` |
| 6 | `2DLayer` | 5 |

Advertise common init `sub_C955D0` 的前三个调用可逐条解释：

1. `sub_414376 -> sub_BA6490` 把 property 1 写为 true；
2. `0xC95605..0xC9560F` 对 SrPlayer 属性容器写 property 6 = 100；
3. `sub_45B60E -> sub_BA7550` 经 `surfride::SrCtrl` 重置运行时 scene/layer，并最终把 property 0 `Enable` 写为 false。

这条路径没有写 property 2、3 或 4。因此 common init 结束时：

- `DrawTargetSceneOnly=true`；
- `TargetScene` 仍为空；
- `FirstCalcMatrix=false`；
- `DrawMask=0xFFFF`；
- `2DLayer=100`；
- `Enable=false`。

后续状态机何时再次启用 player、选择哪段动画，仍须按具体状态调用闭环；本 profile 不自行指定启动动画。

## scene-node 父矩阵

SrPlayer 构造首先调用 `sub_6EDDA0 -> sub_6096C0`，其基类是 `sea::GraphNode`。`sub_6095B0` 明确把父节点指针 `GraphNode+0x1C` 清零；`sub_6096C0` 又把本地矩阵 `+0x64` 和组合矩阵 `+0x94` 都初始化为 3x4 identity。

`sea_node_get_first_calc_affine_matrix` (`0x60A230`) 的行为是：

- 有父节点时计算 `parent composite * local` 并写回 `+0x94`；
- property `FirstCalcMatrix=false` 时返回组合矩阵 `+0x94`；
- property 为 true 时返回本地矩阵 `+0x64`。

GraphNode 的设父函数是 `sub_609F30`，位于 SrPlayer 主虚表 `+0x34`。对 `AdvertiseLogoObject` 的完整 Impl/外层函数范围 `0xC94390..0xC96B00` 检查后，所有形成 `Impl+0x68` 的调用点只有构造、析构、load、common init/reset、ready 检查和 CAST 操作；没有调用虚表 `+0x34`，也没有直接写 SrPlayer `+0x1C`。

Impl `+0x438` 的 `projView::GameObjectBase` 保存 SrPlayer 指针并执行延迟命令。其执行器 `sub_C58BA0` 的 command 0..10 只调用 SrPlayer/SrCtrl 的动画帧、运行时 scene/layer、CAST 可见性和 CAST 参数接口；没有 GraphNode 设父或矩阵写入。`ObjectManager` 的实际生命周期 `sub_B2D580`、`sub_B2DE90`、`sub_B2E240` 只创建、更新和销毁外层对象，也没有额外的 scene-node 附着。

因此这一个具体宿主对象的 SrPlayer 始终没有父节点，组合矩阵保持构造 identity。这里的 identity 是二进制调用链结论，不是编辑器为了看起来正确而采用的默认值。

## 仍然显式的 target 边界

空 `TargetScene` 不会 fallback 到某个命名 target；packet 进入全局队列，随后由当帧已注册且启用的 target 各自过滤。`MainScene` 与 `BgScene` 的创建和 Camera 见 [`chusan-air-scene-profiles.md`](chusan-air-scene-profiles.md)，完整路由见 [`render-target-routing.md`](render-target-routing.md)。

所以 Rust profile 可以证明提供 identity `FirstCalcMatrix`，但仍要求调用者显式选择模拟的 target 和 present 宽高。它不会自动宣称 AdvertiseLogo 只由 `MainScene` 接收，也不会猜测固定分辨率。
