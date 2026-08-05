# 待闭环问题

本页只记录已经有明确二进制边界、但证据尚未闭合的问题。这里的候选解释不得直接进入渲染实现。

## `CHU_UI_Common_BK_00_v11.srd` 横向投影异常

回归样本：

```text
D:\sdhd\assets\data\surfboard\common\commonBackGround\CHU_UI_Common_BK_00_v11.srd
```

已证明：

- SCN 为 `1920x1080`，CAM 为 position `(0,0,1000)`、target `(0,0,0)`、约 `45°`、near `10`、far `100000`。
- 七个 LAYR 的 flags 为 `0x101` 或 `0x1`；`srd_build_runtime_layer`、CAST 初始化和 `srd_cast_is_2d` 的完整链均把它们判为 3D。
- `srd_renderer_configure_project_camera` (`0xAC7400`) 把 target 的整数 Width 原样传给 `sea_camera_set_perspective_parameters` (`0x656450`) 的 Aspect 属性；`sea_camera_build_projection_matrix` (`0x655630`) 与 `srd_build_perspective_fov_rh` (`0x6B3CD0`) 后续没有再除以 Height。
- `srd_set_srimage_size_and_origin` (`0xAD2EB0`) 只复制宽高并按九种 origin 系数计算原点，没有屏幕补偿。
- ImageCast 顶点在提交前只乘 CAST world matrix。CommonBackGroundObject 以全局名称注册/查找；完整类与 object-manager 生命周期审计没有 GraphNode parent setter，也没有写嵌入式 SrPlayer 的 parent/local/composite matrix，因此 FirstCalc 输入已闭环为构造 identity。
- Common 初始化精确写入 `DrawTargetSceneOnly=true`、空 `TargetScene`、`DrawMask=0xFFFF`、`2DLayer=6`，根 renderer key 为 `0x8680`；MainScene 初始接纳、BgScene 初始拒绝。`yellow_loop` 第 0 帧 100 个普通 draw 全部为该 key，并按原版相邻规则合并成 9 个 record。
- 在 identity FirstCalc、`1920x1080` 诊断 viewport 下，完整背景 quad `(-960,-540)..(960,540)` 的 Y 基本覆盖全高，但 X 只覆盖约两像素。异常恰好由 Width-as-Aspect 引起，不是纹理尺寸或 origin 引起。

尚未证明：

- 游戏模式切换之后 MainScene/BgScene Enable、manager current-target，以及可能存在的专用 offscreen target 的当帧组合时序。当前只外推到已证明的构造完成状态。
- 游戏最终 target rotation/offscreen 合成是否在 camera/vertex shader 之后提供额外矩阵；当前证据只证明 target Camera 的 `Projection*View` provider。
- 原版运行时该样本的最终 GPU 常量与 viewport。SrPlayer parent、local/composite matrix、`FirstCalcMatrix` 选择位及其 renderer 调用点已闭环为 null/identity/false，不再列为未知输入。

恢复调查时应继续追 target rotation/offscreen 合成路径，或取得原版运行时 viewport、最终 VS 常量和 SrPlayer 矩阵捕获。禁止以 `Aspect = Width / Height`、自动 fit-to-view、强制 2D 或任意 X scale 作为游戏逻辑修复。

## Fennel 行元数据与剩余 effect/crop 输入

静态 `sub_7C1F90` 的自动断行、两张固定表、空格候选、二次纵向 pass、独立 TextBox `+0x108`、`-254` 标记、fresh mode 1 的 `0x08` X/Y 联动 auto-fit、fresh mode 5/6 的 `0x4000` 垂直截止关闭、flags `0x200` 的固定 cell 度量/尾部 X 居中修正、`sub_7C90A0` 尾部 record-limit 返回值，以及 TextBoxObject `+0x12C/+0x34C` 的锁步行元数据已经实现；`sub_7C0D40` 的负记录过滤、`-254` 立即停止、静态 maximum=-1、texture token 分组、normal/effect 计数和初始 17 桶前向链顺序也已实现；`sub_7C7F90` normal/effect glyph 的 bearing origin、effective scale、2D matrix 顺序，以及 `sub_7C10B0` 的无裁剪/裁剪 UV 重映射两套顶点同样已实现。CATR `FontParamData` 的顺序解析、实际 mode、低位 flags、monospaced、clip 与 shadow 已接入；完整 1292 条 RFZ TEXT 的实际 mode 为 `0:1173, 2:12, 4:107`，真实语料有 551 个初始可见 2D draw、34326 个顶点，Advertise 像素回归仍为 40920 个 changed pixels。

尚未闭合：

- FontResource 的 32 槽 lowest-free 分配、同资源缓存复用、最后引用释放后的精确槽回收、满表返回未注册 slot `0x20`，以及当前玩家 `SCN -> LAYR -> NODE` 中主字体后接 `rubyFont/rfzOutlineFont/rfzOutlineRubyFont` 的 CATR 请求顺序已经闭合并实现；copied reference layer 已证明不会追加请求，而是使用原始目标层已请求的共享资源。`$F[n]`/裸 `$F` 的全局 slot 解析与跨字体 atlas token 路由也已连接。仍待宿主提供的是加载当前 SRD 前仍存活的进程级 FontManager/renderer 资源状态；不得把“空 registry”或编辑器自行分配的 opaque texture handle 声称为任意原版进程的绝对映射；
- FontParam 初始 RFZ style 链已闭合：`pointX/pointY` 经 signed `max(value,1)` 和高字节清零成为 u8，但 `FontDriverRFO` 查找只读取字符码并只复制 style flags；outline/italic/bold 的 flags/value 也已复现，`faceId` 不被 `sub_AC6F50` 读取。`$[0]..$[7]` 顺序替换、`$D/$L` 大写优先/首个删除/可选括号 `atoi`、FontManager 默认 D=20 与 repeat-space count=3、`sub_AD8D00` 时钟、`sub_AC5740` 派生量、`sub_7BFAB0` mode-0 实际几何测量及完整 `sub_7C04F0` 横纵循环/fit-guard/二次排版均已实现，draw-list 已按最终 TextBox 内容接线，并有显式 per-node runtime API 接收替换槽/default/repeat/F4；动画集版本也复用 ProjectRuntime 的 layer enable、CAST transform/color 与 SrImage geometry。project layer/Cast-vector/RefCast 的结构递归顺序、完整世界/gate、copied ImageCast/TextCast 独立 draw 和两者混合的 runtime CAST 调用序列均已接通；copied 文本输入用 `(owner,node)` 作为 key，重复引用实例不会共享替换槽或 F4。target queue 的 type-1/SRD class、首个 rule 匹配、per-pass 稳定 vector、32-entry inclusive-range flush 和无 comparator 结论也已闭环并实现；MainScene/BgScene 共用的 5 项默认 BasePass 与 EntryInfo 映射也已闭环。普通 Image/Fennel packet 的 `flags_60 bit 0x2000` 已排除、`packet+0x64` 已闭环为零，默认 Back2DPass 对完整 u16 order 域恒真；逻辑 stream 现按一个 Image command/每个 Fennel texture-batch command 展平，并能在显式 MainScene/BgScene profile 下生成精确 target-local 顺序。type-1 target filter、Scene 初始 Enable/Attribute/DrawIndex 也已实现；Advertise/Common 构造完成状态均精确为 Main 接纳、Bg 拒绝，宿主根 key、CATR/NODE/RefCast 合成及普通相邻 packet 合并也已接线。尚缺后续 Enable/current-target 切换时序、其他 target、真正依赖 depth/order 的其他路径及 stencil/special-depth 合并，所以不会把该顺序冒充所有运行阶段的完整 Composition GPU packet 列表。右侧 Properties 已保存 project-layer 手动输入，并要求显式选择 Advertise/Common 宿主；initial API 碰到替换 token 会显式拒绝。仍未闭合的二进制行为是 `vertical=True` correction，以及 mode 6 非虚方法 `0xADA300..0xADA348` 的真实调用点；后者仍只有无调用引用的 jump-island thunk `0x45335F`，不得假定 SRD 首帧或动画会自动触发它；
- `sub_7C90A0` 的布局分派已闭合为 `0x20 -> sub_7C4070`、否则 `0x40 -> sub_7C5A20`、否则默认。fresh mode `1` 的 `0x000F` 默认排版、mode `2..4` 的 `0x0CA3/0x1CA3/0x2CA3` Flag20 排版、fresh mode `5/6` 的 `0x6C03/0x7C03` 默认排版，以及三个布局器共用的 flags `0x200` 固定 cell 分支均已实现。Flag20 包括相同的断行/截止/元数据状态机和 `+0x358` 严格最小 advance 下标。完整指令流已证明 `sub_7C5A20` 与 Flag20 的有效差异只有后者的 `+0x358` 初始化/两次更新，但组件内直接字段与 setter 审计仍找不到任何生成 `0x40` 的入口；因此 Flag40 的真实 flags 来源仍待闭合；
- `sub_7C8BE0` 的 hash rehash；当前完整字体最多 7 页、真实文本最多 6 个 batch，不会触发该分支。

在这些输入闭环前，不得自行补 vertical correction、滚动几何测量/宿主输入、mode-6 自动调用、`Flag40` 或 rehash 行为。
