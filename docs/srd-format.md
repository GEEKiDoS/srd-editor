# SRD 格式与游戏逻辑调查记录

本文件只记录已经由样本、现有解析器或 IDA 调查支持的结论。未确认内容明确标为待验证，不能作为格式规范直接固化。

## 容器

文件头：

```text
VTBF magic
version: u32
format: [u8; 4]，通常为 SRFF
block_count: u16
header_extra: u16
blocks...
```

块：

```text
sub_sig: [u8; 4]，常见 vtc0
block_size: u32
tag: [u8; 4]
child_count: u16
prop_count: u16
properties...
children...
```

现有 Python 实现通过重建整棵树完成序列化；TEXT 变长时会自然更新字符串长度、属性编码、块大小和后续布局，不依赖原位覆盖。

## VTBF 属性

属性包含 code、flags、可选扩展字节和值。flags 的低位编码类型，高位参与 count/mult。字符串为 UTF-8：

- 长度小于 127 字节时使用单字节长度。
- 长度为 127 至 32767 字节时使用双字节长度。
- 当前证据不支持长度大于等于 `0x8000` 的字符串。

i32/f32 存在项目特有的紧凑编码，Rust 实现必须先逐项移植并用二进制往返测试验证，不能直接假设全部为普通小端定长值。

## 动画

IDA 中已经确认的解析链：

```text
sub_A9FD60              Animation
  -> sub_427183         thunk
  -> sub_AA2290         TRK
  -> sub_457D1A
  -> sub_AA3FF0         KEY
```

KEY 记录大小为 20 字节：

```text
+0x00 frame
+0x04 value
+0x08 tangent_type
+0x0C tangent_in
+0x10 tangent_out
```

已观察的通道包括位置、旋转、缩放、可见性、尺寸、cell index、alpha 和颜色候选通道。精确 spline 求值、`end_frame` 语义和部分颜色通道仍待游戏侧代码确认。

## 图像与特殊 CAST

- CIMG 使用 CREF/CRE1 引用图集中的 crop，不能默认绘制整张 DDS。
- CRE1 是独立的第二引用表，不能无条件替换 CREF。
- CNUM 会使用数字 cell 序列；spacing、对齐、负号和小数规则仍待确认。
- CSLI 表现为 sliced/9-patch 类图像，但固定边、拉伸区域和对齐规则仍待确认。
- 无显式 CREF 的 runtime-bound CIMG 可能由游戏运行时注入资源，离线预览器不应随意猜测纹理。

## 待验证问题

1. 游戏真实 spline/Hermite 公式及 tangent 单位。
2. 轨道超过 `end_frame` 后的行为。
3. TRS2 属性 `0x3B` 的准确含义。
4. CIMG 的 size、crop、pivot 与节点变换的组合顺序。
5. CNUM 的数字选择、spacing、scale 和对齐。
6. CSLI 的精确切片与拉伸语义。

