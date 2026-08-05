# Chusan `air::Scene` target profiles

本文只记录 `chusanApp.exe` 中 Chusan 自己创建的 `MainScene` 与 `BgScene`。它们是可供编辑器显式选择的 target profile，不是任意独立 SRD 的自动默认宿主。Advertise 的空 `TargetScene` 仍按 [`render-target-routing.md`](render-target-routing.md) 所述进入全局队列；其构造完成时的 Main/Bg 接纳结果已由 DrawMask/DrawIndex 精确闭环。

## 实际创建点

Chusan 构造路径 `sub_AE4660 -> sub_AE52B0` 创建两个 1752-byte `air::Scene`：

| target | 名称构造 | 保存位置 | 注册调用 |
| --- | ---: | ---: | ---: |
| Main | `0xAE52F9..0xAE5339` | owner `+0x54` | `0xAE5377..0xAE5381` |
| Background | `0xAE540B..0xAE544B` | owner `+0x5C` | `0xAE546E..0xAE5478` |

两次构造都调用 `sub_4085C6 -> sub_7023D0`，因此类型确实是 `air::Scene`，不是根据字符串推测。`sub_7023D0` 安装 `air::Scene` 虚表 `0x18E30B0`，并在 target `+0x90` 创建 432-byte `air::Camera`。

注册 thunk `sub_43BB2E -> sub_602210 -> sub_46524E -> sub_6310E0 -> sub_673F10` 最终把 target 按其运行时名称插入管理器 `+0x114` 的 map。`MainScene` 的参数是 order `10000`、第三参数 `true`；`BgScene` 是 order `9900`、第三参数 `false`。

第三参数不是 Enable。注册成功且参数为 true 时，`sub_602210` 调用 `sub_409D72 -> sub_632520`，只把 target 指针写入管理器 `+0x11C`；getter `sub_46DB10 -> sub_631CD0` 也只返回该槽。重命名/重注册路径 `sub_6044E0/sub_6046D0` 会比较并恢复同一指针。因此 Rust 将它记录为“注册后设置 manager current target”，不再称为 start enabled。

## 已闭环的 scene 属性

`sea::BasicScene` 构造函数 `sub_5FFF60` 注册了属性编号与名称；Chusan 随后的 setter 因而可以按编号精确解释：

| 属性 | MainScene | BgScene | 证据 |
| --- | ---: | ---: | --- |
| `Enable` (property 0) | true（构造完成） | true（构造完成） | `sub_5FFB70` 在 target `+0x10` 写 1；具体初始化没有调用 `sub_604460` 改写 |
| `PresentIndex` | 0 | 0 | 两次 `air::Scene` 构造的第二参数 |
| `DrawIndex` (property 2) | 0 | 16 | `sub_424B1D -> sub_604440` 固定写 property 2 |
| `Attribute` (property 3) | 0 | 0 | `0x6002CC..0x600310` 以默认 0、范围 0..7 注册；具体初始化没有改写 |
| `PresentMode` (property 4) | 1（构造默认） | 0 | `sub_43B97B -> sub_604850` 固定写 property 4 |
| `ShaderOnDemand` (property 7) | false | false | `0xAE5390..0xAE53A1` / `0xAE5491..0xAE54A2` |
| `RequestColorOffscreen` (property 14) | true | true | `0xAE53A6..0xAE53B7` / `0xAE54A7..0xAE54B8` |
| `RequestDepthOffscreen` (property 15) | true | true | `0xAE53BC..0xAE53CD` / `0xAE54BD..0xAE54CE` |
| `Clear` | false | false | target `+0x298` 在 `0xAE53D2..0xAE53D5` / `0xAE54D3..0xAE54DB` 清零 |

MainScene 还在 `0xAE535C..0xAE5371` 通过虚表 `+0x80` 把 `{12, 0}` 写入 target `+0x290/+0x294`。写入位置和数值已确认，但字段语义尚未闭环，因此代码和文档不为它命名。

## 构造完成时的全局 SRD target filter

基础 Scene 虚表 `+0x44` (`sub_601C60`) 先检查 target `+0x10` 的 Enable；为 true 时读取 property 2 DrawIndex，并生成：

```text
targetMask = (1 << DrawIndex) & 0x7FFFFFFF
```

所以构造完成时 MainScene 的 mask 为 `0x00000001`，BgScene 为 `0x00010000`。`sub_601CB0 -> sub_63EA50` 对 type-1/SRD command 依次要求：

- `targetMask == 0` 或 `targetMask & command.DrawMask != 0`；
- packet `+0x60 bit 0x100` 清零；
- packet `+0x58` 低字节包含 `1 << Attribute`；
- Attribute 为 1 时，packet `+0x60 bit 0x04` 还必须置位；
- command `+0x10 bit 0x02` 清零。

普通 Image/Fennel packet 的 `+0x58` 低字节为 `0xFF`、`+0x60 bit 0x100` 清零；全局 SRD command 的 `+0x10` 是 packet `+0x84` 的原位副本，低字节为 `0x11`。Advertise 默认 DrawMask 是 `0xFFFF`，因此在上述已证明的初始状态中：

```text
MainScene: 0x00000001 & 0x0000FFFF != 0 -> 接纳
BgScene:   0x00010000 & 0x0000FFFF == 0 -> 拒绝
```

这是构造完成状态的确定结论；若游戏后续通过 Scene `Enable` 属性或其他宿主配置改写状态，必须按对应时序另行追踪，不能外推为所有模式、所有帧恒定不变。

## 默认 BasePass 与 EntryInfo

`air::Scene` 不是等待外部配置后才拥有 pass。共同基类构造 `sub_6CF9A0` 在 target `+0x334` 原位构造 5 个 `sea::PassBasic`，并从静态表 `0x18A74A0` 依次写入名称、BasePass `PassIndex`、Type、Sort、Range 和 RangeValueU32。User 与 RangeValueF32 固定写 0；BasePass 构造的 Entry 默认值为 true，Chusan 的 MainScene/BgScene 后续设置没有关闭或替换这五项。

| 注册顺序 / compact rule index | 名称 | PassIndex | Type | User | Range | Sort | F32 | U32 |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | `Back2DPass` | 4 | 3 (`2D`) | 0 | 4 (`LayerBack`) | 5 (`LayerAllLower`) | 0 | 8388608 |
| 1 | `OpaquePass` | 8 | 0 (`Opaque`) | 0 | 0 | 3 (`DepthLower`) | 0 | 0 |
| 2 | `PunchPass` | 12 | 1 (`Punch`) | 0 | 0 | 3 (`DepthLower`) | 0 | 0 |
| 3 | `TrancePass` | 16 | 2 (`Trans`) | 0 | 0 | 6 (`LayerLower+DepthUpper`) | 0 | 0 |
| 4 | `Front2DPass` | 24 | 3 (`2D`) | 0 | 3 (`LayerFront`) | 5 (`LayerAllLower`) | 0 | 8388608 |

`TrancePass` 是二进制字符串的原始拼写。字段到 24-byte rule 的映射由 BasePass 虚表闭环：`+0x70 -> sub_624B80` 读取 property 0 `PassIndex`；`+0x7C -> sub_624B30` 先读取 property 1 `Entry`，为 true 才从对象 `+0x60..+0x77` 原样复制 Type/User/Range/Sort/F32/U32。

`sub_64BDA0` 对启用项按注册顺序生成 compact rule index，并调用 `sub_4354A4 -> sub_64F970` 更新 target 的 32 项 EntryInfo。这里两个索引不能混同：BasePass `PassIndex` 是 EntryInfo 数组下标，写入范围值才是 compact rule index。MainScene/BgScene 的默认结果因此精确为：

```text
entry[4]  = (0, 0)
entry[8]  = (1, 1)
entry[12] = (2, 2)
entry[16] = (3, 3)
entry[24] = (4, 4)
其余       = (-1, -1)
```

若多个启用 BasePass 使用同一 PassIndex，`sub_64F970` 只在 first 为 -1 时写 first，但每次都更新 last；若 Entry 为 false，该项既不进入 rule vector，也不占 compact rule index。Rust `target_pass::build_evidence_scene_pass_profile` 已逐项复现这个行为，`CHUSAN_MAIN_SCENE` 与 `CHUSAN_BG_SCENE` 均可直接取得该已证明 profile。

## Camera 构造值

`air::Camera` 先调用 `sea_camera_construct` (`0x654550`)。属性注册调用同时写入当前默认值：

| Camera 属性 | 构造值 |
| --- | ---: |
| Near | 1.0 |
| Far | 30000.0 |
| FovY | 45.0 degrees |
| Aspect | 1.0（仅构造瞬间） |
| IsPerspective | true |
| Position | `(0, 0, 30)` |
| Target | `(0, 0, 0)` |
| Up | `(0, 1, 0)` |
| OffsetX / OffsetY | 0 / 0 |

这还不是 target 的最终投影。`sub_7023D0` 随后调用 Camera 虚表 `+0x90`，即 `sub_418467 -> sub_655F10`。该回调从 scene 读取 Width 和 Height，再把 Camera property 3 `Aspect` 写成：

```text
scene_width / max(scene_height, 1)
```

因此 MainScene/BgScene 的已证明 target Camera 是上述构造参数，加上运行时 present buffer 的宽高比。

## Width / Height 的来源边界

`sub_5FFF60` 从 `MEMORY[0x1CC520C][PresentIndex]` 取得 present wrapper，再以 `sub_E7C8F0` / `sub_E7C8E0` 读取 wrapper `+0x18/+0x1C` 作为 scene Width/Height。wrapper 构造 `sub_E7CD10` 又从 primary framebuffer `+0x10/+0x14` 复制这两个值；primary framebuffer 最终使用图形启动配置或实际 display mode 建立。

所以二进制没有为 `MainScene` 内嵌一个独立固定分辨率。编辑器必须显式提供所模拟的 present width/height，不能从 target 名称推导 1080p、portrait 或其他尺寸。

## Rust 边界

`src/game_host.rs` 现在提供：

- `CHUSAN_MAIN_SCENE`；
- `CHUSAN_BG_SCENE`；
- 两个 target 的初始 Enable、DrawIndex、Attribute、dispatch mask；
- Advertise `DrawMask=0xFFFF` 对 Main 接纳、对 Bg 拒绝的 type-1 filter；
- 两个 target 共用的五项 BasePass/rule 与 32 项 EntryInfo profile；
- `projection_view_for_present_size(width, height)`。

Camera 函数只恢复 target 的 `Projection * View`，不生成 `FirstCalcMatrix`。Rust 的初始 filter API 只覆盖上述 Chusan 构造完成时刻；任意独立 SRD、其他 target 或后续 Enable 切换仍保留为显式宿主输入。
