# SrRenderer 的 CAST 可见性矩形与空 target 未初始化状态

本文记录 `sub_AC6660 -> sub_1049940` 的完整 ImageCast 可见性测试、Advertise/Common 空 `TargetScene` 的 null lookup 证据，以及 null 分支读取未初始化矩形的原二进制边界。

## 已闭合的矩形算法

`srd_renderer_configure_project_camera` (`0xAC7400`) 在 target pointer 与 SrProject 都非空时，从 target Width/Height 构造：

```text
SrRenderer+0x24C = width  * 0.5   // center X
SrRenderer+0x250 = height * 0.5   // center Y
SrRenderer+0x254 = width  * 0.5   // half width
SrRenderer+0x258 = height * 0.5   // half height
```

ImageCast、SliceCast、NumberCast 和 Fennel 路径均经 thunk `sub_469227` 调用 `sub_AC6660`。二维 CAST 直接取四个 world corner 的 X/Y；三维 CAST 使用 `SrRenderer+0x48` 的第 0、1、3 行做 homogeneous X/Y/W 与除法。四点以 SSE `MINSS/MAXSS` 的实际 operand 顺序得到 min/max，再构造中心与半尺寸。

`sub_1049940` (`0x1049940`) 是 inclusive center/half AABB 测试：

```text
abs(cx1-cx2) <= halfW1+halfW2
abs(cy1-cy2) <= halfH1+halfH2
```

`sub_AC6660` 对相交返回 0、对不相交返回 1；各 CAST renderer 因此在非零时跳过 draw。`sub_AD9160 -> sub_AC5740` 在 `0xAC57B3..0xAC57C8` 用 TextCast 的 SrImage geometry 和二维标志调用同一 quad builder，随后以 CAST 世界矩阵变换四角，并在 `0xAC5AA8` 调用同一裁剪 thunk；非零时直接跳过整个文字 draw。Rust 已在命名 target 明确解析成功的 ImageCast 与当前 2D Fennel 路径复现四角投影、SSE 边界、NaN/有符号零比较和 inclusive AABB 剔除。

完整 91 文件语料在 `1920x1080` 命名 target 下保留 551 个 project-layer Fennel draw/34326 个顶点；reference 展开后保留 1155 个 draw/54504 个顶点，其中 604 个来自 copied TextCast。与未接该调用时相比，43 个 target 外复制实例被排除。Slice 的 `0xADA980` 与 Number 的 `0xADE9FA` 都已接入统一 runtime draw 枚举、相同 CAST-level 可见性检查和 target planner；Number 与二进制一致先以整个 CNUM SrImage quad 剔除，再决定是否遍历 glyph，不做逐 glyph 可见性猜测。

## 空名称 lookup 已闭环为 null

`SglScene` 的注册工厂在 `sub_12A9CE0` 的 `0x12A9D4E..0x12A9D58` 通过 `sub_45A628` 注册；类型 getter `sub_12B6D50` 返回完整 32 位键 `0x005B916A`。AFB loader `sub_126B1F0` 从原始 AFNC/AFRC payload 构建记录，工厂分派 `sub_126F740` 使用记录中的完整 32 位类型键，不是字符串或低字节。

对本地完整 `D:\sdhd\assets\data` 的 293 个 `.afb` 做只读字节审计：

- `0x005B916A` 的小端与大端表示均为 0 次；
- 正向对照 `SglInstancingModel` 类型键 `0x005B9172` 的小端表示为 35 次，分布在 15 个 AFB；
- 例如 `st_22801.afb+0xB14` 直接保存 `72 91 5B 00`，证明当前发行 AFB 的此类工厂键未经压缩且可被同一方法定位。

Scene 管理器构造器 `sub_62EA50` 在 `0x62F50C..0x62F532` 分配 `+0x114` 的 0x20 字节名称 map；`sub_673A40` 只构造空树与零计数，不插入空 key。该 map 唯一的 target 插入包装器 `sub_403D7D -> sub_6738D0` 只有 `sub_673F10` 一个调用点。注册 thunk `sub_43BB2E` 的 18 个调用点中，正常运行时的固定名称和字符串拼接名称均为非空；`sub_769D10` 是调试 `Scene List` 的 Create 回调；剩余 AFB 驱动的 `star::SglScene` 在发布 AFB 中没有实例。

lookup 链 `sub_4324D4 -> sub_6384A0 -> sub_4545C0 -> sub_631F60 -> sub_674980` 对空字符串执行精确树查找；找到 end sentinel 时直接返回 0，没有 fallback。因此 Advertise/Common 的空 property 2 在当前发布数据与正常启动路径中精确解析为 null，packet 进入全局队列。

## null 分支的原二进制未初始化读取

`srd_construct_renderer` (`0xAC4010`) 只初始化 `+0x248` 和 `+0x25C/+0x25D`，没有初始化 `+0x24C..+0x258`。`surfride::SrPlayer::Impl` 的分配链最终是 `_aligned_malloc(size, 8)`，也没有清零这四个 f32。对 Surfride `0xAA0000..0xAF0000` 的完整 rendered-listing 搜索只发现 `0xAC7400` 写 renderer 的这组矩形字段。

与此同时，`sub_AC6660` 在每个 ImageCast 前无条件读取该矩形。既然空名称 lookup 已证明返回 null，这就是原二进制对未初始化堆内容的真实读取，而不是隐藏空-key target。静态证据无法为这四个 f32 给出确定值；不同分配历史理论上可以产生不同剔除结果。

因此当前严格结论是：

- 命名 target 成功解析时，矩阵与可见性剔除已闭环并实现；
- Advertise/Common 的空名称 lookup 已证明返回 null，`+0x08/+0x48` 保持 identity，packet 进入全局队列；
- Chusan Advertise/Common 的 property 2 字符串确实为空；
- null 分支的 `+0x24C..+0x258` 没有可证明的确定初值。

Rust 的 `renderer_project_target=None` 现在表示已证明的原游戏 null lookup。为了保持内存安全和可重复预览，该路径确定性地跳过依赖 `+0x24C..+0x258` 的剔除，不会把零、Present size、接收 target size 或 SRD SCN size 伪造成原始矩形。该宿主策略是对原程序未初始化读取的显式边界，不是逐位复现任意堆历史。
