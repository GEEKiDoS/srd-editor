# SrSliceCast 顶点与单元归一化坐标证据

本页只记录已经由 `surfride::SrSliceCast` RTTI、虚表入口和最终顶点写入共同闭环的几何结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`B874FB682B5F963DB72E0660A6D9D7CFA061A787851C652F8A09056AB1779077`

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

因此两个最终 UV 通道的选择和顺序已经闭环；CSLI 单元归一化坐标仍只用于 packed color 插值。Rust 现在解析 CREF 的两个 signed i16、按 SLIC `0x46` 选择记录，并由 `slice_texture_coordinates` 复现所有 flip/order 分支。16 字节矩形表已经进一步闭环到 TEXL/TEX/CROP，详见 [`texture-table.md`](texture-table.md)；实际 D3D9 纹理资源对象仍需继续追踪。

53 个本地 SRD 中，799 个 CSLI 共解析出 4813 条 CREF；按每个 SLIC 的 signed `0x46` 和声明数量执行与游戏相同的边界检查后，有 4997 个单元能够选择到一条实际 CREF 记录。

## 两个 packed vertex color

`srd_render_slice_cast` 对每个顶点先加载 CSLI 的四个 `0x44`。它用该顶点的单元归一化 Y 对 `0/1` 和 `2/3` 两对颜色分别调用 `srd_lerp_color_u8` (`0x5FEFA0`)，再用归一化 X 对两项结果插值，形成双线性 CSLI 基色。该插值逐通道使用 f32：

```text
value = first * (1.0 - factor) + second * factor
result = truncate(clamp(value, 0, 255))
```

随后两个写入顶点的 packed color 为：

```text
primary = SLIC per-vertex 0x44
        * SLIC 0x3A
        * bilinear CSLI 0x44
        * CAST multiplicative tint

secondary = saturating_add(CAST additive tint, SLIC 0x33)
```

每次乘法都调用 `srd_multiply_color_u8` (`0x5FEA70`)，逐通道执行 `u32(a) * u32(b) / 255`；加法调用 `srd_add_color_saturating_u8` (`0x5FEB90`)，和大于等于 255 时写 255。primary/secondary 分别写入 36 字节顶点的 `+12` 和 `+16`。

Rust 已实现这些精确组合器、世界 CAST 乘法色/加法色来源，以及生成 36 字节双 UV 顶点的 `build_slice_render_quad`。

样本方面，5145 个 active SLIC 全部有 `0x3A` 和恰好四个 `0x44`，但全部没有显式 `0x33`。缺省值由 `SrProject` 虚表槽 `+4` 的 `srd_project_allocate_zeroed` (`0xA9F310`) 闭环：它对每次请求的完整分配区执行 `memset(pointer, 0, size)`，然后才返回给 CSLI/SLIC 解析器。因此缺失 `0x3A/0x33/0x44` 对应的运行时字节均为零，包括 104 字节 CSLI 模板复制范围之外的后续单元。Rust 的 runtime color 访问器复现该零默认。

## 仍未闭环

- DDS 资源建立与设备丢失恢复。
- shader、混合、深度和裁剪状态；36 字节顶点、双 UV、triangle strip 与最终 `DrawPrimitive` 参数已经闭环。
