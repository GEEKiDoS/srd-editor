# 3D SrTextCast / Fennel 矩阵与提交路径

分析对象为 `chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`。本页只记录 3D RFZ TextCast 相对已闭环 2D 路径的差异。

## 真实语料边界

完整 91 个 SRD 有 1,292 个 TextCast 定义，其中 1,285 个位于 2D LAYR、7 个位于 3D LAYR。7 个 3D 定义全部来自 `CHU_UI_LinkedVERSE_Gate_00.srd`：

- `SCN[0]/LAYR[0]/NODE[22,149,151]`；
- `SCN[0]/LAYR[5]/NODE[822,823,825,826]`。

引用展开后共有 2,819 个 TextCast runtime context，其中 2,704 个 2D、115 个 3D；没有 copied layer 改写源层 2D/3D 模式。全部 2,819 个 context 都解析到 RFZ。完整语料 `FontParamData vertical=True` 和 `$[0]..$[7]` runtime substitution 均为零，因此两项不能作为 3D 行为的替代解释。

`srd-runtime-state-audit --file=linkedverse --integer-frames --text-only` 对匹配的 6 个文件执行 17,324 个初始/ANMS 整数状态，检查 1,989,385 个 3D TextCast context。30,662 个通过 world enable/render gate，150 个继续通过二进制零颜色门控，并且同一 150 个全部通过命名 target 的精确四角投影/AABB 剔除。首个实际可绘制状态是：

```text
CHU_UI_LinkedVERSE_Gate_00.srd
SCN[0] / ANMS[10] / frame 1
LAYR[0] / NODE[149] TXT_rule
```

## `sub_AC5740` 写入 TextBox `+0x2EC`

`sub_AD9160` 以 `SrRenderer` 为 this，依次传入 SrTextCast world `+0x8C`、local transform `+0x5C`、SrImage `+0xF8`、text state `+0x1F4` 和 CAST 二维标志，最终进入 `sub_AC5740` (`0xAC5740`)。

`0xAC57CB..0xAC584F` 的两个分支为：

```text
2D: M = affine4x4(CAST world)

3D: M = (SrRenderer + 0x08) * affine4x4(CAST world)
    M[row][1] *= -1.0, row = 0..3
```

常量 `dword_181BFA0` 的字节为 `00 00 80 BF`，即精确 f32 `-1.0`。四次乘法作用于矩阵偏移 `+0x04/+0x14/+0x24/+0x34`，因此翻转的是完整 Y 列，不是平移行、单个轴向量或 glyph Y 坐标。结果在 `0xAC5844..0xAC584F` 原样复制到 `TextBoxObject+0x2EC`。

3D 分支中的 `SrRenderer+0x08` 是已闭环的 renderer camera bridge；命名 target 时为 `inverse(target ProjectionView) * SRD ProjectionView`，null target 构造初态保持 identity。

## CPU glyph 顶点与 packet 矩阵分流

`sub_7C7F90` (`0x7C7F90`) 先建立包含 TextBox position 和布局 `+0x108` 的局部平移矩阵，然后在 `0x7C80C9..0x7C8201` 分流：

- 2D：计算 `TextBox+0x2EC * local_translation`，把该结果用于 `sub_7C10B0` 的 CPU glyph 顶点；随后以 null 调用 `ceylon_vertex_builder_set_packet_matrix_from_object`，packet `c0..c7` 为 identity；
- 3D：`sub_7C10B0` 只收到 local translation；`ceylon_vertex_builder_set_packet_matrices(TextBox+0x2EC)` 把当前矩阵写入 packet `+0xA0`，上一矩阵写入 `+0xE0`，即 VS `c0..c3/c4..c7`。

每个 texture batch 仍以 format 13、triangle list 和调用方 `is_2d` 提交。`ceylon_submit_vertex_batch` (`0x6DF020`) 把 3D 的 false 清入 packet `+0x60 bit 7`，选择精确 key `AAMAAABAABGAAAAAAA`；D3D9 backend 因此上传 target `ProjectionView` 到 `c10..c13`。

该 key 复用的精确 332-byte `VS_03` 只读取 `c0..c3`、`c8`、`c10..c14`，没有任何 `c4..c7` 源操作数。因此 TextBox builder 保存的上一矩阵不会影响此 shader 的顶点结果，也不会参与已证明的当前矩阵相邻合并键。新建 runtime 的首次 draw 中上一矩阵仍精确为 identity。

## Rust 接入与 fixture

`fennel_textbox_transform` 复现上述乘法顺序和 Y 列翻转；`build_fennel_normal_vertex_batches` 的既有 3D 分支只对 CPU 顶点应用局部平移；runtime/initial Fennel builders 现在都保留有效 `is_2d`，选择相应 packet、shader、可见性与固定常量。

`CHU_UI_LinkedVERSE_Gate_00.srd` 的 `ANMS[10] frame 1 / LAYR[0]/NODE[149]` 回归得到 498 个 format-13 顶点，并固定完整 `c0..c3` f32 bit matrix：

```text
38AE9A75 80000000 00000000 3AE52AC0
00000000 BE23B0CD 00000000 C2641E2F
00000000 80000000 BF800347 44778657
00000000 80000000 BF800000 447A0000
```

这条 fixture 同时证明该 3D TextCast 在真实 ANMS 中可达，不能因为完整语料构造初态仍只有 557 个可见 2D Fennel draw 而继续排除 3D 分支。

当前没有把 Advertise/Common 的已证明宿主 profile 强套到 LinkedVERSE 来制造像素结论。2D Fennel 的 D3D9Ex/ResetEx 回归保持通过，3D format-13 shader、常量上传和 draw 路径与其共用同一 backend；但 LinkedVERSE 3D TextCast 的最终像素基线仍等待其实际 SrPlayer target、present/screen 参数由游戏二进制闭环后再固定。
