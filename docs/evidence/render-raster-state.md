# SRD 光栅与颜色写入状态证据

本页记录 Ceylon draw packet 到 D3D9 cull、fill、color-write 与 scissor 状态的闭环。分析对象：

- `chusanApp.exe` SHA-256：`28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`
- 保存后的 IDB SHA-256：`189A48449F4761584712182DB6755698526D50BDDD0A00C113DF376E5C01096F`

关键函数为 `ceylon_reset_render_state_defaults` (`0xE5BE10`)、`ceylon_apply_draw_packet_state` (`0x6CEE30`)、`d3d9_flush_raster_and_scissor_state` (`0xE5D900`)、`d3d9_map_cull_mode` (`0xE5AFC0`) 和 `d3d9_map_fill_mode` (`0xE5B020`)。

## Cull mode

RenderState `+0x60` 保存内部 cull 值。默认构造值为 `1`。`d3d9_map_cull_mode` 直接读取 `0x1930664` 的四项表：

```text
internal 0 -> D3DCULL_CW   (2)
internal 1 -> D3DCULL_CCW (3)
internal 2 -> D3DCULL_NONE (1)
internal 3 -> D3DCULL_NONE (1)
```

`d3d9_flush_raster_and_scissor_state` 把映射结果提交到 `D3DRS_CULLMODE` (`22`)。在 draw packet 应用阶段，`draw_flags_00 bit 0x00800000` 未设置时，临时 RenderState 的 cull 值被强制写成内部 `2`；设置时才保留基础 RenderState。`CeylonDrawPacketPresetState::srd_renderer_initial()` 的 `0x0029E000` 未设置该位，因此选定 SRD quad 精确使用 `D3DCULL_NONE`，不是编辑器为了显示而自行关闭剔除。

## Fill mode 与颜色写入

RenderState `+0x7C` 的默认内部 fill 值为 `2`。映射函数行为为：

```text
internal 0 -> D3DFILL_POINT     (1)
internal 1 -> D3DFILL_WIREFRAME (2)
other      -> D3DFILL_SOLID     (3)
```

结果提交到 `D3DRS_FILLMODE` (`8`)。RenderState `+0x78` 则直接提交到 `D3DRS_COLORWRITEENABLE` (`168`)。draw packet 依次用 `draw_flags_00 bits 13..16` 形成四位颜色写掩码，并允许 `flags_60 bit 0x2000` 额外设置最高位：

```text
mask bit 0 <- draw_flags_00 bit 0x00002000
mask bit 1 <- draw_flags_00 bit 0x00004000
mask bit 2 <- draw_flags_00 bit 0x00008000
mask bit 3 <- draw_flags_00 bit 0x00010000 OR flags_60 bit 0x2000
```

初始 SRD packet 的四个 draw flag 位全为 1，所以首个 fixture 使用颜色写掩码 `0xF`。

## Scissor

同一 flush 函数把 RenderState `+0x64` 提交到 `D3DRS_SCISSORTESTENABLE` (`174`)，并把 `+0x68..+0x74` 的四个 LONG 传给 `SetScissorRect`。默认值为 disabled 与 `{5,0,0,0}`；material 的独立 enable/rectangle source 选择见 `render-scissor-state.md`。

Rust 的 `CeylonRasterState`、`d3d9_cull_mode_from_internal` 与 `d3d9_fill_mode_from_internal` 复现上述表和 packet 覆盖。选定 fixture 现在已闭环为 `CULL_NONE + SOLID + COLORWRITE 0xF`，可以在不引入显示性猜测的前提下继续接入 format 14 draw submission。
