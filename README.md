# SRD Editor

SEGA Surfride `.srd` 文件的离线解析、预览与编辑工具。

## 已确认的项目目标

- 使用 Rust 实现 SRD 二进制解析与完整重序列化。
- 使用 Dear ImGui 构建桌面编辑界面。
- 使用 Direct3D 9 复现游戏侧的 SRD 渲染路径。
- 支持纹理图集、裁剪、节点层级、动画、文本与常见 CAST 类型。
- 保留原始字节和未知字段，避免编辑时破坏尚未还原的结构。
- 通过离线样本和 IDA 数据库验证，不启动游戏或 `amdaemon`。

旧 Python/PySide6 项目位于相邻的 `WORK/srd_editor`，仅作为已验证逻辑与回归证据来源；本仓库是新的权威实现。

## 当前状态

仓库和最小 Rust 程序已建立。ImGui、窗口系统、D3D9 绑定与纹理解码依赖尚未选定，需在验证维护状态和接口后确定。

已确认的格式与逆向结论记录在 [`docs/srd-format.md`](docs/srd-format.md)。

