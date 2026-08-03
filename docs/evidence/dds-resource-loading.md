# DDS 资源加载与 D3D9 创建证据

本页记录 SRD 外部 `.dds` 从资源工厂、文件读取、DDS 描述符解析到 D3D9/D3DX9 创建分支的完整闭环。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 本轮保存后的 IDB SHA-256：`B2694ACD47ABBF0EC35E07DCD3699C3878075041ECD975C2C408B09348C78A9E`
- D3D9 常量对照：本机 Windows SDK `10.0.26100.0/shared/d3d9types.h`

## SRD 路径到具体资源类型

`srd_resource_load_srff_and_external_textures` (`0xAA55C0`) 为每条 TEX 文件名追加 `.dds`，检查文件存在后通过 `surfride::SrResource` 虚表槽 `+0x4C` 提交路径。该调用经过 `0x42801A -> 0x706CE0`、虚表槽 `+0x48` 的 `0x42117F -> 0x706E50`，最终进入按扩展名查找工厂的 `0x6E70B0`。

`air_resource_manager_construct_and_register_factories` (`0x6E3E50`) 在 `0x6E40FB` 注册字面量 `dds`。注册函数 `air_register_dds_texture_resource_factory` (`0x6E3520`) 安装 RTTI 已命名的 `earth::FactoryConstructor<air::TextureResource,air::Resource>::vftable`。工厂构造槽进入 `air_texture_resource_factory_construct` (`0x6E6DD0`)，分配 `0x150` 字节并调用 `air_texture_resource_construct` (`0x704EF0`)；后者安装 `air::TextureResource::vftable`，并在对象 `+0x148` 构造内部 `ImageLoader`。

因此，SRD TEX 外部 `.dds` 路径的接收对象已经由扩展注册、工厂 RTTI 和构造虚表三者共同证明为 `air::TextureResource`，不是根据文件后缀推测。

## 文件字节与 ImageLoader

`air_resource_load_file_bytes` (`0x708B10`) 打开资源对象 `+8` 的路径，取得完整文件长度，分配缓冲区并读取全部字节，再调用资源虚表槽 `+0x40`。`air_resource_accept_loaded_bytes` (`0x708960`) 保存指针到 `+0xD8`、长度到 `+0xDC`，调用虚表槽 `+0x2C` 后把状态 `+0xF0` 置为 `1`。

该槽经 `air_texture_resource_decode_loaded_bytes` (`0x7087C0`) 调用虚表槽 `+0x1C`，落到 `air_texture_resource_submit_image_load` (`0x708020`)。它把原始字节、长度、路径和对象 `+0xE0` 传给内部 ImageLoader。`air_texture_image_loader_submit_dds` (`0x707E00`) 直接从原始缓冲区读取：

- `+12`：height；
- `+16`：width；
- `+28`：mipmap 数量；
- 整个文件缓冲区及其长度被保存到后端上传命令。

`chamomile_execute_texture_creation_command` (`0xE800E0`) 对命令类型 `1` 调用 `chamomile_process_dds_texture_upload` (`0xE7F5F0`)。这条链证明后端输入是完整 DDS 文件，而不是上游已解码像素。

## DDS 描述符解析

`chamomile_parse_dds_descriptor` (`0xE59FA0`) 只在首个 `u32` 等于 `0x20534444`（`DDS `）时接受输入；该函数没有验证 DDS header-size 字段。它读取的原始偏移为：

| DDS 偏移 | 用途 |
| --- | --- |
| `+12` | height |
| `+16` | width |
| `+28` | mip count；零转换为一 |
| `+80` | pixel-format flags |
| `+84` | FourCC |
| `+88` | bit count |
| `+100` | 16/32 位 RGB 分支使用的 mask |
| `+112` | caps2/cubemap flags |

格式分派和内部格式编号如下：

- flags `& 0x40`：16 位为内部 `0/4/5`，24 位为 `3`，32 位为 `1/2/16`；分支还使用 flags bit `0x1` 和 offset `+100` 的 mask。
- flags `& 0x4`：DXT1..DXT5 为内部 `48..52`，每像素位数为 `4/8/8/8/8`；数值 FourCC `111..116` 为内部 `64/65/66/80/81/82`。
- flags `& 0x20`：bit count 8 为内部 `33`，同时有 flags bit `0x1` 时为 `34`；bit count 16 为内部 `32`。没有类型信息证明 `0x20` 的源枚举名，因此 Rust 中保留数值命名。

内部格式 `32..34` 把 DDS `+128` 作为 1024 字节 palette，像素数据从 `+1152` 开始；其他格式的数据从 `+128` 开始。

caps2 bit `0x200` 表示 cube，六个 face bit `0x400..0x8000` 按低到高顺序映射为连续 face index；非 cube 的 face count 为一。每个 surface record 为 20 字节，顺序为 face-major、mip-minor，内容包括数据指针/文件偏移、height、width 和字节长度。每个 face 的 mip 0 重新使用原始尺寸，后续 mip 为 `max(previous >> 1, 1)`。

内部格式 `48..52` 计算字节数前把宽高分别向上补到四的倍数；其他格式使用原始宽高。最终公式均为 `bits_per_pixel * width * height / 8`。

## Direct3D 9 原生创建分支

`chamomile_process_dds_texture_upload` 只对内部格式 `48..52` 施加尺寸限制：宽和高都必须通过 `((value - 1) & value) == 0`，否则进入 D3DX 回退。设备对象 `+0x32` 或上传对象 `+69` 非零也会强制回退。其余情况调用 `d3d9_create_texture_direct_from_dds` (`0xE5B730`)。

二维纹理由 `d3d9_create_texture2d_from_dds_descriptor` (`0xE554B0`) 调用 `IDirect3DDevice9::CreateTexture`；cube 由 `d3d9_create_cube_texture_from_dds_descriptor` (`0xE5DFC0`) 调用 `CreateCubeTexture`。SRD 路径的固定参数为 usage `0`、pool `0`（Windows SDK 的 `D3DPOOL_DEFAULT`）和 null shared handle。

内部格式 `3/33/32` 在原生创建前转换为内部格式 `1`。DXT1..DXT5 的 mip 数会被截断：二维纹理的下一层宽或高小于 4 时停止；cube 的下一层 edge 小于 4 时停止。随后后端按解析出的 surface records 准备逐层上传；具体 lock/copy/unlock 调用仍需单独闭环，当前 Rust 不对未证明部分赋予实现语义。

`game_texture_format_to_d3d9` (`0xE5B070`) 的完整映射已经逐项转录到 Rust，包含 DXT1..5、浮点格式、深度格式以及 `INTZ`/`RAWZ`。`game_texture_usage_to_d3d9` (`0xE5B4D0`) 和 `game_texture_pool_to_d3d9` (`0xE5B400`) 也已确认 SRD 路径最终传入 `0/0`。

## D3DX9_43 回退分支

导入表证明游戏调用来自 `D3DX9_43` 的：

- `D3DXCreateTextureFromFileInMemoryEx`；
- `D3DXCreateCubeTextureFromFileInMemoryEx`。

`d3d9_create_texture_from_memory_d3dx` (`0xE5B9B0`) 保证 header mip count 至少为一，并按 caps2 bit `0x200` 选择 2D/cube。

二维调用 `d3d9_create_texture2d_from_memory_d3dx` (`0xE558F0`) 的参数为：header width、header height、header mip count、usage `0`、format `D3DFMT_UNKNOWN`、pool `D3DPOOL_DEFAULT`、filter/mipfilter `D3DX_DEFAULT (-1)`、color key `0`、source info null。仅当 raw pixel-format flags `& 0x1838 != 0` 时 palette 指向文件 `+128`，否则为 null。

cube 调用 `d3d9_create_cube_texture_from_memory_d3dx` (`0xE5E310`) 使用 edge length `D3DX_DEFAULT (-1)`、header mip count、usage `0`、unknown format、default pool、两个 default filter、null source info 和 null palette。

因此编辑器会直接调用同一代 D3D9/D3DX 接口，不自行实现 DXT 或 RGBA 像素解码；但不会把所有纹理统一交给 D3DX，因为这会偏离游戏已经证明的原生创建分支。

## 本地 DDS 语料回归

`WORK/diff-test/surfboard` 下 97 个 DDS 的结果为：

- 70 个内部格式 `1`（A8R8G8B8）；
- 27 个内部格式 `52`（DXT5）；
- 96 个只有一层 mip，1 个有十层；
- 没有 cube；
- 每个文件按游戏公式计算的最终 payload 末尾都与文件长度完全一致；
- 93 个进入原生 `CreateTexture`，4 个非二次幂 DXT5 进入 D3DX 回退。

四个回退样本为：

- `CHU_UI_Common_AvatarBox_00.dds`
- `CHU_UI_Common_select_box_01.dds`
- `CHU_UI_Duel_00_v13.dds`
- `CHU_UI_UnlockChallenge_shutter.dds`

Rust 的 `DdsDescriptor`、格式映射和 `D3d9TextureCreation` 计划均由单元测试及这 97 个文件的语料测试覆盖。

## 证据边界

已经闭环：资源对象类型、完整文件读取、DDS header 字段、格式/mip/cube/palette surface 布局、D3D9 与 D3DX9_43 分支条件及创建参数。

仍需闭环：原生纹理创建后的逐 surface lock/copy/unlock、失败和设备丢失时的资源生命周期、最终 shader 对两个 UV 通道和多纹理槽的消费。
