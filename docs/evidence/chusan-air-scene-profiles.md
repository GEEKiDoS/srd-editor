# Chusan `air::Scene` target profiles

本文只记录 `chusanApp.exe` 中 Chusan 自己创建的 `MainScene` 与 `BgScene`。它们是可供编辑器显式选择的 target profile，不是独立 SRD 的自动默认宿主。Advertise 的空 `TargetScene` 仍按 [`render-target-routing.md`](render-target-routing.md) 所述进入全局队列，并可能被多个已注册 target 接收。

## 实际创建点

Chusan 构造路径 `sub_AE4660 -> sub_AE52B0` 创建两个 1752-byte `air::Scene`：

| target | 名称构造 | 保存位置 | 注册调用 |
| --- | ---: | ---: | ---: |
| Main | `0xAE52F9..0xAE5339` | owner `+0x54` | `0xAE5377..0xAE5381` |
| Background | `0xAE540B..0xAE544B` | owner `+0x5C` | `0xAE546E..0xAE5478` |

两次构造都调用 `sub_4085C6 -> sub_7023D0`，因此类型确实是 `air::Scene`，不是根据字符串推测。`sub_7023D0` 安装 `air::Scene` 虚表 `0x18E30B0`，并在 target `+0x90` 创建 432-byte `air::Camera`。

注册 thunk `sub_43BB2E -> sub_602210 -> sub_46524E -> sub_6310E0 -> sub_673F10` 最终把 target 按其运行时名称插入管理器 `+0x114` 的 map。`MainScene` 的参数是 order `10000`、start enabled `true`；`BgScene` 是 order `9900`、start enabled `false`。

## 已闭环的 scene 属性

`sea::BasicScene` 构造函数 `sub_5FFF60` 注册了属性编号与名称；Chusan 随后的 setter 因而可以按编号精确解释：

| 属性 | MainScene | BgScene | 证据 |
| --- | ---: | ---: | --- |
| `PresentIndex` | 0 | 0 | 两次 `air::Scene` 构造的第二参数 |
| `DrawIndex` (property 2) | 0 | 16 | `sub_424B1D -> sub_604440` 固定写 property 2 |
| `PresentMode` (property 4) | 1（构造默认） | 0 | `sub_43B97B -> sub_604850` 固定写 property 4 |
| `ShaderOnDemand` (property 7) | false | false | `0xAE5390..0xAE53A1` / `0xAE5491..0xAE54A2` |
| `RequestColorOffscreen` (property 14) | true | true | `0xAE53A6..0xAE53B7` / `0xAE54A7..0xAE54B8` |
| `RequestDepthOffscreen` (property 15) | true | true | `0xAE53BC..0xAE53CD` / `0xAE54BD..0xAE54CE` |
| `Clear` | false | false | target `+0x298` 在 `0xAE53D2..0xAE53D5` / `0xAE54D3..0xAE54DB` 清零 |

MainScene 还在 `0xAE535C..0xAE5371` 通过虚表 `+0x80` 把 `{12, 0}` 写入 target `+0x290/+0x294`。写入位置和数值已确认，但字段语义尚未闭环，因此代码和文档不为它命名。

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
- `projection_view_for_present_size(width, height)`。

该函数只恢复 target Camera 的 `Projection * View`。它不生成 `FirstCalcMatrix`，也不宣称 Advertise 一定由 MainScene 接收；scene-node 根矩阵、当帧 active target 集合和 target bit/filter 仍是独立宿主输入。
