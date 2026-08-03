# SRD Editor

SEGA Surfride `.srd` 文件的离线解析、预览与编辑工具。

## 已确认的项目目标

- 使用 Rust 实现 SRD 二进制解析与完整重序列化。
- 使用 Dear ImGui 构建桌面编辑界面。
- 使用 Direct3D 9 复现游戏侧的 SRD 渲染路径。
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
- SRD 路径到 `air::TextureResource` 的资源工厂链、DDS header/格式/mip/cube/palette surface 布局、D3D9 原生创建与 D3DX9_43 回退参数；97 个本地 DDS 全量回归。
- TEX `0x62` 到 Wrap/Clamp、Linear/Point 双包装对象及最终 D3D9 sampler state 的完整绑定链。
- SrSliceCast 两个 packed vertex color 的解析零默认、双线性 CSLI 插值、逐通道乘法和饱和加法组合器；未证明的 CAST tint 保持为显式输入。
- `surfride::SrPlayer -> SrPlayer::Impl -> SrRenderer` 对象链、精确 4x4 乘法、Width/Height 视口矩阵以及 2D/3D CAST 最终屏幕 X/Y 映射。
- 本地 53 个 SRD 的结构解析回归。
- 53 个样本中的 192 个 SCN 和 1090 个 CRFD 引用目标回归。
- 53 个样本中的 799 个 CSLI 和 23 个实际父级单元索引关系回归。
- avatar 样本 MOT target、公共 rotation Z 通道和游戏三次曲线结果回归。
- 36 字节 D3D9 顶点声明、四顶点非索引 triangle strip、混合、alpha/depth/stencil 与 scissor 状态提交。

尚未实现：SRD 写回、公共 packed color/alpha 通道、投影矩阵的上游 camera/backend 输入、CNUM 历史 glyph 动画、TEXT、原生 DDS 的逐 surface 上传和设备丢失生命周期、shader/双 UV 消费、可运行的 D3D9 渲染后端与 ImGui 编辑界面。这些部分会在对应游戏代码完成证据闭环后逐项加入。贴图像素解码不自行重写：将按游戏分支直接使用 D3D9 与 `D3DX9_43`。

调查证据和待验证假设记录在 [`docs/srd-format.md`](docs/srd-format.md)。
