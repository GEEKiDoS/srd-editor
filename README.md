# SRD Editor

SEGA Surfride `.srd` 文件的离线解析、预览与编辑工具。

## 已确认的项目目标

- 使用 Rust 实现 SRD 二进制解析与完整重序列化。
- 使用 Dear ImGui 构建桌面编辑界面。
- 使用 Direct3D 9Ex 承载编辑器和复现游戏侧的 SRD 渲染路径。
- 按构建宿主机的原生指令集发布，不把编辑器绑定到原游戏的 32 位 x86 架构。
- 不依赖或调用 D3DX；纹理解码使用独立库并将结果上传至 D3D9，着色器采用不依赖 D3DX 的、经二进制行为验证的方案。
- 支持纹理图集、裁剪、节点层级、动画、文本与常见 CAST 类型。
- 保留原始字节和未知字段，避免编辑时破坏尚未还原的结构。
- 通过离线样本和 IDA 数据库验证，不启动游戏或 `amdaemon`。

旧 Python/PySide6 项目位于相邻的 `WORK/srd_editor`，只能用于提供调查线索、样本路径和待核对问题。旧实现与旧文档中的字段含义、动画公式和渲染规则都不能直接移植；本仓库只接受能够由游戏二进制及样本闭环证明的逻辑。

## 当前状态

已经实现并验证：

- 只读 VTBF/SRFF 结构解析，按游戏读取器保留未知头字段和属性原始编码。
- `ANIM → MOT → TRK → KEY` 记录读取。
- 游戏标量轨道的时间区间、端点、线性、保持和三次曲线求值。
- LAYR、NODE、TRS2/TRS3 记录读取、2D/3D flags 分派和首子/同级层级构建。
- `SRFF -> SRCK -> PROJ -> SCN  -> LAYR` 项目场景表，以及 CRFD 在同文件 SCN/LAYR 表中的首次完整名称解析。
- `PROJ -> CAM ` 的 position/target/angle/near/far、`SCN 0x40/0x41` composition 尺寸、全局 `sea::Camera` 的 RH View/Perspective/`Projection*View`，以及 Simple shader `mtxPrjView` 常量 provider。
- 公共空间动画通道、游戏自有 sin/cos 近似、局部 3x4 仿射矩阵和 `parent_world * local` 乘法。
- CSLI/SLIC 记录、`surfride::SrSliceCast` 链接、尺寸/origin 计算、网格单元生成、NODE `0x32` 父级单元索引及其完整偏移链。
- SrSliceCast active 单元的局部四顶点、2D/3D Y 轴分支、36 字节游戏顶点顺序、CSLI CREF 选择、flags flip/order 与两个相同最终 UV 通道。
- TEXL/TEX/CROP 的 540 字节记录、外部 DDS 基础路径、纹理尺寸、16 字节归一化矩形表，以及 CREF 到实际矩形的解析。
- SRD 路径到 `air::TextureResource` 的资源工厂链、DDS header/格式/mip/cube/palette surface 布局、D3D9 原生创建、游戏中的 D3DX9_43 回退分支和二维 SYSTEMMEM staging/`UpdateSurface` 参数；旧 97 文件、完整 `surfboard` 的 360 个 DDS，以及整个游戏 `data` 的 14,694 个 DDS 均已全量回归。全游戏语料只有 A8R8G8B8/DXT1/DXT5，7,204 个文件会走原游戏 D3DX 回退；编辑器自身不链接或调用 D3DX。
- 编辑器 DDS 后端已实装：7,490 个游戏原生兼容文件保持原 A8R8G8B8/DXT1/DXT5 surface 布局，经 SYSTEMMEM staging 和 `UpdateSurface` 上传；7,204 个 NPOT DXT fallback 由 `image_dds 0.7.2` 的纯 Rust BC1/BC3 decoder 转为 RGBA8，再转换为 D3D9 A8R8G8B8 字节布局上传。14,694 个文件、14,722 个 mip 已全部由真实 D3D9Ex HAL device 创建并上传成功。
- TEX `0x62` 到 Wrap/Clamp、Linear/Point 双包装对象及最终 D3D9 sampler state 的完整绑定链。
- CAST `CATL/CATR` 通用属性列表、`ExtParamData` 12 字节运行时结构、blend preset 覆盖和继承式层级键。
- SrSliceCast 两个 packed vertex color 的解析零默认、双线性 CSLI 插值、逐通道乘法和饱和加法组合器；未证明的 CAST tint 保持为显式输入。
- `surfride::SrPlayer -> SrPlayer::Impl -> SrRenderer` 对象链、CAM 到 Camera 的运行时传递、精确 4x4 乘法、Width/Height 视口矩阵以及 2D/3D CAST 最终屏幕 X/Y 映射。
- 完整游戏 `surfboard` 下 91 个 SRD 的结构解析回归。
- 91 个样本中的 263 个 SCN 和 1299 个 CRFD 引用目标回归。
- 91 个样本中的 983 个 CSLI 和 25 个实际父级单元索引关系回归。
- avatar 样本 MOT target、公共 rotation Z 通道和游戏三次曲线结果回归。
- 36 字节 D3D9 顶点声明、四顶点非索引 triangle strip、混合、alpha/depth/stencil 与 scissor 状态提交。
- 绘制包到 64 位 ShapeEnv shader cache key 的全部位来源、CREF/CRE1 到 D3D9 texture stage 0/1 的映射、vertex-format/blend/multi-texture 模块索引，以及完整 `surfboard` 语料 19,484 个初始 image node 的实际 key 回归。
- ShaderSelector 注册顺序、SRD/ShapeEnv 对 Simple 槽位 9 的实际选择、Simple 的 18 字节键与 71 项表、Default 的 46 字节键机制、嵌入式 Cg source 的精确 dword 解码/include 闭包、format 14 的双 UV/双顶点色公式，以及 stage 0/1 到 pixel/vertex Shader resource 的映射。
- 默认及逐 packet 的 VS `c0..c9`、PS `c0` 常量提交，以及选定无贴图 fixture 的 VS `c10..c13 = Projection*View` provider。
- draw packet 到 D3D9 cull/fill/color-write 的精确覆盖：首个 fixture 为 `CULL_NONE`、`SOLID`、四通道写入，不依赖编辑器侧显示性兜底。
- 首个证据完整的 CPU draw list：`CHU_UI_System_00_v10.srd` 的 scene 0/layer 0/node 1 `C_fill` 在显式 identity `FirstCalcMatrix` 和 1920 宽目标下生成唯一无贴图 ImageCast draw，包含精确四顶点、世界色、packet、Simple key、固定常量、blend/raster/depth；TEXT、贴图、特殊 CAST 矩阵分支和未注册 bytecode 的 key 不会被伪装为已支持。
- 首个真实 D3D9Ex SRD draw submission：创建 format 14 顶点声明与动态 DEFAULT-pool 顶点缓冲，上传已验证 VS/PS、VS `c0..c13`、PS `c0` 和精确 blend/raster/depth 状态，执行非索引 `D3DPT_TRIANGLESTRIP`；不可见 smoke 在 `EndScene` 后通过 `GetRenderTargetData` 回读 identity-host fixture 内部像素，并在强制 `ResetEx` 后重复验证。外部 material scissor 作为显式 context 输入，不从 SRD 猜测。
- 隔离 x86 取证工具已对完整 XML 的 82 个 Simple key 生成原版 Cg assembly，经 `D3DCompiler_47!D3DAssemble` 得到 164 份无 D3DX D3D9 bytecode，并全部由 D3D9 HAL device 成功创建 shader 对象；编辑器发布物不依赖 Cg。
- 可运行的原生 D3D9Ex 编辑器外壳：按宿主机指令集构建，直接使用 `d3d9.dll` 的 `Direct3DCreate9Ex`/`IDirect3DDevice9Ex` 创建 HAL device，不链接或调用 D3DX/Cg；已在本机 ARM64 Windows 构建并完成真实 `PresentEx`/`ResetEx` 冒烟测试。
- Dear ImGui D3D9 renderer：固定管线、动态顶点/索引缓冲、字体纹理、scissor、large-mesh offset、状态备份恢复，以及 D3D9Ex reset 时 DEFAULT-pool 资源的失效与重建。窗口和 backbuffer 使用物理像素，ImGui 使用逻辑坐标；字体图集按实际 DPI 栅格化并支持跨显示器 `ScaleFactorChanged` 重建。
- After Effects 风格工作区初版：中央 Composition、左侧 Project 与 Scene/Status、右侧 Properties、下方合并的 Layers/Timeline；命令行加载真实 SRD 后，场景、层、NODE、变换、纹理和首个动画的实际关键帧会进入这些面板。
- Composition 已接入与 SCN 尺寸一致的 D3D9 DEFAULT-pool render-target texture，并通过 ImGui texture ID 在面板中按宽高比居中缩放显示。纹理保持场景像素尺寸，面板布局使用逻辑单位，最终 ImGui 顶点/scissor 再按 framebuffer scale 转到 HiDPI 物理像素；显示缩放不修改 SRD 矩阵或 `FirstCalcMatrix`。

尚未实现：SRD 写回、公共 packed color/alpha 通道、投影屏幕矩阵中另一个 backend context 的组合语义、CNUM 历史 glyph 动画、TEXT、完整语料未出现的 DDS 内部格式转换/cube request、SRD texture 到 draw stage 0/1 的最终绑定、ShapeEnv 剩余 context 到完整 Simple 键的映射，以及其余 bytecode 的 runtime 选择和 sampler 设备接入。这些部分会在对应游戏代码完成证据闭环后逐项加入。贴图像素解码由独立库完成，全程不依赖 D3DX。

运行编辑器并直接加载一个文件：

```powershell
cargo run -- "D:\sdhd\assets\data\surfboard\system\CHU_UI_System_00_v10.srd"
```

执行不可见窗口的 D3D9Ex/ImGui 两帧及强制 `ResetEx` 冒烟测试：

```powershell
cargo run -- --d3d9ex-smoke "D:\sdhd\assets\data\surfboard\system\CHU_UI_System_00_v10.srd"
```

执行首个无贴图 SRD draw、物理 backbuffer 像素回读及强制 `ResetEx` 冒烟测试：

```powershell
cargo run -- --srd-draw-smoke "D:\sdhd\assets\data\surfboard\system\CHU_UI_System_00_v10.srd"
```

执行整个目录的真实 D3D9Ex DDS 创建/上传审计：

```powershell
cargo run --release -- --dds-device-audit "D:\sdhd\assets\data"
```

调查证据和待验证假设记录在 [`docs/srd-format.md`](docs/srd-format.md)。
