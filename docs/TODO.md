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

静态 `sub_7C1F90` 的自动断行、两张固定表、空格候选、二次纵向 pass、独立 TextBox `+0x108`、`-254` 标记和 `sub_7C90A0` 尾部 record-limit 返回值已经实现；`sub_7C0D40` 的负记录过滤、`-254` 立即停止、静态 maximum=-1、texture token 分组、normal/effect 计数和初始 17 桶前向链顺序也已实现；`sub_7C7F90` 静态 normal glyph 的 bearing origin、effective scale、2D matrix 顺序和无裁剪顶点同样已实现。SrTextCast 初始 world/color、零颜色门控和 ShapeEnv material 的 CW cull 已接入 D3D9Ex Composition。完整 1292 条 RFZ TEXT 全部通过，单条最多 6 个 batch/1620 个 normal 顶点；真实语料有 550 个初始可见 2D draw，Advertise 像素回归为 40920 个 changed pixels。

尚未闭合：

- TextBoxObject `+0x12C` 的 `float2` 行位置向量与 `+0x34C` 的 16 字节行描述记录的全部后续消费者；
- record `+0x0C & 0x40000` 的第二组 effect glyph 参数与生成来源；
- `sub_7C8BE0` 的 hash rehash；当前完整字体最多 7 页、真实文本最多 6 个 batch，不会触发该分支。

在这些输入闭环前，不得自行补 effect、裁剪参数或 rehash 行为。
