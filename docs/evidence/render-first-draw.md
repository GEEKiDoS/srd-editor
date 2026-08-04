# 首个证据完整 SRD draw list

本页把已经分别闭环的解析、运行时状态、矩阵、shader 和 D3D9 状态收束为第一个可以提交 GPU 的 draw。它不增加未经证明的游戏语义，也不把编辑器预览布局当作游戏输入。

## 选定 fixture

文件为 `surfboard/system/CHU_UI_System_00_v10.srd`。解析后的 scene 0/layer 0 中，唯一满足当前全部证据边界的对象是 node 1 `C_fill`：

- 它是 `SrImageCast`，不是带 `TEXT` 子块及 CIMG flags `0x100` 的 `SrTextCast`；
- 它不绑定纹理；
- Shape key 为 `00000000:00371F90`；
- Simple compact key 为 `AAEBABBAAAGAAAAAAA`，对应已经嵌入并由真实 D3D9 HAL device 创建验证的 VS/PS；
- 初始 packet 精确导出 `CULL_NONE`、`SOLID`、color-write `0xF`，且 Z disabled。

对应证据链分别见 [`cimg-image-cast.md`](cimg-image-cast.md)、[`render-shader-constants.md`](render-shader-constants.md)、[`render-shader-bytecode.md`](render-shader-bytecode.md)、[`render-raster-state.md`](render-raster-state.md) 与 [`render-alpha-depth-stencil.md`](render-alpha-depth-stencil.md)。

## FirstCalcMatrix 边界

`build_evidence_complete_initial_image_draws` 要求调用者显式提供 `FirstCalcMatrix`。语料测试传入 identity，只是在独立宿主输入固定为 identity 时验证确定结果；它不声称原游戏任意调用现场都使用 identity。该矩阵来自 SrPlayer 所在的外部 scene graph，不在 SRD 文件中，详见 [`projection.md`](projection.md)。

编辑器未来的 fit-to-view 属于 Composition 显示变换，必须与这个游戏根矩阵分层保存。HiDPI 只改变窗口/backbuffer 的物理像素和 ImGui 的逻辑到物理比例，也不能进入 SRD 的 `FirstCalcMatrix`。

## 当前生成结果

在 `FirstCalcMatrix = identity`、target width `1920` 时，Rust 生成一个 draw：

```text
scene/layer/node = 0/0/1
positions        = (0,0,0), (0,1080,0), (1920,0,0), (1920,1080,0)
primary color    = 00 00 00 FF（四顶点相同）
secondary color  = 00 00 00 00（四顶点相同）
primitive        = non-indexed triangle strip, 4 vertices
```

draw 同时携带精确 packet、VS `c0..c13`/PS `c0` 固定常量、blend、raster 和 depth 状态。世界矩阵使用 `FirstCalcMatrix * layer local` 后沿 CAST 层级执行 `parent_world * local`；世界色只在 NODE flags 明确要求时继承父级乘色、加色和可见性。

## 显式排除范围

当前 builder 只产出证据完整子集：

- 遇到 CAST 特殊矩阵 flags `0x0007_0000` 时返回错误；
- 排除 `SrTextCast`；
- 暂不产出有纹理 draw；
- 暂不产出没有已注册 VS/PS bytecode 的 shader key；
- CNUM、CSLI 与引用层递归尚未进入这个首个 draw list。

因此“没有产出”不等于对象不可渲染，只表示它尚未到达本项目要求的完整证据门槛。当前 97 个单元测试和 19 个本地/完整游戏语料测试均通过；其中本页 fixture 测试断言 draw 数量、节点、shader key、四顶点、两组顶点色、color-write 和深度状态。
