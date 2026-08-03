# TEX 采样包装对象与 D3D9 提交证据

本页记录 TEX `0x62` 到实际 D3D9 纹理/采样器状态的完整闭环。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`D863D930AF8CDF263D8CB172E7824D7D1F48C212444F3098B625B49951435E09`
- D3D9 常量对照：本机 Windows SDK `10.0.26100.0/shared/d3d9types.h`

## 包装对象的类型与成对建立

`ceylon_texture_construct` (`0xE7E1E0`) 构造 120 字节对象并安装 `off_1932850`。该虚表的类型名槽最终返回字面量 `ceylon::resource::Texture`，因此对象类型不是依据邻近 RTTI 推测，而是由对象自身虚表证明。

SRD 资源建立路径在 `0xAAB339` 从 TEX 记录 `+0x20C` 取属性 `0x62`，并按 TEXL 顺序调用 `srd_set_render_texture_pair` (`0xAC4F90`)。后者为同一已加载纹理保存两个 `ceylon::resource::Texture` 包装对象，并设置：

| TEX `0x62` / 包装序号 | 包装字段 | 内部值 |
| --- | --- | --- |
| `(field_62 & 0x00F0) != 0` | 两个对象 `+0x18` | `2`；否则 `0` |
| `(field_62 & 0x0F00) != 0` | 两个对象 `+0x1C` | `2`；否则 `0` |
| 第一个对象 | `+0x20`, `+0x24` | `1`, `1` |
| 第二个对象 | `+0x20`, `+0x24` | `0`, `0` |

四个 setter 已逐一落到直接字段写入：`0xE7E870` 写 `+0x18`，`0xE7E880` 写 `+0x1C`，`0xE7E770` 写 `+0x20`，`0xE7E760` 写 `+0x24`。原 IDA 中若干 MFC/STL 名称是错误的库函数碰撞，现已按实际写入和下游 D3D9 使用更正。

## 包装字段同步到后端采样状态

绘制前，`ceylon_texture_sync_sampler_state` (`0xE7E8B0`) 取得：

```text
backing = wrapper[+0x14]
sampler_state = backing[+0x4C]
```

与本页有关的直接复制为：

| `ceylon::resource::Texture` | 后端状态 | 后续 D3D9 状态 |
| --- | --- | --- |
| `+0x18` | `+0x0C` | `D3DSAMP_ADDRESSU` |
| `+0x1C` | `+0x10` | `D3DSAMP_ADDRESSV` |
| `+0x20` | `+0x14` | `D3DSAMP_MINFILTER` |
| `+0x24` | `+0x18` | `D3DSAMP_MAGFILTER` |

`+0x24 == 2` 有一个写入 `+0x1C` 的特殊分支，但 SRD 配置路径只写 `0` 或 `1`，因此 SRD 的 MagFilter 总是走 `+0x18` 直接复制分支。

## 绘制包到 D3D9

304 字节绘制包在 `+0x30/+0x34/+0x38` 保存最多三个纹理包装对象。`sub_6CEE30` 遍历这三个槽，对每个非空对象调用上述同步函数，再把 `backing +0x4C` 的状态指针写入当前纹理槽并标记 dirty。随后 `d3d9_flush_texture_sampler_state` (`0xE59410`) 逐槽提交。

该函数对设备指针 `dword_1CC5158 +0x78` 的虚表调用为：

- 虚表 `+0x104`：`IDirect3DDevice9::SetTexture(stage, texture)`；
- 虚表 `+0x114`：`IDirect3DDevice9::SetSamplerState(stage, type, value)`。

四个字段的实际 `type` 参数直接出现在调用点：

| 后端状态 | `type` | Windows SDK 名称 |
| --- | --- | --- |
| `+0x0C` | `1` | `D3DSAMP_ADDRESSU` |
| `+0x10` | `2` | `D3DSAMP_ADDRESSV` |
| `+0x14` | `6` | `D3DSAMP_MINFILTER` |
| `+0x18` | `5` | `D3DSAMP_MAGFILTER` |

游戏不会把内部值直接传给 D3D9。`ceylon_address_mode_to_d3d9` (`0xE5B540`) 使用地址 `0x19306E8` 的表 `[1, 2, 3, 4]`；Windows SDK 将其定义为 Wrap、Mirror、Clamp、Border。因此 SRD 写入的内部值 `0/2` 分别成为 Wrap/Clamp。

`ceylon_filter_mode_to_d3d9` (`0xE5B050`) 使用地址 `0x19306F8` 起始的表 `[1, 2, 3, 0]`；Windows SDK 将 `1/2/3/0` 定义为 Point、Linear、Anisotropic、None。因此 SRD 两个包装对象中的内部值 `1/0` 分别成为 Linear/Point。

由此 TEX `0x62` 的已证明语义是：

```text
address_u = any(field_62 & 0x00F0) ? Clamp : Wrap
address_v = any(field_62 & 0x0F00) ? Clamp : Wrap
pair[0]   = Linear minification + Linear magnification
pair[1]   = Point minification  + Point magnification
```

## SrSliceCast 的 pair 选择

`srd_cast_get_texture_override` (`0xAD3D80`) 以 `selector & 1` 返回对象 `+0x1B4/+0x1B8` 的两个显式纹理覆盖槽。`srd_render_slice_cast` (`0xADA650`) 在 `0xADAB63` 读取对象字节 `+0x100`，并通过保留的 outgoing stack 把它作为 `srd_select_render_texture_pair` (`0xAC6E90`) 的最后一个参数。

`srd_select_render_texture_pair` 先保留显式覆盖指针；覆盖为零且 signed CREF image index 在 pair 数组范围内时：

```text
selector == 0  -> pair[0] -> Linear
selector != 0  -> pair[1] -> Point
```

因此 `SrSliceCast +0x100` 的渲染语义可以精确命名为 Point 采样选择标志，而无需从画面或旧编辑器行为猜测。

## CREF/CRE1 到绘制包槽位

`srd_construct_renderer` (`0xAC4010`) 在 renderer `+0xF0` 构造 vertex builder；builder 构造函数 `0x6DE3F0` 又在自身 `+0x20` 构造 304 字节绘制包。因此 renderer 内嵌绘制包的起点是 `+0x110`，而 `srd_select_render_texture_pair` 写入的 renderer `+0x140/+0x144` 精确对应 packet `+0x30/+0x34`：

| SRD 通道 | renderer 偏移 | packet 偏移 | D3D9 stage |
| --- | --- | --- | --- |
| CREF / TEXCOORD0 | `+0x140` | `+0x30` | `0` |
| CRE1 / TEXCOORD1 | `+0x144` | `+0x34` | `1` |

这两个指针没有在提交过程中重排。`ceylon_submit_vertex_batch` (`0x6DF020`) 把 builder `+0x20` 传入 `ceylon_enqueue_draw_packet` (`0x670BE0`)；后者从 packet `+0x30` 起遍历恰好三个纹理槽并保留非空资源，随后 `ceylon_copy_draw_packet_prefix` (`0x66F200`) 原位复制 `+0x30/+0x34/+0x38`。

packet 构造函数 `0x6CD8A0` 把三个槽全部初始化为空，而 SRD 路径只写前两个槽。因此当前已证明的 SRD 映射是 CREF -> slot/stage 0、CRE1 -> slot/stage 1、slot/stage 2 保持空。显式覆盖只改变对应槽的资源来源，不改变槽号。

## Rust 对应

`TextureDefinition::sampler_pair` 保留原始 `field_62`，同时生成已证明的 Wrap/Clamp 与 Linear/Point 状态。`ResolvedSliceTexture` 携带整对状态；调用方以 `TextureSamplerPair::select(point_sampled)` 复现 `SrSliceCast +0x100` 的选择。`ImageDefinition::resolve_texture_slots` 进一步把两个独立坐标结果解析为 packet slot 0/1，并保留“显式覆盖优先、否则按 TEXL 下标选择”的来源。枚举的 `repr(u32)` 数值就是 Windows SDK 的 D3D9 常量，可供后续 D3D9 后端直接提交。

## 仍未闭环

- DDS 资源对象、格式选择、mipmap、D3D9/D3DX9_43 创建参数和二维 SYSTEMMEM staging/`UpdateSurface` 已闭环，见 [`dds-resource-loading.md`](dds-resource-loading.md)；剩余的是内部格式转换、cube request 和设备丢失生命周期。
- CIMG/CRE1 已闭环到 TEXL 条目、Linear/Point pair 选择以及 packet/D3D9 stage 0/1；双 UV 的像素公式仍未闭环。
- 绘制包的 36 字节顶点格式、非索引 triangle strip 和 `DrawPrimitive` 参数已经闭环，见 [`render-vertex-submission.md`](render-vertex-submission.md)；混合、alpha/depth/stencil 和 scissor 状态也已分别闭环，剩余 shader 与其他 draw state。
