# SRD Simple shader 常量提交证据

分析对象为 `chusanApp.exe` SHA-256 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`；本轮 IDB SHA-256 `7BE4A919000CC62E435A8BCBD534916D14DC17991C9E0C6DBA4E7A97D8C767B8`。

## D3D9 常量调用

`ceylon_submit_default_fixed_shader_constants` (`0x671A00`) 和 `ceylon_submit_draw_packet_fixed_shader_constants` (`0x671B80`) 通过 D3D9 device 虚表 `+0x178/+0x1B4` 分别调用 `SetVertexShaderConstantF` 与 `SetPixelShaderConstantF`。

默认提交为：

```text
VS c0..c3 = identity
VS c4..c7 = identity
VS c8     = [0,0,0,0]
VS c9     = [0,0,0,0]
PS c0     = [0,0,1,0]
```

逐 draw packet 提交为：

```text
VS c0..c3 = packet+0xA0，packet+0x9E 为 0 时改用 identity
VS c4..c7 = packet+0xE0，未启用时改用同一 identity
VS c8     = [packet+0x1C, packet+0x20, packet+0x3C, packet+0x40]
VS c9     = packet+0x44..0x53
PS c0     = [packet+0x1C, packet+0x20, packet+0x24, 0]
```

packet 构造器 `0x6CD8A0` 初始化 `+0x1C/+0x20=0`、`+0x24=1`、`+0x3C/+0x40=0`、`+0x44..+0x50=0`。`srd_begin_quad_draw` (`0xAC5320`) 的二维分支调用矩阵 setter 时传 null，明确使 packet world matrices 走 identity 分支。

## 选定无贴图 fixture

`CHU_UI_System_00_v10.srd` 的三个首批候选节点都选择 Simple key：

```text
AAEBABBAAAGAAAAAAA
```

对应 VS 读取 position、两个 color、texcoord0，以及：

```text
c0..c3   mtxWorld
c8       fixedParam0
c10..c13 mtxPrjView
```

对应 PS 不读取 sampler，只读取两个 color 和 `c0.z`。因此这个 fixture 的首批绘制不依赖纹理加载；它需要的固定 packet 常量已由上述提交链闭环，`c10..c13` 则由 `sea::AllEnvBasic` 的 `Projection*View` provider 闭环，详见 [`projection.md`](projection.md)。

Rust `CeylonSrdFixedShaderConstants::initial_2d` 现保存这组初始寄存器值。对应的 332-byte VS 和 216-byte PS 也已按对齐的 `u32` token 表嵌入 `shader_bytecode.rs`；lookup 只接受精确 key `AAEBABBAAAGAAAAAAA`，其他 key 返回 `None`，不会用近似 shader 代替。
