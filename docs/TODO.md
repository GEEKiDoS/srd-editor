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
- ImageCast 顶点在提交前只乘 CAST world matrix；当前已定位的 CommonBackGroundObject 构造、加载、状态函数没有写嵌入式 SrPlayer 的 local/composite matrix，也没有直接调用 GraphNode parent setter。
- 在 identity FirstCalc、`1920x1080` 诊断 viewport 下，完整背景 quad `(-960,-540)..(960,540)` 的 Y 基本覆盖全高，但 X 只覆盖约两像素。异常恰好由 Width-as-Aspect 引起，不是纹理尺寸或 origin 引起。

尚未证明：

- CommonBackGroundObject 与承载其他 SRD 的具体画面对象之间，是否存在后续建立的 GraphNode 父子关系或横向补偿 FirstCalcMatrix。
- Common background 作为其他 SRD 下层画面时，实际进入 MainScene、BgScene 或专用 offscreen/pass 的哪一条当帧组合路径。
- 游戏最终 target rotation/offscreen 合成是否在 camera/vertex shader 之后提供额外矩阵；当前证据只证明 target Camera 的 `Projection*View` provider。
- 原版运行时该样本的最终 GPU 常量、viewport 和 SrPlayer `+0x1C/+0x64/+0x94/+0xC4` 实值。

恢复调查时必须从保存了 CommonBackGroundObject 指针的具体画面类继续追踪其后续成员访问，或取得原版运行时矩阵/常量捕获。禁止以 `Aspect = Width / Height`、自动 fit-to-view、强制 2D 或任意 X scale 作为游戏逻辑修复。

## Fennel 行元数据与剩余 effect/crop 输入

静态 `sub_7C1F90` 的自动断行、两张固定表、空格候选、二次纵向 pass、独立 TextBox `+0x108`、`-254` 标记、fresh mode 1 的 `0x08` X/Y 联动 auto-fit、fresh mode 5/6 的 `0x4000` 垂直截止关闭、flags `0x200` 的固定 cell 度量/尾部 X 居中修正、`sub_7C90A0` 尾部 record-limit 返回值，以及 TextBoxObject `+0x12C/+0x34C` 的锁步行元数据已经实现；`sub_7C0D40` 的负记录过滤、`-254` 立即停止、静态 maximum=-1、texture token 分组、normal/effect 计数和初始 17 桶前向链顺序也已实现；`sub_7C7F90` normal/effect glyph 的 bearing origin、effective scale、2D matrix 顺序，以及 `sub_7C10B0` 的无裁剪/裁剪 UV 重映射两套顶点同样已实现。CATR `FontParamData` 的顺序解析、实际 mode、低位 flags、monospaced、clip 与 shadow 已接入；完整 1292 条 RFZ TEXT 的实际 mode 为 `0:1173, 2:12, 4:107`，真实语料有 551 个初始可见 2D draw、34326 个顶点，Advertise 像素回归仍为 40920 个 changed pixels。

尚未闭合：

- FontResource 的 32 槽 lowest-free 分配、同资源缓存复用、最后引用释放后的精确槽回收、满表返回未注册 slot `0x20`，以及当前玩家 `SCN -> LAYR -> NODE` 中主字体后接 `rubyFont/rfzOutlineFont/rfzOutlineRubyFont` 的 CATR 请求顺序已经闭合并实现；copied reference layer 已证明不会追加请求，而是使用原始目标层已请求的共享资源。`$F[n]`/裸 `$F` 的全局 slot 解析与跨字体 atlas token 路由也已连接。仍待宿主提供的是加载当前 SRD 前仍存活的进程级 FontManager/renderer 资源状态；不得把“空 registry”或编辑器自行分配的 opaque texture handle 声称为任意原版进程的绝对映射；
- FontParam `pointX/pointY`（完整语料有 14 个非默认 text）、outline/italic/bold/faceId/scroll 等 style 字段从 FontObject `+0x50..+0x64` 到最终 glyph resource/度量的完整下游；`vertical=True` 在当前完整 text 语料为 0，未使用的 correction 分支仍不得泛化。mode 6 非虚方法 `0xADA300..0xADA348` 仍只有无调用引用的 jump-island thunk `0x45335F`，不得假定 SRD 首帧或动画会自动触发它；
- `sub_7C90A0` 的布局分派已闭合为 `0x20 -> sub_7C4070`、否则 `0x40 -> sub_7C5A20`、否则默认。fresh mode `1` 的 `0x000F` 默认排版、mode `2..4` 的 `0x0CA3/0x1CA3/0x2CA3` Flag20 排版、fresh mode `5/6` 的 `0x6C03/0x7C03` 默认排版，以及三个布局器共用的 flags `0x200` 固定 cell 分支均已实现。Flag20 包括相同的断行/截止/元数据状态机和 `+0x358` 严格最小 advance 下标。完整指令流已证明 `sub_7C5A20` 与 Flag20 的有效差异只有后者的 `+0x358` 初始化/两次更新，但组件内直接字段与 setter 审计仍找不到任何生成 `0x40` 的入口；因此 Flag40 的真实 flags 来源仍待闭合；
- `sub_7C8BE0` 的 hash rehash；当前完整字体最多 7 页、真实文本最多 6 个 batch，不会触发该分支。

在这些输入闭环前，不得自行补 point/style、mode-6 自动调用、`Flag40` 或 rehash 行为。
