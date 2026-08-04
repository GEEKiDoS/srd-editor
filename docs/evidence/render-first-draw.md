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

`build_evidence_complete_initial_image_draws` 要求调用者显式提供无默认值的 `SrdHostDrawContext`，其中分别保存 `FirstCalcMatrix` 和 target Camera 的 `Projection*View`。语料测试对两者传入 identity，只是在独立宿主输入固定时验证确定结果；它不声称原游戏任意调用现场都使用 identity。两者都不在独立 SRD 文件中，详见 [`projection.md`](projection.md) 与 [`render-target-routing.md`](render-target-routing.md)。

编辑器未来的 fit-to-view 属于 Composition 显示变换，必须与这个游戏根矩阵分层保存。HiDPI 只改变窗口/backbuffer 的物理像素和 ImGui 的逻辑到物理比例，也不能进入 SRD 的 `FirstCalcMatrix`。

## 当前生成结果

在 `FirstCalcMatrix = identity`、显式 target `Projection*View = identity` 时，Rust 生成一个 draw：

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

因此“没有产出”不等于对象不可渲染，只表示它尚未到达本项目要求的完整证据门槛。当前 97 个单元测试和 20 个本地/完整游戏语料测试均通过；其中本页 fixture 测试断言 draw 数量、节点、shader key、四顶点、两组顶点色、color-write 和深度状态。

## 实际 D3D9Ex 提交与像素验证

`src/d3d9_srd.rs` 已把这个 draw 接到真实 D3D9Ex device：

- 由 `SRD_D3D9_VERTEX_DECLARATION` 创建 format 14 vertex declaration；
- 在 DEFAULT pool dynamic/write-only vertex buffer 中按原始 36 字节布局写入四顶点；
- 选择嵌入式 key 对应 VS/PS，上传 VS `c0..c13` 与 PS `c0`；
- 提交已闭合 blend、cull、fill、color-write 和 depth state；
- 执行 `DrawPrimitive(D3DPT_TRIANGLESTRIP, 0, 2)`；
- 用 state block 恢复编辑器调用前的 D3D9 状态。

material scissor 在二进制中是外部 context，而不是 SRD 属性，所以 renderer API 要求显式传入 `SrdDx9ExternalContext`。独立 smoke 明确使用 disabled scissor；这只是测试宿主输入，不外推为游戏任意调用现场的状态。当前首个 GPU 子集同样排除需要未知基础 alpha function/reference 的 alpha-test 或 stencil draw。

`--srd-draw-smoke` 在物理 backbuffer 上先清为 `0xFF202226`。为保留既有 GPU 诊断，它只在 smoke flag 下显式构造“SRD CAM 作为 target Camera”的诊断 host；该 host 不会用于普通编辑器预览，也不声称等于游戏 target。`EndScene` 后用 `GetRenderTargetData` 复制到 SYSTEMMEM surface，并在 fixture 内部采样点验证 B/G/R 为零。随后它进入 ImGui draw 和 `PresentEx`，再强制 `ResetEx`、释放并重建 SRD/ImGui DEFAULT-pool 资源，第二帧重复同一像素断言。

Composition 现已创建与 SCN 尺寸一致、格式取自实际 backbuffer 的 DEFAULT-pool render-target texture，并把它注册到 ImGui texture table。面板按可用逻辑尺寸保持宽高比居中显示；HiDPI framebuffer scale 只作用于 ImGui 最终顶点与 scissor。`ResetEx` 前会先从 texture table 移除 COM 引用并释放 target，reset 后重建和重新注册。

smoke 在 `EndScene` 后同时回读 Composition texture 和主 backbuffer 的 fixture 内部像素，两者均验证为黑色；随后强制 reset 的第二帧重复通过。这里的 aspect-fit 是编辑器显示层变换，不会回写或替代 `FirstCalcMatrix`。
