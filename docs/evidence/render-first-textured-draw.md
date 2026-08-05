# 首个贴图 D3D9Ex draw 与二维 ShapeEnv 修复

本页记录首个已实际提交到 D3D9Ex 的单贴图 Simple draw，以及像素回读暴露出的宿主矩阵边界。分析对象为：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`D1E8497481C8E365A36982B6EAB98C9EAEE52274120911078A64B3C14D6BBDB2`

## 外部 DDS 名称与编辑器路径映射

`srd_resource_load_srff_and_external_textures` (`0xAA55C0`) 对资源对象保存的路径调用 `path_get_parent_directory` (`0x5FE360`)，随后用 `path_append_c_string` (`0x5F90F0`) 追加 TEX `+0x204` 的基础名和固定 `.dds` 后缀。独立的 Chusan 路径 provider `chusan_get_surfboard_root_path` (`0x7D6360`) 建立 `surfboard/`，`chusan_get_surfboard_texture_root_path` (`0x7D61A0`) 再追加 `texture/`。

编辑器接收的是本地完整 game data 根，因此 `TextureDefinition::external_dds_path` 明确执行编辑器侧映射：

```text
<game-data-root>/surfboard/texture/<TEX base name>.dds
```

这条本地映射由 Chusan 的专用 texture root 和实际文件语料共同验证；它不声称游戏的不同资源对象在所有调用点都以同一个字符串拼接步骤完成绝对路径。

## 单贴图 Simple key 与 bytecode

`CHU_UI_Advertise_00_v10.srd` 的多个 ImageCast 使用：

```text
ShapeEnv = 00000000:00373F90
Simple   = EAEBABBAABGAAAAAAA
```

layer 4 flags 为 `0x100`，所以 CAST 是二维。该 exact key 的 VS 为 384 bytes，SHA-256 `A3E0CA2EFA3452A529DDE7E92EB638FAAD4A230DF5A5537D2D50BE53BC045BD4`；单贴图 PS 为 248 bytes，SHA-256 `066761E3FE149084A9526FDD1A091138B9DC894EAC29FA707D71992E4ED4E23F`。VS 读取 `c10 screenParam`；PS 声明 `textureBase : texunit 0`，并由 `texld` 使用 TEXCOORD0。

编辑器只在 compact key 精确相等时选择这对 bytecode。CREF 绑定 packet/stage 0；CRE1 对应 stage 1；stage 2 保持空。每个已绑定 stage 只提交二进制证明的 `ADDRESSU`、`ADDRESSV`、`MINFILTER` 和 `MAGFILTER`，没有补写未证明的 `MIPFILTER`。

## 非退化验证 fixture

最先调查的 layer 1/node 6 `C_circle` 在初始状态四个顶点全部为 `(4.039699, 26.258043, 0)`，是零面积 triangle strip，因此不能作为像素验证 fixture。DDS `CHU_UI_Advertise_00_v250.dds` 自身为 2048×1024 DXT5，独立解码后的 alpha 范围包含 0 与 255；失败不是空纹理造成。

当前 smoke 精确选择 scene 0/layer 4/node 4 `C_movie_dummy`：

- texture index `5`；
- `CHU_UI_Movie_dummy.dds`；
- 声明和解码尺寸均为 `720×408`；
- 初始四顶点形成非退化矩形，primary color alpha 为 255；
- stage 0 使用该 TEX 记录解析出的精确 sampler。

`SrdD3d9TextureSet` 保留所需 DDS 源字节，创建 DEFAULT-pool `IDirect3DTexture9`。`ResetEx` 前释放 GPU texture，reset 后从保留的 DDS 重新创建和上传；renderer 同时重建 vertex declaration、dynamic vertex buffer、shader 对象和 Composition target。

## 像素回读结果与宿主输入边界

`--srd-texture-smoke` 只提交上述一个 draw。首帧和强制 `ResetEx` 后的第二帧均通过 `GetRenderTargetData` 读回 Composition，结果一致：

```text
changed pixels = 849776
bbox           = (346, 194)..(1573, 885)
diagnostic FNV = 09DF61BBE19B88A5
```

该结果证明真实 DDS、stage 0、sampler、二维 exact shader key/bytecode、`screenParam=[960,540,0,0]`、draw submission、render target 和 ResetEx 重建路径共同产生了与输入矩形对应的二维覆盖。FNV 只作为当前设备诊断值，不设为跨 GPU 的精确规范。

重新核对 `srd_renderer_configure_project_camera` (`0xAC7400`) 后确认，下列 Camera bridge 只在 property 2 成功解析到非空命名 target 时执行：

1. `sea_camera_set_perspective_parameters` (`0x656450`) 的 Aspect 确实接收物理 render-target Width，而不是函数前面另行取得的外部 aspect；
2. 外部 camera 的 `Projection*View` 先由 `ceylon_inverse_matrix4x4` (`0x6B4170`) 求逆；
3. 随后与 SRD 全局 camera 的 `Projection*View` 相乘，结果进入 `SrRenderer+0x08`；
4. 顶层 CAST 的根还来自 SrPlayer 所在外部 scene graph 的 `FirstCalcMatrix`。

`AdvertiseLogoObject` 的实际父节点已进一步证明为 null，所以该具体宿主的 `FirstCalcMatrix` 是 identity；但 target Camera present size 和 ShapeEnv2D filter source size 仍是宿主输入。smoke 分别显式传入 `1080x1920` 与 `1920x1080`，不会把二者静默等同。

Advertise 的空 `TargetScene` packet 进入全局队列，并由所有注册 target 分别以自身 Camera 和 draw-mask/visibility 配置过滤；它不会隐式 fallback 到某个命名 Scene。空 target 同时使 renderer Camera bridge 保持 identity，详见 [`render-target-routing.md`](render-target-routing.md) 与 [`render-empty-target-matrices.md`](render-empty-target-matrices.md)。此前竖线的直接根因仍是 builder 漏写 CAST 二维标志，错误选择三维 `A...` VS 并向 `c10..c13` 上传 Camera matrix；修复链与 screenParam provider 见 [`render-shape-env-2d.md`](render-shape-env-2d.md)。
