# 投影、视口与最终屏幕坐标证据

本页只记录从游戏二进制闭环得到的结论。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`17D48EAB2573608708392E6BE81094C1E25DF0A7EF2E5BEA1817DF39F5A676F8`

## 运行时对象链

`srd_construct_player` (`0xAA68B0`) 分配 `0x2E8` 字节并调用 `srd_construct_player_impl` (`0xAA67A0`)，随后把结果保存到 `surfride::SrPlayer + 0xE8`。MSVC RTTI 对该对象给出的原始类型名为：

```text
.?AVImpl@SrPlayer@surfride@@
```

即 `surfride::SrPlayer::Impl`。Impl 构造器在 `Impl + 0x10` 构造一个对象；其 vtable `off_190D870` 的 RTTI 原始类型名为：

```text
.?AVSrRenderer@surfride@@
```

即 `surfride::SrRenderer`。因此 `srd_player_get_renderer` (`0xAAAF80`) 的返回式：

```text
*(SrPlayer + 0xE8) + 0x10
```

不是推测出的偏移，而是 `surfride::SrRenderer *`。`srd_build_runtime_layer` 把 `SrPlayer *` 保存到 runtime layer `+0x138`，所以每个 CAST 可以沿 `CAST+0x2C -> layer+0x138` 取得同一个播放器及其 renderer。

## SrRenderer 中的矩阵

`srd_construct_renderer` (`0xAC4010`) 使用 `srd_identity_matrix4x4` (`0x632230`) 初始化：

```text
SrRenderer + 0x08: 4x4 identity
SrRenderer + 0x48: 4x4 identity
```

`srd_prepare_player_renderer` 的未命名主体 `0xAACA80` 最终调用 `0xAC7400` 配置 renderer。该函数末尾证明 `SrRenderer+0x48` 的构造顺序：

```text
viewport = viewport_matrix(width, height)
temporary = viewport * pre_viewport_transform
screen_matrix = temporary * *(Matrix4x4 *)(SrRenderer + 0x08)
*(Matrix4x4 *)(SrRenderer + 0x48) = screen_matrix
```

这里保留 `pre_viewport_transform` 与 `renderer_transform` 这两个中性名称。二者的精确矩阵乘法关系已经闭环；产生它们的上游 camera/backend 虚函数语义尚未全部命名，因此当前代码没有把它们擅自称作 view 或 projection matrix。

## Width、Height 与视口矩阵

函数 `0x5FFF60` 明确注册：

```text
property 9  = "Width"
property 10 = "Height"
```

`0x603FA0` 返回 property 9，`0x603DD0` 返回 property 10。`0xAC7400` 把这两个 signed i32 通过 `cvtdq2ps` 转为 f32，并从单位矩阵构造：

```text
[ width *  0.5, 0,             0, width  * 0.5 ]
[ 0,            height * -0.5, 0, height * 0.5 ]
[ 0,            0,             1, 0            ]
[ 0,            0,             0, 1            ]
```

常量内存已按 f32 读取验证：

```text
0x1796960 =  0.5
0x18A5C84 = -0.5
```

因此它把 `[-1,+1]` 的 X 映射到 `[0,width]`，把 `+1..-1` 的 Y 映射到 `[0,height]`，同时执行 D3D 屏幕坐标所需的 Y 反转。它不是投影后的另一条待实现步骤；它已经被乘入 `SrRenderer+0x48`。

## 4x4 乘法顺序

`srd_mul_matrix4x4` (`0x604010`) 使用四行 SIMD 数据。对每个输出行和每一列，其计算分组严格为：

```text
(lhs.w * rhs.row3 + lhs.z * rhs.row2)
  +
(lhs.y * rhs.row1 + lhs.x * rhs.row0)
```

这等价于 row-major `lhs * rhs`，但 Rust 实现仍保留二进制的乘法、加法分组，避免把可观察的 f32 舍入顺序改写成另一种数学等价式。

`srd_copy_matrix4x4` (`0x6014A0`) 逐 dword 复制 16 项；`srd_identity_matrix4x4` 逐项写入标准 4x4 单位矩阵。这三个函数共同闭环了 `0xAC7400` 末尾的矩阵构造。

## CAST 四角到屏幕坐标

`srd_project_cast_corners_to_screen` (`0xAD3B00`) 固定处理 CAST `+0xC8` 的四个世界空间角点，每点 stride `12` 字节。

二维 CAST 分支调用 CAST virtual slot `+0x30` 判断 `is_2d`，然后直接输出：

```text
screen_x = world_x
screen_y = world_y
```

三维 CAST 分支取得 `SrRenderer+0x48`，按以下指令顺序计算：

```text
screen_h.x = y*m01 + x*m00
screen_h.x += z*m02
screen_h.x += m03

screen_h.y = x*m10
screen_h.y += y*m11
screen_h.y += z*m12
screen_h.y += m13

screen_h.w = x*m30
screen_h.w += y*m31
screen_h.w += z*m32
screen_h.w += m33

screen_x = screen_h.x * (1.0 / screen_h.w)
screen_y = screen_h.y * (1.0 / screen_h.w)
```

矩阵第 2 行在这个函数中不读取。因为 `SrRenderer+0x48` 已经包含 Width/Height 视口矩阵，所以除以 W 后得到的是最终像素屏幕坐标，而不是仍需额外 viewport 换算的 NDC。二维分支直接复制 XY 到同一输出数组也与这一结论一致。

## Rust 对应实现

[`src/projection.rs`](../../src/projection.rs) 当前实现：

- `identity_matrix4x4_game`
- `viewport_matrix_game`
- `mul_matrix4x4_game`
- `compose_screen_matrix_game`
- `project_point_to_screen_game`
- `project_cast_corners_to_screen`

单元测试覆盖二维直通、三维 X/Y/W 行选择、透视除法、NDC 四角到 D3D 屏幕坐标、4x4 乘法顺序及两次矩阵组合。

## 尚未闭环

- `0xAC7400` 中两个上游 camera/backend 矩阵虚函数的类名、字段名和 SRD 外部输入来源。
- 3D CAST 在 renderer 提交阶段的深度值和 D3D9 顶点格式。
- 裁剪、scissor 与实际 D3D9 viewport state 是否在其他路径对渲染提交再施加限制。

这些未决项不影响本页已经证明的 `SrRenderer+0x48` 构造顺序和 CAST X/Y 最终屏幕映射，但在实现完整 renderer 前仍需继续追踪。
