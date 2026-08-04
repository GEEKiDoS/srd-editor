# 编辑器 D3D9 与 Dear ImGui 后端

本页记录编辑器基础设施的实际实现和验证边界。它不作为 Surfride 游戏渲染语义的证据；游戏侧的顶点、状态、shader 和资源结论仍必须分别由二进制调用链闭环。

## 架构与依赖边界

编辑器按 Rust 构建宿主机的原生指令集发布，不绑定原游戏的 32 位 x86 ABI。当前实现通过 `windows-rs` 直接调用系统 `d3d9.dll`：

- `Direct3DCreate9(D3D_SDK_VERSION)`；
- 默认 adapter、`D3DDEVTYPE_HAL`、windowed discard swap chain；
- 优先 hardware vertex processing，创建失败后退到 software vertex processing；
- `D3DFMT_D24S8` 自动 depth/stencil；
- `D3DPRESENT_INTERVAL_ONE`。

运行时不链接、不加载也不调用 D3DX 或 NVIDIA Cg。Cg 仅存在于隔离的离线 shader 取证流程中，详见 [`render-shader-bytecode.md`](render-shader-bytecode.md)。

## Dear ImGui renderer

`src/imgui_dx9.rs` 是针对 imgui-rs draw data 的原生 Rust D3D9 renderer，行为以 Dear ImGui 官方 DX9 backend 为基础。实现包括：

- D3D9 fixed-function pipeline，不需要 UI shader；
- ImGui 顶点到 XYZ、diffuse、UV 动态 vertex buffer 的转换；
- 16/32 位动态 index buffer；
- display position、framebuffer scale、每条命令的 vertex/index offset 和 scissor；
- RGBA 字体图集到 `A8R8G8B8` 的 BGRA 字节转换；
- 半像素偏移的正交投影；
- state block 与 world/view/projection 的保存、恢复；
- texture ID 注册表，为后续 Composition 中的 D3D9 render target/texture 显示保留接入口。

这一后端只负责编辑器 UI。它的 fixed-function 状态不能代替 SRD 的游戏 shader 与 draw packet 状态。

## Device lost 与 Reset

窗口 resize 会先记录新的 backbuffer 尺寸。每帧通过 `TestCooperativeLevel` 区分 ready、device lost 和 device not reset：

1. device lost 时不提交或 present；
2. 可以 reset 时，先释放 ImGui 的 DEFAULT-pool vertex/index buffer 与字体纹理；
3. 调用 `IDirect3DDevice9::Reset`；
4. 重新创建字体纹理，其他动态缓冲在下一帧按需创建；
5. 成功后继续 `Clear`、`BeginScene`、ImGui draw、`EndScene`、`Present`。

后续 SRD GPU 资源接入同一生命周期时，也必须按其 pool 和游戏证据分别注册失效/重建，不能仅假设 ImGui 的处理足够。

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
cargo run -- --d3d9-smoke
cargo run -- --d3d9-smoke "D:\sdhd\assets\data\surfboard\system\CHU_UI_System_00_v10.srd"
```

两条命令均成功创建真实 D3D9 HAL device，生成并提交第一帧 ImGui draw data；随后显式触发 resize/reset 路径，重建 DEFAULT-pool UI 资源并成功提交第二帧。第二条命令还在帧生成前实际解析指定 SRD 并构建文档面板。

同次检查还执行完整回归：88 个单元测试及 18 个本地语料测试全部通过，其中完整游戏数据根为 `D:\sdhd\assets\data`。

## 当前边界

本页证明的是可运行编辑器窗口、D3D9 device、ImGui draw 与 reset 生命周期。它尚不证明 SRD 像素渲染完成。仍需在独立证据闭环后接入：

- DDS 解码/上传与 stage 0/1 资源绑定；
- format 14 顶点提交；
- 精确 shader key 到已验证 bytecode 的 runtime 选择；
- shader 常量、sampler、blend/depth/stencil/scissor；
- scene、reference cast 和动画状态到实际 draw ordering。
