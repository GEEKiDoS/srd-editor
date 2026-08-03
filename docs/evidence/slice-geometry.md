# SrSliceCast 顶点与单元归一化坐标证据

本页只记录已经由 `surfride::SrSliceCast` RTTI、虚表入口和最终顶点写入共同闭环的几何结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`5B812E07874112C5C3F4C96E4293EC4E9203C4C6EE91CAD2F6C2902BAD3A622F`

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

## CREF 与最终 UV

`srd_render_slice_cast` 在 `0xADAB48..0xADAB5E` 以 SrSliceCast 为 `this` 调用 `srd_build_image_texture_coordinates` (`0xAD7EE0`)，输入 SLIC flags 和 SLIC `0x46`，并把结果写到栈上的 32 字节数组。`0xADAEA0..0xADAEE2` 从该数组按四组 8 字节读取坐标，并把同一对结果分别复制到顶点 `+20/+24` 和 `+28/+32` 两个 UV 通道。

该函数用参数 `0` 选择 SrImage 中的 CSLI CREF 指针与数量，并调用 `srd_resolve_cref_texture_coordinates` (`0x129B8D0`)。后者执行：

```text
selector = signed SLIC 0x46
if selector < 0 or selector >= CSLI 0x45 count: no CREF rectangle
entry = CREF[selector]
image_index     = entry.signed_i16[0]
rectangle_index = entry.signed_i16[1]
```

若两个下标均非负，运行时图像表使用 540 字节步长；记录 `+532` 指向 16 字节矩形表，并读取 `rectangle_index` 对应的四个 f32 端点。输出开头的 signed i16 是 `image_index`，随后是四对最终纹理坐标。

设矩形为 `[x0,y0,x1,y1]`。SLIC flags `0x10` 交换 X 端点，`0x20` 交换 Y 端点；flags `& 0xC0` 再选择固定顺序：

```text
0x00: (x0,y0) (x0,y1) (x1,y0) (x1,y1)
0x40: (x1,y0) (x0,y0) (x1,y1) (x0,y1)
0x80: (x1,y1) (x1,y0) (x0,y1) (x0,y0)
0xC0: (x0,y1) (x1,y1) (x0,y0) (x1,y0)
```

因此两个最终 UV 通道的选择和顺序已经闭环；CSLI 单元归一化坐标仍只用于 packed color 插值。Rust 现在解析 CREF 的两个 signed i16、按 SLIC `0x46` 选择记录，并由 `slice_texture_coordinates` 复现所有 flip/order 分支。实际纹理资源对象和 16 字节矩形表的文件来源仍需继续追踪。

53 个本地 SRD 中，799 个 CSLI 共解析出 4813 条 CREF；按每个 SLIC 的 signed `0x46` 和声明数量执行与游戏相同的边界检查后，有 4997 个单元能够选择到一条实际 CREF 记录。

## 仍未闭环

- SLIC `0x3A`、`0x33`、四个 `0x44` 和 CSLI 四个 `0x44` 共同生成两个 packed vertex color 的完整组合语义。
- 运行时图像对象及其 16 字节矩形表如何由 CIMG/外部纹理资源建立。
- 图集资源自身的尺寸修正、采样状态、混合状态、索引顺序和最终 D3D9 draw call 参数。
