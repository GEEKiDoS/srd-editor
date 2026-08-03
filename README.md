# SRD Editor

SEGA Surfride `.srd` 文件的离线解析、预览与编辑工具。

## 已确认的项目目标

- 使用 Rust 实现 SRD 二进制解析与完整重序列化。
- 使用 Dear ImGui 构建桌面编辑界面。
- 使用 Direct3D 9 复现游戏侧的 SRD 渲染路径。
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
- 公共空间动画通道、游戏自有 sin/cos 近似、局部 3x4 仿射矩阵和 `parent_world * local` 乘法。
- CSLI/SLIC 记录、`surfride::SrSliceCast` 链接、尺寸/origin 计算、网格单元生成、NODE `0x32` 父级单元索引及其完整偏移链。
- SrSliceCast active 单元的局部四顶点、2D/3D Y 轴分支、36 字节游戏顶点顺序、CSLI CREF 选择、flags flip/order 与两个相同最终 UV 通道。
- TEXL/TEX/CROP 的 540 字节记录、外部 DDS 基础路径、纹理尺寸、16 字节归一化矩形表，以及 CREF 到实际矩形的解析。
- SRD 路径到 `air::TextureResource` 的资源工厂链、DDS header/格式/mip/cube/palette surface 布局、D3D9 原生创建、游戏中的 D3DX9_43 回退分支和二维 SYSTEMMEM staging/`UpdateSurface` 参数；旧 97 文件语料和完整游戏 `surfboard` 下 360 个 DDS 均已全量回归。游戏的 D3DX 分支仅作为兼容行为证据，编辑器自身不链接或调用 D3DX。
- TEX `0x62` 到 Wrap/Clamp、Linear/Point 双包装对象及最终 D3D9 sampler state 的完整绑定链。
- CAST `CATL/CATR` 通用属性列表、`ExtParamData` 12 字节运行时结构、blend preset 覆盖和继承式层级键。
- SrSliceCast 两个 packed vertex color 的解析零默认、双线性 CSLI 插值、逐通道乘法和饱和加法组合器；未证明的 CAST tint 保持为显式输入。
- `surfride::SrPlayer -> SrPlayer::Impl -> SrRenderer` 对象链、精确 4x4 乘法、Width/Height 视口矩阵以及 2D/3D CAST 最终屏幕 X/Y 映射。
- 完整游戏 `surfboard` 下 91 个 SRD 的结构解析回归。
- 91 个样本中的 263 个 SCN 和 1299 个 CRFD 引用目标回归。
- 91 个样本中的 983 个 CSLI 和 25 个实际父级单元索引关系回归。
- avatar 样本 MOT target、公共 rotation Z 通道和游戏三次曲线结果回归。
- 36 字节 D3D9 顶点声明、四顶点非索引 triangle strip、混合、alpha/depth/stencil 与 scissor 状态提交。
- 绘制包到 64 位 ShapeEnv shader cache key 的全部位来源、CREF/CRE1 到 D3D9 texture stage 0/1 的映射、vertex-format/blend/multi-texture 模块索引，以及完整 `surfboard` 语料 19,484 个初始 image node 的实际 key 回归。
- ShaderSelector 的 46 字节紧凑键到 180 位 feature 的解码、`#define name value` 前缀生成、前缀与原始 Cg source 的字节级拼接，以及 stage 0/1 到 pixel/vertex Shader resource 的映射。

尚未实现：SRD 写回、公共 packed color/alpha 通道、投影矩阵的上游 camera/backend 输入、CNUM 历史 glyph 动画、TEXT、DDS 内部格式转换/cube request/设备丢失生命周期、ShapeEnv 生成源码/最终 bytecode 与双 UV 像素公式、可运行的 D3D9 渲染后端与 ImGui 编辑界面。这些部分会在对应游戏代码完成证据闭环后逐项加入。贴图像素解码不自行重写；编辑器使用独立解码库并直接上传到 D3D9，全程不依赖 D3DX。

调查证据和待验证假设记录在 [`docs/srd-format.md`](docs/srd-format.md)。
