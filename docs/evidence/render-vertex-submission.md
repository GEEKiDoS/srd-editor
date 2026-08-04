# SRD 四顶点格式与 D3D9 primitive 提交证据

分析对象：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；
- 保存后的 IDB SHA-256：`1B780E16652216CA5021F0A31EDCD7EDBBD46B16D39EF0C06810875D4BE906E9`。

## 36 字节顶点格式

`srd_render_image_cast` (`0xAD77D0`) 在 `0xAD7C75..0xAD7D16` 连续写四个顶点，每次把目标指针增加 `0x24`。`srd_render_slice_cast` 和 `srd_render_number_glyph` 使用相同步长和字段偏移。格式如下：

| 顶点偏移 | 大小 | 已证明来源 |
| --- | --- | --- |
| `+0x00` | 12 | 三个 f32 位置分量 |
| `+0x0C` | 4 | primary packed color |
| `+0x10` | 4 | secondary packed color |
| `+0x14` | 8 | UV 通道 0，两个 f32 |
| `+0x1C` | 8 | UV 通道 1，两个 f32 |

Image、Slice 和 Number 都按四顶点顺序写入。Image/Number 的 primary 为描述符顶点色乘 CAST multiplicative tint；secondary 先把 CAST additive tint 转为 `[R*A/255, G*A/255, B*A/255, 0]`。Slice 的 secondary 是 CAST additive tint 与 SLIC `0x33` 的逐通道饱和加法，不走 Image/Number 的 alpha 预乘路径。

Rust 的 `#[repr(C)] SrdRenderVertex` 固定上述字段顺序，并以 `size_of`/`offset_of` 测试验证 36 字节布局。`ImageDefinition::build_render_quad` 已把动画后的 size/origin、第一份描述符顶点色、两份最终 UV 和两种世界 tint 组合成同布局四顶点；`build_slice_render_quad` 使用 Slice 已证明的颜色链，并把同一最终 UV 复制到两个通道；`NumberDefinition::build_glyph_render_quad` 则使用已排版 glyph 的四个位置和 Number 的双描述符状态建立同格式顶点。

## 格式 14 的 D3D9 vertex declaration

全局顶点格式注册器在 `sub_671D30` 的 case `14` 精确追加五个元素。每个元素保存内部 semantic、type 和 usage index；`sub_1319690` 再累计同一 stream 的 byte offset，并通过两个映射表写成 8 字节 `D3DVERTEXELEMENT9`。type 表为恒等映射 `0..16`；semantic 表在 `0..8` 后把内部 `9..12` 映射为 D3D9 usage `10..13`。

格式 `14` 最终声明为：

| Stream | Offset | Type | Usage | Index |
| --- | --- | --- | --- | --- |
| 0 | 0 | `D3DDECLTYPE_FLOAT3` (`2`) | `POSITION` (`0`) | 0 |
| 0 | 12 | `D3DDECLTYPE_D3DCOLOR` (`4`) | `COLOR` (`10`) | 0 |
| 0 | 16 | `D3DDECLTYPE_D3DCOLOR` (`4`) | `COLOR` (`10`) | 1 |
| 0 | 20 | `D3DDECLTYPE_FLOAT2` (`1`) | `TEXCOORD` (`5`) | 0 |
| 0 | 28 | `D3DDECLTYPE_FLOAT2` (`1`) | `TEXCOORD` (`5`) | 1 |
| `0xFF` | 0 | `D3DDECLTYPE_UNUSED` (`17`) | 0 | 0 |

`sub_1319690` 以设备虚表 `+0x158` 调用 `IDirect3DDevice9::CreateVertexDeclaration`，之后提交路径以 `+0x15C` 调用 `SetVertexDeclaration`。Rust 的 `SRD_D3D9_VERTEX_DECLARATION` 直接保存上述六条记录，可交给后续 D3D9 后端创建声明对象。

## 游戏内部格式与 primitive 参数

`srd_render_image_cast`、每个 active Slice 单元和每个 Number glyph 都经 `sub_AC5320` 建立绘制包。该函数在 `0xAC54A1` 对顶点批次对象的虚表 `+0x08` 传入：

```text
vertex_format_id = 14
primitive_type   = 4
vertex_count     = 4
```

四顶点的几何顺序是左上、左下、右上、右下，正好形成 triangle strip。后端 `d3d9_map_internal_primitive_type` (`0xE5B490`) 使用表 `[1,2,3,4,5,6]`，所以内部类型 `4` 映射为 D3D9 primitive 值 `5`，即 `D3DPT_TRIANGLESTRIP`。`d3d9_primitive_count_from_vertex_count` (`0xE5B420`) 对内部类型 `4` 返回 `vertex_count - 2`，四顶点因此提交两个 primitive。

## 绘制包到 IDirect3DDevice9

`ceylon_apply_draw_packet_state` (`0x6CEE30`) 同步渲染状态、纹理和采样器，再进入 `ceylon_submit_draw_packet` (`0x6CF110`)。后者完成以下设备调用：

- 设备虚表 `+0x15C`：`IDirect3DDevice9::SetVertexDeclaration`；
- 设备虚表 `+0x190`：`IDirect3DDevice9::SetStreamSource`；
- 设备虚表 `+0x198`：`IDirect3DDevice9::SetStreamSourceFreq`；
- 设备虚表 `+0x1A0`：`IDirect3DDevice9::SetIndices`；
- `d3d9_draw_nonindexed_primitive` (`0xE58930`) 的设备虚表 `+0x144`：`IDirect3DDevice9::DrawPrimitive`。

SRD 四顶点类型走非索引路径，最终参数等价于：

```text
DrawPrimitive(D3DPT_TRIANGLESTRIP, start_vertex, 2)
```

引擎也预建了 `(0,1,3, 1,2,3)` 的 quad index pattern，但那属于内部 primitive 类型 `6` 的独立 indexed 分支，不能替代 SRD 当前实际提交的类型 `4`。

## 仍未闭环

- draw packet 的 shader、blend、depth、stencil、scissor、cull、fill 与 color-write 已分别闭环；剩余的是把这些已建模状态接到编辑器的真实 draw submission，并继续追踪尚未命名的其他 packet 状态。
- Image/Text 双 UV 在 shader 或固定管线中的组合公式。
- DDS 描述符、D3D9/D3DX9_43 创建参数和二维 SYSTEMMEM staging/`UpdateSurface` 已闭环，见 [`dds-resource-loading.md`](dds-resource-loading.md)；内部格式转换、cube request 和设备丢失/重建仍待闭环。
