# 首个贴图 D3D9Ex draw 与宿主矩阵边界

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
Simple   = AAEBABBAABGAAAAAAA
```

该 key 的 VS 与首个无贴图 fixture 相同：332 bytes，SHA-256 `86669F24505A70D6DB560C6B2838EBA7D262B0206825BA3927658AB5A7112D61`。单贴图 PS 为 248 bytes，SHA-256 `066761E3FE149084A9526FDD1A091138B9DC894EAC29FA707D71992E4ED4E23F`；assembly 声明 `textureBase : texunit 0`，并由 `texld` 使用 TEXCOORD0。

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

## 像素回读结果与不能越过的边界

`--srd-texture-smoke` 只提交上述一个 draw。首帧和强制 `ResetEx` 后的第二帧均通过 `GetRenderTargetData` 读回 Composition，结果一致：

```text
changed pixels = 287
bbox           = (961, 0)..(961, 286)
diagnostic FNV = 408934AC672702F9
```

该结果证明真实 DDS、stage 0、sampler、精确 shader key/bytecode、draw submission、render target 和 ResetEx 重建路径共同产生了非清屏色像素。但它是一条竖线，不是输入矩形应有的二维覆盖，因此不能作为完整 SRD 画面正确性的证明；FNV 也只作为当前设备诊断值，不设为跨 GPU 的精确规范。

重新核对 `srd_renderer_configure_project_camera` (`0xAC7400`) 后确认：

1. `sea_camera_set_perspective_parameters` (`0x656450`) 的 Aspect 确实接收物理 render-target Width，而不是函数前面另行取得的外部 aspect；
2. 外部 camera 的 `Projection*View` 先由 `ceylon_inverse_matrix4x4` (`0x6B4170`) 求逆；
3. 随后与 SRD 全局 camera 的 `Projection*View` 相乘，结果进入 `SrRenderer+0x08`；
4. 顶层 CAST 的根还来自 SrPlayer 所在外部 scene graph 的 `FirstCalcMatrix`。

所以 identity `FirstCalcMatrix` 是编辑器 smoke 的显式宿主输入，不等价于任意游戏调用现场。下一阶段必须闭环一个实际 Chusan 调用点的外部 camera/scene-node 变换，或把预览 placement 明确作为用户可见的宿主参数；不能通过视觉试错擅自加入横向缩放。

后续已进一步证明 Advertise 的空 `TargetScene` packet 进入全局队列，并由所有注册 target 分别以自身 Camera 和 draw-mask/visibility 配置过滤；它不会隐式 fallback 到某个命名 Scene。详见 [`render-target-routing.md`](render-target-routing.md)。这使当前竖线的根因边界更明确：`CeylonSrdFixedShaderConstants::initial_2d` 直接使用 SRD CAM `Projection*View` 不符合游戏的 target-local shader 环境，但独立 SRD 也不能自行给出唯一替代 Camera。修正必须等待明确的宿主预览策略，而不是加入视觉补偿矩阵。
