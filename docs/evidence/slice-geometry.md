# SrSliceCast 顶点与单元归一化坐标证据

本页只记录已经由 `surfride::SrSliceCast` RTTI、虚表入口和最终顶点写入共同闭环的几何结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`FE18BFE0E0F162713BA62AE1A3F17CA402768CC8F9CB640800743FDE05109D6A`

## 渲染入口归属

`srd_create_slice_cast` (`0xAC9EF0`) 把对象主虚表设为 `0x190DBF0`；该虚表的 MSVC RTTI 名称为 `surfride::SrSliceCast`。主虚表槽 `+0x1C` 保存 thunk `j_srd_render_slice_cast` (`0x447910`)，其唯一目标是 `srd_render_slice_cast` (`0xADA650`)。因此该函数的类归属不是根据相似渲染代码推测，而是由构造器、RTTI、虚表数据引用和 thunk 目标共同确认。

## 单元局部坐标

`srd_render_slice_cast` 先调用 `srd_get_cast_csli_cell_rects`，遍历其 20 字节运行时记录，并且只对 `active` 字节非零的记录调用虚表槽 `+0xBC`。SrSliceCast 对应实现是 `srd_build_slice_cell_quad_positions` (`0xADB2B0`)。

输入为单元 `x/y/width/height`，SrImage origin 位于 CAST `+0x178/+0x17C`。函数保持下列 f32 运算顺序：

```text
left   = x - origin_x
right  = left + width
far_y  = (height - origin_y) + y
near_y = y - origin_y
```

2D 分支：

```text
first_y  = near_y
second_y = far_y
```

3D 分支：

```text
second_y = -far_y
first_y  = height - far_y
```

四个局部顶点的固定顺序为：

```text
0 = (left,  first_y,  0)
1 = (left,  second_y, 0)
2 = (right, first_y,  0)
3 = (right, second_y, 0)
```

`srd_render_slice_cast` 随后按已有 CAST 世界 3x4 仿射矩阵逐顶点变换该结果。

## 单元归一化坐标

CAST `+0x184/+0x188` 是从 CSLI `0x40/0x41` 得到的 SrImage 总宽高。对每个 active 单元，`srd_render_slice_cast` 在 `0xADAAA8..0xADAB0A` 计算：

```text
u0 = (1.0 / image_width)  * cell.x
v0 = (1.0 / image_height) * cell.y
u1 = (cell.x + cell.width)  * (1.0 / image_width)
v1 = (cell.y + cell.height) * (1.0 / image_height)
```

与位置相同的四顶点顺序对应：

```text
0 = (u0, v0)
1 = (u0, v1)
2 = (u1, v0)
3 = (u1, v1)
```

这些值不会直接写入顶点 UV。`0xADAD14..0xADADB2` 把它们作为 X/Y 插值因子，对 CSLI 四个 packed color 做两级插值。最终顶点 UV 的来源见下一节。

Rust 的 `CsliDefinition::generate_active_quads` 复现上述 active 过滤、运算分组、顶点顺序、2D/3D Y 分支和单元归一化坐标顺序。

## 最终 UV 来源仍在追踪

`srd_render_slice_cast` 在 `0xADAB48..0xADAB5E` 以 SrSliceCast 为 `this` 调用 `sub_AD7EE0`，输入 SLIC flags 和若干运行时资源字段，并把结果写到栈上的 32 字节数组。`0xADAEA0..0xADAEE2` 从该数组按四组 8 字节读取坐标，并把同一对结果分别复制到顶点 `+20/+24` 和 `+28/+32` 两个 UV 通道。

因此目前可以确认“两个 UV 通道相同”和“最终 UV 来自 `sub_AD7EE0` 的资源相关输出”，但尚不能把 CSLI 单元归一化坐标命名为最终 UV。`sub_AD7EE0` 内部进一步调用 `sub_129B8D0`，并读取 CAST `+0x1A4..+0x1B0`、原始 SLIC flags 及运行时图像字段；该资源选择链仍需继续闭环。

## 仍未闭环

- SLIC `0x3A`、`0x33`、四个 `0x44` 和 CSLI 四个 `0x44` 共同生成两个 packed vertex color 的完整组合语义。
- CSLI `CREF`、CIMG `CREF/CRE1` 到实际纹理对象和最终 UV 的选择链。
- 图集资源自身的尺寸修正、采样状态、混合状态、索引顺序和最终 D3D9 draw call 参数。
