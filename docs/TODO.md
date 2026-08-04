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

## Fennel 自动断行与垂直边界

完整 `surfboard` 的 1292 条 RFZ TEXT 已有 1270 条落在证据完整的静态 fitting 子集。剩余样本边界为：

```text
自动断行/禁则：9
垂直边界：13
```

已证明 FontManager `+0xE0/+0xE4` 分别由固定 45/14 项 UTF-16 表初始化，`sub_F38DB0/sub_F38D20` 执行精确 membership 查询；但 `sub_7C1F90` 中断点回退、空格候选、两类集合与当前/前一 glyph 的组合、`-254` 标记和第二次纵向对齐 pass 尚未完整移植。恢复调查时以这 22 条真实样本逐条回归，不得用通用文本库的自动换行、Unicode Line Breaking Algorithm 或简单裁切替代。
