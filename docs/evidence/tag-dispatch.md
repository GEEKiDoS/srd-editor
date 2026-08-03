# SRD 标签分派证据

状态：下列父子标签分派已由当前游戏二进制中的四字节比较和直接调用目标证明。标签的业务含义仍需结合目标对象消费代码继续确认。

本文列出的 16 个主要分派函数已将 IDA 映像中的完整函数字节与当前 `chusanApp.exe` 的 PE 原始节数据逐字节比较，结果全部一致。IDA 数据库已为 38 个完成标签或基础读取闭环的函数加入 `srd_parse_*` / `vtbf_*` 保守名称并保存；没有命名尚未证明的属性语义。

## 顶层

| 输入标签/层级 | 解析函数 | 已证明的子标签 |
|---|---|---|
| `SRFF` 文件 | `0xA9F220` | 只对顶层 `SRCK` 调用 `0xA9F370` |
| `SRCK` | `0xA9F370` | `PROJ`；其内部再分派 `SCN `、`TEXL`、`FONT`、`CAM `、`CATR` |
| `SCN ` | `0xAA2FC0` | `LAYR` → `0xAA1FD0`；`ANMS` → `0xA9FB50` |
| `LAYR` | `0xAA1FD0` | `CAST` → `0xAA0130`；`ANIM` → `0xA9FD60` |

`0xA9F370` 仅在其当前子块为 `PROJ` 时分配项目记录并处理内部子块。因此 `PROJ` 是 `SRCK` 内的实际项目容器，而不是与 `SRCK` 平级的入口。

## CAST 数据

`CAST` 解析函数 `0xAA0130`：

| 子标签 | 直接目标 |
|---|---|
| `NODE` | `0xAA06F0` |
| `TRS2` | `0xAA0970` |
| `TRS3` | `0xAA0C10` |
| `DATA` | `0xAA0570` |
| `CATL` | `0xAA0380` |

`DATA` 解析函数 `0xAA0570`：

| 子标签 | 直接目标 |
|---|---|
| `CIMG` | `0xAA1410` |
| `CSLI` | `0xAA3320` |
| `CRFD` | `0xAA2DE0` |
| `CNUM` | `0xAA2440` |

进一步分派：

- `CIMG` (`0xAA1410`) 分别处理 `CREF`、`CRE1`、`TEXT`。
  - `CREF` 与 `CRE1` 都调用同一个记录解析函数 `0xAA1390`，但写入两个不同的目标数组。
  - `TEXT` 调用 `0xAA18F0`。
- `CNUM` (`0xAA2440`) 分别处理 `CREF`、`CRE1`；遇到 `TEXT` 时只执行标签比较，没有调用 `0xAA18F0` 或其他文本解析函数。
- `CSLI` (`0xAA3320`) 处理 `CREF` → `0xAA1390`、`SLIC` → `0xAA37A0`。
- `CATL` (`0xAA0380`) 处理 `CATR`。

后续运行时证据已经证明 CIMG 两表同时保留，并由 UV 通道参数最低位分别选择；详见 [`cimg-image-cast.md`](cimg-image-cast.md)。CNUM 的两表、SrNumberCast 建立和 glyph 映射见 [`cnum-number-cast.md`](cnum-number-cast.md)。

## 动画标签

| 输入标签 | 解析函数 | 子标签/目标 |
|---|---|---|
| `ANMS` | `0xA9FB50` | `SANM` → `0xAA3290` |
| `ANIM` | `0xA9FD60` | `MOT ` → `0xAA2290`；另处理 `CATR` |
| `MOT ` | `0xAA2290` | `TRK ` → `0xAA3FF0` |
| `TRK ` | `0xAA3FF0` | `KEY ` → 根据 format 分派多种记录解析器 |

关键记录的精确布局见 [`animation-records.md`](animation-records.md)。

## 纹理和字体

| 输入标签 | 解析函数 | 子标签/目标 |
|---|---|---|
| `TEXL` | `0xAA3E50` | `TEX ` → `0xAA3A70` |
| `TEX ` | `0xAA3A70` | `CROP` → `0xAA3CF0` |
| `FONT` | `0xAA1110` | `CHAR` → `0xAA12D0` |

`CAM ` 使用 `0xAA0040`，`CATR` 使用 `0xAA2AD0`。属性 code 与对象字段语义仍需逐函数闭环。

## 尚未证明

- 各标签名称对应的最终渲染业务语义。
- `NODE`、`TRS2`、`TRS3` 的世界变换组合顺序。
- TEXT 的完整运行时消费。
- CNUM 历史 glyph 动画，以及 CNUM、CSLI、TEXT 的最终绘制行为。
- 动画通道编号与 CAST 字段的绑定。
