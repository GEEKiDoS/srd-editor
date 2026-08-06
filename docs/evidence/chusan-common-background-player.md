# Chusan `CommonBackGroundObject` 的 SrPlayer 宿主状态

本文只记录 `chusanApp.exe` 中 `CommonBackGroundObject` 实际持有的 `projView::SrPlayer`。旧编辑器和文件名只用于定位样本，不作为行为证据。

## 对象布局与初始化

外层对象为 12 字节句柄，持有 496 字节 Impl；嵌入式 SrPlayer 位于 `Impl+0x68`。资源初始化回调 `sub_B5B450` 在 `0xB5B477..0xB5B496` 对该成员执行三项确定操作：

1. `sub_414376 -> sub_BA6490` 执行所有同类 SrPlayer 共用的资源初始化；
2. 对属性容器写 property 6 = 6；
3. `sub_45B60E` 重置并关闭 player。

`srd_construct_player` (`0xAA68B0`) 证明 property 6 名为 `2DLayer`，构造默认值为 5，合法域为 `0..127`。共用初始化还把 `DrawTargetSceneOnly` 写为 true；本类范围没有写 `TargetScene`、`FirstCalcMatrix` 或 `DrawMask`，所以初始化结束时它们分别保持空字符串、false 和 `0xFFFF`。

`sub_BA7B00` 是后续模式切换使用的显式 Enable 入口：先把参数传给嵌入式控制器，再经 SrPlayer 虚表 `+0x30` 同步 Enable。这里把“资源初始化结束时关闭”和“模式切换后可开启”分开记录，编辑器不会把两者压成一个猜测的永久状态。

## 根矩阵与对象归属

SrPlayer 的 GraphNode 构造把 parent 清零，并把 local/composite 3x4 矩阵初始化为 identity。对 Common 类的构造、加载、状态、析构及 object-manager 生命周期范围审计，没有发现：

- 调用 GraphNode 设父入口；
- 直接写 SrPlayer parent；
- 直接写 local/composite matrix；
- 把它嵌入某个具体画面对象后再保存为父子节点。

对象以精确名称 `CommonBackGroundObject` 注册到全局 object manager，其他画面按该名称查找；它不是被保存进每个引用该背景的 SRD 画面对象中的成员。因此当前已证明的 `FirstCalcMatrix` 输入是构造 identity，而不是编辑器为了扩大画面加入的补偿矩阵。

调用点也已闭合：`srd_player_impl_prepare_renderer` (`0xAACA80`) 在 `0xAACB05..0xAACB19` 通过 SrPlayer 虚表 `+0x40` 取得 `sea_node_get_first_calc_affine_matrix` (`0x60A230`) 的返回指针，并原样传给 `srd_renderer_configure_project_camera`。SrPlayer `+0xC4` 只是 property 3 的选择 byte；false 返回 composite `+0x94`，true 才返回 local `+0x64`。Common 为 false、parent 为空，因此这里不存在尚未建模的 `+0xC4` 矩阵或内部横向缩放。

## `2DLayer=6` 到 renderer key

`sub_AAD040` (`0xAAD040`) 对 property 6 的同步直接读写 `SrPlayer::Impl+0x1A8` bits `8..14`。`srd_player_get_renderer` 返回 `Impl+0x10`，所以该地址同时就是嵌入式 `SrRenderer+0x198`，中间不存在复制层。

`ceylon_construct_draw_packet` (`0x6B8B10`) 在 `0x6B8B31..0x6B8B53` 建立默认 key `0x00008580`。property 6 只替换 bits `8..14`，因此 Common 的根 key 精确为：

```text
kind bit 15 = 1
2DLayer     = 6
level       = 0x80
root key    = 0x00008680
```

`srd_update_runtime_scene_layers` (`0xAC21A0`) 把 `SrRenderer+0x198` 地址直接作为顶层继承 key 传入 CAST 树；`srd_render_cast` (`0xAD45E0`) 再把 CAST key 写回该字段并应用 NODE `0xA0` 的低字节偏移。由此可见 `2DLayer` 控制 packet order/相邻合并键，不会把 Common 的 LAYR/CAST 从 3D 改成 2D。

## target 路由

Common 与 Advertise 一样满足：

- `DrawTargetSceneOnly=true`；
- `TargetScene` 字符串为空；
- `DrawMask=0xFFFF`。

空名称执行精确 map lookup。Scene 名称 map 构造为空，唯一插入入口的注册点均已审计，本地完整 AFB 也不存在 `star::SglScene` 类型记录，因此结果为 null；packet 进入全局队列，再由每个已注册 target 过滤。Chusan MainScene/BgScene 构造完成状态使 Common 的普通 packet 被 MainScene 接纳、被 BgScene 拒绝。原二进制 null 分支读取未初始化可见性矩形的边界见 [`render-visibility-culling.md`](render-visibility-culling.md)。

## 实际样本回归

对 `CHU_UI_Common_BK_00_v11.srd` 的 `yellow_loop` 第 0 帧，使用 Common/MainScene 的已证明 null lookup：

- 产生 100 个普通 runtime draw；
- 100 个 draw 的 renderer key 全部为 `0x8680`；
- 按 `ceylon_enqueue_draw_packet` 的精确相邻比较与 triangle-strip 退化连接规则合并为 9 个 record；
- 100 个逻辑 source 全部仍可从 9 个 record 反查，不发生丢失或跨 key 合并。

该已闭环的 null lookup 路径中，`srd_renderer_configure_project_camera` 不进入 Width/Height/Camera 分支，`SrRenderer+0x08/+0x48` 保持构造 identity；三维 ImageCast 把 world XYZ 写入顶点，packet `c0..c3` 使用该 identity，最终 MainScene Camera 只从 target-local ShapeEnv 写入 `c10..c13`。D3D9Ex smoke 覆盖完整 `1920x1080`，ResetEx 前后哈希一致。原二进制随后读取未初始化可见性矩形；Rust 预览在该边界跳过剔除，不伪造默认矩形。
