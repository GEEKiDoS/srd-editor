# 编辑器 D3D9Ex、HiDPI 与 Dear ImGui 后端

本页记录编辑器基础设施的实际实现和验证边界。它不作为 Surfride 游戏渲染语义的证据；游戏侧的顶点、状态、shader 和资源结论仍必须分别由二进制调用链闭环。

## 架构与依赖边界

编辑器按 Rust 构建宿主机的原生指令集发布，不绑定原游戏的 32 位 x86 ABI。当前实现通过 `windows-rs` 直接调用系统 `d3d9.dll`：

- `Direct3DCreate9Ex(D3D_SDK_VERSION)` 和 `IDirect3D9Ex::CreateDeviceEx`；
- 默认 adapter、`D3DDEVTYPE_HAL`、windowed discard swap chain；
- 优先 hardware vertex processing，创建失败后退到 software vertex processing；
- `D3DFMT_D24S8` 自动 depth/stencil；
- `D3DPRESENT_INTERVAL_ONE`。

运行时不链接、不加载也不调用 D3DX 或 NVIDIA Cg。Cg 仅存在于隔离的离线 shader 取证流程中，详见 [`render-shader-bytecode.md`](render-shader-bytecode.md)。

初始化还会把首个证据闭环 fixture 的嵌入式 `vs_3_0/ps_3_0` token 直接交给同一个 `IDirect3DDevice9Ex::CreateVertexShader/CreatePixelShader`。创建失败会中止编辑器启动或 smoke test；这一验证不加载 Cg/D3DX。

## Dear ImGui renderer

`src/imgui_dx9.rs` 是针对 imgui-rs draw data 的原生 Rust D3D9 renderer，接收 D3D9Ex device 的 `IDirect3DDevice9` 基接口，行为以 Dear ImGui 官方 DX9 backend 为基础。实现包括：

- D3D9 fixed-function pipeline，不需要 UI shader；
- ImGui 顶点到 XYZ、diffuse、UV 动态 vertex buffer 的转换；
- 16/32 位动态 index buffer；
- display position、framebuffer scale、每条命令的 vertex/index offset 和 scissor；
- RGBA 字体图集到 `A8R8G8B8` 的 BGRA 字节转换；
- 半像素偏移的正交投影；
- state block 与 world/view/projection 的保存、恢复；
- texture ID 注册表，为后续 Composition 中的 D3D9 render target/texture 显示保留接入口。

这一后端只负责编辑器 UI。它的 fixed-function 状态不能代替 SRD 的游戏 shader 与 draw packet 状态。

## D3D9Ex 状态与 ResetEx

窗口 resize 会先记录新的物理 backbuffer 尺寸并标记 reset pending。没有待处理 resize 时，每帧通过 `IDirect3DDevice9Ex::CheckDeviceState` 检查目标窗口状态；实现不再调用传统 D3D9 的 `TestCooperativeLevel`：

1. device lost 时不提交或 `PresentEx`；
2. resize 或 DPI 变化需要 reset 时，先释放 ImGui 的 DEFAULT-pool vertex/index buffer 与字体纹理；
3. 调用 `IDirect3DDevice9Ex::ResetEx`；
4. 重新创建字体纹理，其他动态缓冲在下一帧按需创建；
5. 成功后继续 `Clear`、`BeginScene`、ImGui draw、`EndScene`、`PresentEx`。

后续 SRD GPU 资源接入同一生命周期时，也必须按其 pool 和游戏证据分别注册失效/重建，不能仅假设 ImGui 的处理足够。

## HiDPI

窗口初始尺寸使用 winit 逻辑单位，D3D9Ex backbuffer 始终使用 `Window::inner_size()` 返回的物理像素。`imgui-winit-support` 的 `HiDpiMode::Default` 维护逻辑 `display_size`、实际 `display_framebuffer_scale`、鼠标坐标和 `ScaleFactorChanged`。

默认字体不是由 GPU 放大低分辨率图集：编辑器以 `13 * dpi_factor` 的实际像素尺寸重新栅格化字体，并以 `font_global_scale = 1 / dpi_factor` 保持 13 个逻辑单位的版面高度。跨显示器 DPI 变化时，事件处理会：

1. 释放当前 ImGui font/VB/IB DEFAULT-pool 资源；
2. 用新 DPI 清空并重建字体图集；
3. 记录窗口新的物理 backbuffer 尺寸；
4. 在下一帧执行 `ResetEx` 并重新上传字体纹理。

面板宽度、timeline 固定列和 ImGui style 数值保持逻辑单位，由 framebuffer scale 映射到物理像素，因此不会再额外 `scale_all_sizes` 造成双重缩放。

## 工作区与真实文档路径

编辑器启用 Dear ImGui docking，默认布局为：

- 中央 `Composition`；
- 左侧 `Project` 与 `Scene & Status`；
- 右侧 `Properties`；
- 下方 `Layers & Timeline`，固定的层/状态列和可横向滚动的时间轴位于同一张 table，因此共享垂直滚动和行选择。

命令行第一个非选项参数作为 SRD 路径。文档加载使用当前 Rust `SrdFile`、`Project` 和 `TextureList` 解析器；面板显示真实 scene/layer/NODE、变换、纹理记录，以及选中 layer 第一个动画中的实际 key frame。中央 Composition 尚未连接 SRD GPU draw submission，界面会明确显示 pending，不输出近似画面。

## 本机验证

2026-08-04 在 `aarch64-pc-windows-gnullvm` Rust host 上执行：

```powershell
cargo run -- --d3d9ex-smoke
cargo run -- --d3d9ex-smoke "D:\sdhd\assets\data\surfboard\system\CHU_UI_System_00_v10.srd"
```

两条命令均通过 `Direct3DCreate9Ex`/`CreateDeviceEx` 成功创建真实 D3D9Ex HAL device，并由该 device 成功创建嵌入式 SRD VS/PS；随后生成并以 `PresentEx` 提交第一帧 ImGui draw data，再显式触发 resize/`ResetEx` 路径，重建 DEFAULT-pool UI 资源并成功提交第二帧。第二条命令还在帧生成前实际解析指定 SRD 并构建文档面板。

同次检查还执行完整回归：97 个单元测试及 18 个本地语料测试全部通过，其中完整游戏数据根为 `D:\sdhd\assets\data`。

## 当前边界

本页证明的是可运行编辑器窗口、D3D9Ex device、HiDPI ImGui draw 与 ResetEx 生命周期。它尚不证明 SRD 像素渲染完成。仍需在独立证据闭环后接入：

- DDS 解码/上传与 stage 0/1 资源绑定；
- format 14 顶点提交；
- 其余精确 shader key 到已验证 bytecode 的 runtime 选择；
- 已建模 shader 常量的实际上传，以及 sampler、blend/depth/stencil/scissor；
- scene、reference cast 和动画状态到实际 draw ordering。
