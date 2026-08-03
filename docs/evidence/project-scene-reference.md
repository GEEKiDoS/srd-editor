# PROJ、SCN 场景表与 CRFD 引用目标证据

本页闭环 `CRFD 0x80/0x81` 的查找范围。结论来自当前游戏二进制的解析、运行时建立和引用绑定三条连续调用链，不使用文件名相关性推断。

分析对象：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`5ACFF2969D10D0840FEA516DCB20699324E2286DDB00E6C404E862F77B695074`。

## 文件内项目和场景表

`srd_parse_srff` (`0xA9F220`) 先确认文件 format 为 `SRFF`，再只把顶层 `SRCK` 交给 `srd_parse_srck` (`0xA9F370`)。后者只在 `SRCK` 的直接子块为 `PROJ` 时建立项目记录。

`PROJ` 的属性 `0x00` 以 u16 保存到项目记录 `+0x44`。解析结束后，二进制按 `count * 0x68` 分配连续记录，并把每个直接 `SCN ` 子块依次交给 `srd_parse_scn` (`0xAA2FC0`)。因此该数组是当前同一 SRD 文件的场景表。

`srd_parse_scn` 对引用解析相关字段的直接写入为：

| 属性 | SCN 记录偏移 | 用途 |
| --- | --- | --- |
| `0x03` | `+0x00`, `char[0x40]` | 场景名 |
| `0x10` | `+0x44`, u32 | 直接 `LAYR` 数量 |
| `0x17` | `+0x54`, u32 | 直接 `ANMS` 数量 |

随后它按 `0x9C` 字节为每个直接 `LAYR` 分配记录，并调用 `srd_parse_layr`。这里没有从 SCN 名生成路径，也没有加载另一个 SRD。

## 运行时表的建立

SRD 加载路径 `sub_AAAFE0` (`0xAAAFE0`) 遍历上述 `PROJ` 场景数组，为每个 `SCN ` 分配一个运行时条目并调用 `srd_init_runtime_scene_from_scn` (`0xAC2B80`)。该函数：

1. 把解析 SCN 指针写到运行时条目 `+0x04`；
2. 把 SCN `char[0x40]` 名称复制为运行时条目 `+0x0C` 的字符串；
3. 遍历 SCN `+0x44/+0x48` 的 LAYR 数组，逐层建立运行时层并保存到条目 `+0x24` 的向量。

这些条目被加入 `SrPlayer::Impl+0x294`。`srd_player_get_runtime_scene_table` (`0xAAAFA0`) 直接返回这一地址；多个公开句柄解析函数也以它作为最高层场景索引表。

## CRFD 的两级精确查找

`srd_resolve_reference_scene_links` (`0xAA9450`) 创建内部名为 `ReferenceScene` 的辅助场景，随后遍历全部运行时 CAST，并把同一个 `SrPlayer::Impl+0x294` 场景表传给 CAST 虚表 `+0x0C`。

SrRefCast 的该槽进入 `srd_resolve_reference_cast_resource` (`0xADB550`)：

1. 从头遍历运行时场景表；
2. 将 CRFD `+0x000` 的 `source_name` 与运行时 SCN 名按字节比较，并额外要求长度完全相等；
3. 首次相等即停止场景遍历；
4. 将 CRFD `+0x200` 的 `layer_name` 传给 `srd_find_runtime_layer_by_name` (`0xAC2A20`)；
5. 该函数从头遍历所选 SCN 的运行时 LAYR 向量，同样按字节和完整长度匹配，返回第一个相等层；
6. 找到层后才建立 SrRefCast 的引用运行时对象。

查找是区分大小写的完整字节匹配。二进制没有扩展名拼接、相对目录、同目录探测或外部资源根搜索。

## Rust 实现和语料回归

Rust 现在实现：

- `SRFF -> SRCK -> PROJ -> SCN  -> LAYR` 的已证明层级；
- PROJ/SCN 声明数量与直接子块数量验证；
- SCN 64 字节名称和直接 LAYR 表；
- `Project::resolve_reference` 的首次、完整、区分大小写的两级匹配。

本地 53 个 SRD 共解析出 192 个 `SCN `；1090 个 CRFD 全部在其所属文件的项目场景表内解析到 SCN 和 LAYR，未使用任何外部路径回退。

独立引用层的构造、嵌套解析、世界状态和递归更新/绘制入口见 [`reference-runtime-recursion.md`](reference-runtime-recursion.md)。复制层的完整动画对象和最终 D3D9 draw submission 仍待后续闭环。
