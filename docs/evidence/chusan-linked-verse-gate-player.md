# Chusan `PlayLinkedVerseGateObject` 的 SrPlayer 宿主状态

本文只记录 `chusanApp.exe` 中 `projView::PlayLinkedVerseGateObject` 的嵌入式
`projView::SrPlayer`，以及本地完整游戏数据中与其资源枚举相连的记录。旧编辑器和
文件名不作为行为推导来源。

## 对象与资源加载链

RTTI complete-object locator `0x1A2AA04` 的唯一字节引用位于 `0x1922224`，所以
外层 vtable 从 `0x1922228` 开始。该 vtable 的构造写入点 `sub_D904C0`
(`0xD904FC`) 分配 `0x328` 字节 Impl。Impl 构造 `sub_D901D0` 在
`0xD90206..0xD90216` 对 `Impl+0x68` 调用 `sub_465E51 -> sub_BA5080`，因此
嵌入式 SrPlayer 的偏移精确为 `+0x68`。

外层 load 位于 `0xD92300..0xD92370`。它经 `sub_4055A6` 调用 Impl helper
`sub_D921F0`；后者在 `0xD9221E` 把资源枚举值写为 `0x54`，再经
`sub_417878 -> sub_112F0A0` 查询进程级 SurfFile table，最后把 surfboard 根路径、
返回的相对路径和 texture 根路径交给 SrPlayer load。

本地 `db/SurfFileTableRecord.bin` 是 91 条记录的精确长度前缀流：文件头为 u32
记录数，每条依次为 u32 id、name/description/path 三个 u32 长度加原始字节。完整
解析恰好消费 9170 字节，无尾部数据。索引和 id 都为 84 的记录是：

```text
name = LinkedVerseGate
path = play/linkedVerse/CHU_UI_LinkedVERSE_Gate_00.srd
```

因此 load helper 的 `0x54` 与目标 SRD 的关系来自游戏代码和游戏数据表的直接连接，
不是根据类名猜测。

## SrPlayer 属性

初始化片段 `0xD92160..0xD92188`，以及实际重试状态路径 `sub_D93A60`
(`0xD93A86..0xD93AAC`)，执行相同序列：

1. 对 `Impl+0x68` 调用 `sub_414376 -> sub_BA6490` 的共用 SrPlayer 初始化；
2. 以 property id 6、值 `0x46` 调用属性容器 setter；
3. 继续外层状态回调。

property 6 已由 `srd_construct_player` 闭环为 `2DLayer`，所以本类是十进制 70。
共用初始化设置 `DrawTargetSceneOnly=true`。对完整类模块
`0xD900C0..0xD9402A` 的 rendered-listing 搜索只有 `0xD9216F` 和
`0xD93A95` 两次形成 SrPlayer 属性容器 `Impl+0x158`，两次都只写 property 6；没有
其他属性写入。因此 `TargetScene`、`FirstCalcMatrix` 与 `DrawMask` 保持构造/共用
初始化后的空字符串、false、`0xFFFF`。

`2DLayer=70` 经 `sub_AAD040` 写入 renderer key bits `8..14`，所以根 key 是：

```text
kind bit 15 = 1
2DLayer     = 70 (0x46)
level       = 0x80
root key    = 0x0000C680
```

该 key 只控制提交层级/相邻合并，不会把 SRD 内的 3D LAYR 改成 2D。

## 根矩阵与 target 路由

SrPlayer 的 GraphNode 构造把 parent 清零并建立 identity local/composite matrix。完整
类模块没有调用 GraphNode 设父 thunk `sub_41CC88 -> sub_609F30`，也没有直接写
`Impl+0x84`（SrPlayer `+0x1C` parent）。所以当前闭环的 FirstCalc 输入是 identity。

空 `TargetScene` 继续走已经证明的精确 null lookup/global queue 路径；它不会自动
绑定 MainScene。以构造完成状态过滤时，`DrawMask=0xFFFF` 使普通 SRD packet 被
MainScene 接纳、被 BgScene 拒绝。

## 运行时 Camera 边界

真实 `ANMS[10] frame 1 / LAYR[0]/NODE[149]` 在上述 null-target 宿主下仍产生 498
个 3D Fennel 顶点；TextBox packet `c0..c3` 是 world 矩阵与完整 Y 列翻转：

```text
3D87965E 80000000 00000000 3FB1F560
00000000 BD87965E 00000000 C1BCF414
00000000 80000000 3F800000 00000000
00000000 80000000 00000000 3F800000
```

这修正了先前用“命名 identity target”诊断 host 得到的矩阵；真实空 target 不会把
SRD ProjectionView 乘入 TextBox 矩阵。

但把 Chusan `MainScene` 的构造时 Camera `(0,0,30)` 当成接收 target 时，这批顶点
投影到约 `(1485,1625)..(3057,1882)`，完全落在 1920x1080 画面下方，D3D9Ex
Fennel 差分为 0。用户提供的 Common background 实机对照同样证明构造时 Camera
会把中央内容放大并裁掉四周。由此可确定剩余缺口是 MainScene 后续运行时 Camera /
ProjectionView 写入与画面切换时序；在该链闭合前，不得把构造相机像素结果称为游戏
最终画面。

