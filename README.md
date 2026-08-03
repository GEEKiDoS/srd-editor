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
- 公共空间动画通道、游戏自有 sin/cos 近似、局部 3x4 仿射矩阵和 `parent_world * local` 乘法。
- CSLI/SLIC 记录、`surfride::SrSliceCast` 链接、尺寸/origin 计算、网格单元生成、NODE `0x32` 父级单元索引及其完整偏移链。
- 本地 53 个 SRD 的结构解析回归。
- 53 个样本中的 799 个 CSLI 和 23 个实际父级单元索引关系回归。
- avatar 样本 MOT target、公共 rotation Z 通道和游戏三次曲线结果回归。

尚未实现：SRD 写回、颜色及 CAST 专属通道、投影/视口、CREF/CRE1 运行时选择、最终 CSLI 顶点/UV、D3D9 渲染和 ImGui 编辑界面。这些部分会在对应游戏代码完成证据闭环后逐项加入。ImGui、窗口系统、D3D9 绑定与纹理解码依赖尚未选定。

调查证据和待验证假设记录在 [`docs/srd-format.md`](docs/srd-format.md)。
