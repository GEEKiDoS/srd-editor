# SRD 格式与游戏逻辑证据账本

旧 Python 编辑器、旧报告和会话摘要只提供待调查线索，不构成格式或运行时语义证据。本文件中的每项结论必须最终附带游戏二进制地址、反编译/汇编依据和对应样本验证；未完成闭环的内容一律标记为“待验证”，不能直接进入 Rust 实现。

## 证据等级

- **已证明**：游戏二进制中的读取/写入/分派代码与至少一个实际样本能够互相印证。
- **部分证明**：已经定位二进制代码，但结构字段、调用上下文或样本行为尚未闭环。
- **待验证线索**：仅来自旧实现、旧文档、字符串、标签名或样本相关性。
- **未知**：没有足够证据，不赋予语义。

已完成证据闭环的内容：

- VTBF 读取逻辑：[`evidence/vtbf-reader.md`](evidence/vtbf-reader.md)
- 标签分派图：[`evidence/tag-dispatch.md`](evidence/tag-dispatch.md)
- 动画记录布局：[`evidence/animation-records.md`](evidence/animation-records.md)
- LAYR/NODE/TRS、公共动画通道和矩阵链：[`evidence/scene-transform.md`](evidence/scene-transform.md)
- CSLI/SLIC 网格、NODE `0x32` 与父级单元偏移：[`evidence/csli-layout.md`](evidence/csli-layout.md)
- SrSliceCast active 单元局部顶点、CREF 选择与最终 UV：[`evidence/slice-geometry.md`](evidence/slice-geometry.md)
- CIMG、CREF/CRE1 双通道与 SrImageCast：[`evidence/cimg-image-cast.md`](evidence/cimg-image-cast.md)
- CNUM 解析、SrNumberCast 初值与 glyph 映射：[`evidence/cnum-number-cast.md`](evidence/cnum-number-cast.md)
- SrImage 尺寸 `11/12`、顶点色 `13..16` 与双坐标描述符 `17/20` 动画：[`evidence/image-coordinate-animation.md`](evidence/image-coordinate-animation.md)
- CRFD、SrRefCast 与引用动画帧通道 `23`：[`evidence/crfd-reference-cast.md`](evidence/crfd-reference-cast.md)
- TEX `0x62`、双采样包装对象、SrSliceCast 选择与 D3D9 采样状态：[`evidence/texture-binding.md`](evidence/texture-binding.md)
- TEXL/TEX/CROP 记录、归一化矩形与外部 DDS 路径：[`evidence/texture-table.md`](evidence/texture-table.md)

下述尚未附带该等级证据的 SRD 语义仍按“待验证线索”处理。

## 容器（待验证线索）

文件头：

```text
VTBF magic
unknown_04: 4 bytes
format: [u8; 4]，通常为 SRFF
block_count: u16
unknown_0e: 2 bytes
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

游戏读取器已经证明魔数、format、顶层块数和块布局；`unknown_04` 与 `unknown_0e` 在已定位入口中没有被读取，不能沿用旧解析器的 `version`/`header_extra` 命名。写入规则与未知字节语义尚未证明。

## VTBF 属性（待验证线索）

属性的边界编码、类型大小表、count/mult 扩展和字符串长度前缀已经由游戏读取器证明，详见证据文档。字符串内容在该层仅按字节复制；是否要求 UTF-8 并未由读取器证明。

- 长度小于 127 字节时使用单字节长度。
- 长度为 127 至 32767 字节时使用双字节长度。
- 该读取器的两字节形式只能表达至 `0x7FFF`。

属性类型的业务语义仍需由各标签的消费代码逐项证明；不能仅凭宽度给 type code 命名。

当前 53 个样本使用旧 Python 解析器进行未编辑往返时，有 5 个文件并非字节一致。已观察到其中至少一种原因是短字符串也可能使用双字节长度前缀，而旧序列化器会改写为单字节形式。这只证明“原始编码必须保留”，并不证明完整字符串规则。

## 动画

当前二进制已经重新证明解析链：

```text
sub_A9FD60              Animation
  -> sub_427183         thunk
  -> sub_AA2290         TRK
  -> sub_457D1A
  -> sub_AA3FF0         KEY
```

但 KEY 记录不是固定 20 字节；游戏根据 TRK format 分派为 8 或 20 字节。20 字节分支之一的布局为：

```text
+0x00 frame
+0x04 value
+0x08 tangent_type
+0x0C tangent_in
+0x10 tangent_out
```

具体分派、时间折叠、端点、线性、保持和三次曲线公式已经由运行时求值器证明，详见证据文档。公共空间通道 `0..8` 与 visibility 通道 `10` 也已证明并实现；颜色通道和 CAST 专属通道仍不得猜测。

## 场景变换

LAYR flags 位 0、NODE/TRS2/TRS3 记录、公共运行时变换、游戏三角函数、局部 3x4 仿射矩阵以及 `parent_world * local` 世界组合顺序已经完成二进制闭环，详见 [`evidence/scene-transform.md`](evidence/scene-transform.md)。

NODE `0x3C/0x3D` 的首子/同级链以及根节点选择已经闭环并实现。NODE `0x32` 在父节点 `surfride::SrSliceCast` 时索引父级 CSLI 生成单元；尺寸、origin mode、自定义 origin、显式单元累计、越界、active、2D/3D 分支和中心偏移均已闭环并实现。

## 投影与视口

`surfride::SrPlayer -> SrPlayer::Impl -> SrRenderer` 的运行时对象链、`SrRenderer+0x48` 屏幕矩阵、Width/Height 属性、D3D 风格视口矩阵、精确 row-major 4x4 乘法以及 CAST 四角的二维直通/三维透视除法已经完成二进制闭环并实现，详见 [`evidence/projection.md`](evidence/projection.md)。

游戏在构造 `SrRenderer+0x48` 时已经乘入 `[0,width] x [0,height]` 的像素视口映射和 Y 反转，因此三维 CAST 在除以 W 后得到最终屏幕 X/Y，不存在另一个尚未执行的 NDC-to-viewport 步骤。上游 camera/backend 两个输入矩阵的类方法语义仍待继续命名和追踪。

## 图像与特殊 CAST

- CSLI/SLIC 的解析、NODE 链接、type 2 分派、网格生成、父级偏移、active 单元局部四顶点以及用于 packed color 插值的单元归一化坐标已经闭环，详见 [`evidence/slice-geometry.md`](evidence/slice-geometry.md)。
- CSLI CREF 每条记录的两个 signed i16、SLIC `0x46` 选择、运行时图像/矩形下标、flags flip/order 以及两个相同最终 UV 通道已经闭环。
- CSLI/SLIC packed color 的零默认、双线性插值、逐通道乘法和饱和加法公式已经闭环；CAST 两种 tint 的来源仍未闭环。
- TEXL/TEX/CROP 到运行时 16 字节归一化矩形表以及 `.dds` 路径构造已经闭环；实际纹理对象和最终 draw call 状态仍未闭环。
- CIMG 的 CREF/CRE1 双表保存、选择器、TEXL/CROP 坐标解析、坐标偏移、Point/Linear 选择与 Image/Text cast 分类已经闭环；TEXT 内部仍未完成。
- CNUM 的结构布局、CREF/CRE1 双表、NumberCast 初值、完整静态格式化、数字及四种特殊字符的 glyph 映射、逐字符 quad 排版和每 glyph 纹理坐标已经闭环；历史 glyph 动画与最终绘制状态仍未完成。
- CAST 专属通道 `11/12` 的尺寸/origin、`13..16` 的 packed vertex color、`17/20` 的 selector/CREF/CRE1，以及 SrRefCast 专属 `23` 的 CRFD 动画帧请求已经闭环；引用 SRD 的加载与递归应用尚未完成。
- DDS 图集裁剪、runtime-bound 资源、数字排版与 sliced sprite 均不得依据旧预览器的表现直接实现。

## 待验证问题

1. VTBF/SRFF 文件头、块长度、子块布局和全部属性编码。
2. 各标签的构造/解析分派函数及运行时对象类型。
3. 公共 packed color/alpha 通道，以及通道 `23` 请求后的引用 SRD 加载与递归应用。
4. 投影屏幕矩阵上游 camera/backend 输入及 3D 深度、裁剪提交逻辑。
5. CNUM 历史 glyph 动画、TEXT 与 shader/固定管线中的双 UV 消费流程。
6. CNUM/NumberCast 动画通道、CAST tint/default color 与最终 D3D9 绘制语义。
