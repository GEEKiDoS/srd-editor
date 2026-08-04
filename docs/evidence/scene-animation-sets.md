# SCN `ANMS` / `SANM` 场景动画集与运行时页面选择

状态：文件布局、运行时构造、layer gate、动画查找、动画集 identity 解码，以及 Chusan AdvertiseLogo 的六阶段入口/出口表均已由当前游戏二进制闭环。编辑器不再把同一 SCN 的所有互斥页面同时提交。

分析对象：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- IDA 会话：`chusan_srd_3`；
- 实测 SRD：`surfboard/advertise/CHU_UI_Advertise_00_v10.srd`。

## 文件记录

`srd_parse_anms` (`0xA9FB50`) 建立 `0x50` 字节记录：

| 属性 | 记录偏移 | 已证明行为 |
| --- | --- | --- |
| `0x03` | `+0x00`, `char[0x40]` | 动画集名称 |
| `0x18` | `+0x40`, i32 | 选择动画集时写入各动画的初始 frame |
| `0x19` | `+0x44`, i32 | runtime duration |
| `0x0E` | `+0x48`, i32 | `SANM` 记录数 |
| — | `+0x4C` | 连续 `SANM[0x44]` 数组 |

`srd_parse_sanm` (`0xAA3290`) 建立 `0x44` 字节记录：

| 属性 | 记录偏移 | 默认值 | 已证明行为 |
| --- | --- | --- | --- |
| `0x03` | `+0x00`, `char[0x40]` | 空串 | 在同下标 runtime LAYR 中区分大小写查找 ANIM |
| `0x0F` | `+0x40`, i32 | `1` | 非零时启用同下标 runtime LAYR |

SCN 解析记录 `+0x54` 是直接 `ANMS` 数，`+0x58` 指向步长 `0x50` 的 ANMS 数组。Rust 现在保存 `Scene::animation_sets`，并验证 SCN/ANMS/SANM 的声明数与直接子块数。

## 运行时构造

`srd_init_runtime_scene_from_scn` (`0xAC2B80`) 先在 runtime scene `+0x24` 建立 LAYR 向量，再按 SCN `+0x54` 遍历 ANMS，为每项分配 `0x30` 字节 `surfride::SrAnimationSet` 并加入 runtime scene `+0x30` 向量。

`SrAnimationSet` 构造函数 `0xAD7030`：

1. 取 `min(ANMS.SANM_count, runtime_layer_count)`；
2. 按相同下标把每个 SANM 与 runtime LAYR 配对；
3. 保存 SANM 动画名；
4. 调用 `srd_find_animation_by_name`，保存找到的 runtime ANIM 指针；
5. 不存在或为空的名字保存空动画指针，不回退到“第一个动画”。

`sub_AC2FD0` 选择一个 SrAnimationSet 时：

1. 撤销前一个动画集的运行时动画状态；
2. 把当前 SrAnimationSet 写到 runtime scene `+0x70`；
3. 读取 ANMS `0x18`，以同一 raw frame 初始化动画集和每个已找到的 ANIM；
4. 读取 ANMS `0x19` 写入 runtime scene `+0x5C`；该地址相对内嵌 runtime animation `+0x3C` 正好是 `+0x20`，与已闭环的 runtime duration 字段一致；
5. 对每个 SANM 调用 `sub_AD7760`，把 `(SANM+0x40 != 0)` 直接写到对应 runtime LAYR `+0x168`；
6. 对非空 ANIM 立即调用 `srd_set_animation_frame_raw` 和动画通道应用器。

所以 `ANMS` 不是仅供编辑器分组的元数据，而是场景级同步动画与互斥 layer gate。未选择 ANMS 时把所有 LAYR 同时绘制，不符合游戏运行时。

## identity 解码

SrCtrl 的动画集查找路径为 `sub_ACB1A0 -> sub_AA51A0`。对 32 位 identity：

```text
scene_index         = ((identity >> 24) & 0x3F) - 1
animation_set_index = (identity & 0x3FF) - 1
```

两个索引都先做范围检查，再从 `SrPlayer::Impl+0x294` 的 runtime scene 表和 runtime scene `+0x30` 的 SrAnimationSet 向量取对象。`sub_ACCAD0` 找到对象后经 `sub_AD7790 -> sub_AC2FD0` 选择它；`sub_ACBFD0 -> sub_AD7770 -> sub_AC2F60` 查询同一 identity 是否已经结束。

## AdvertiseLogo 六阶段表

`sub_C93B10` 以 `5 * stage` 读取 `0x1919378` 的六条五 dword 记录。前两个 dword 分别传给 SrCtrl 作为入口/出口动画集 identity：

| stage | 入口 identity → ANMS | 出口 identity → ANMS | 第五 dword |
| --- | --- | --- | --- |
| 0 | `0x41000001` → `[0] AS_warning_in` | `0x41000002` → `[1] AS_warning_out` | `10000` |
| 1 | `0x41000003` → `[2] AS_sega_in` | `0x41000004` → `[3] AS_sega_out` | `2000` |
| 2 | `0x41000005` → `[4] AS_allnet_in` | `0x41000006` → `[5] AS_allnet_out` | `2000` |
| 3 | `0x41000007` → `[6] AS_rights_in` | `0x41000008` → `[7] AS_rights_out` | `0` |
| 4 | `0x41000009` → `[8] AS_attention_in` | `0x4100000A` → `[9] AS_attention_out` | `2000` |
| 5 | `0x4100000B` → `[10] AS_title_in` | `0x4100000D` → `[12] AS_title_out` | `0` |

记录的第三、第四 dword 只在 stage 4/5 非零，并被别的宿主系统函数消费；当前页面 gate 实现不为其猜测名称或 SRD 语义。`AS_title_loop` 是 ANMS `[11]`，但不在这张入口/出口 identity 对中，因此也没有被擅自插入阶段序列。

`sub_C931E0` 的 common-init 后状态路径先启用 SrPlayer，再从这张表选择 `stage` 的入口 identity；stage 初值先写 `0`，某个外部条件可改为 `1`。独立编辑器没有该外部游戏状态，因此 UI 明确提供 ANMS 选择器，不声称能自动决定真实机台当前 stage。

## Rust 与 GPU 回归

Rust 当前实现：

- 解析并保存 ANMS/SANM；
- `ProjectRuntime::apply_animation_set` 按 SANM 下标设置 layer gate，并以同一 frame 应用命名 ANIM；
- Composition 使用显式选择的 ANMS 与时间轴 frame 构建 draw；
- 原先的“所有初始 LAYR”构建器仅保留给底层诊断与单贴图 smoke，不再作为正常页面预览。

本地 53 个 SRD 的回归统计：

```text
ANMS sets=764
SANM slots=3688
enabled slots=1572
named slots=1612
unresolved named animations=0
```

AdvertiseLogo 的确定性回归：

- `ANMS[0] AS_warning_in`, frame 30：证据完整 draw 只来自 LAYR 0/5；
- `ANMS[13] AS_movie_in`, frame 20：恰好得到 LAYR 4 / NODE 4 的贴图 draw；
- D3D9Ex `--srd-draw-smoke`：1920×1080 共 `2,073,600` 个 changed pixels，`white_pixels=0`，ResetEx 前后 FNV-1a 均为 `97D30483E5DD6325`；
- D3D9Ex 单贴图 smoke 仍为 `849,776` changed pixels、`white_pixels=0`、FNV-1a `09DF61BBE19B88A5`，ResetEx 前后一致。

当前页面中 TEXT 等尚未完成的 CAST 仍不会被虚构渲染；例如 warning 页已不再全白，但在 TextCast 闭环前只能显示已支持的背景 draw。
