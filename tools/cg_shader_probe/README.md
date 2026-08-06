# Cg shader probe

Evidence-only x86 utility. It does not start the game and is not part of the
editor runtime or release artifacts.

The probe reads the verified embedded shader records from `chusanApp.exe`,
recreates the Simple selector define prefix, invokes the game's 32-bit
`cg.dll` to obtain canonical D3D assembly, and assembles that text through
`D3DCompiler_47!D3DAssemble`. It never loads or calls D3DX.
Each assembled shader is then passed to a hidden-window D3D9 HAL device's
`CreateVertexShader` or `CreatePixelShader`; any rejected bytecode stops the
probe. `manifest.tsv` records the exact device- and shader-creation HRESULTs.

Build and run from an x86-capable Windows host:

```powershell
cargo build --manifest-path tools/cg_shader_probe/Cargo.toml --target i686-pc-windows-msvc
tools/cg_shader_probe/target/i686-pc-windows-msvc/debug/cg-shader-probe.exe `
  D:/Downloads/sessions/chusanApp.exe `
  D:/sdhd/bin/cg.dll `
  AAEBABBAADIIEAAAAA `
  D:/Downloads/sessions/WORK/shader_probe
```

The generated `.asm` and `.bin` files are investigation output and are not
committed to this repository.

Passing `D:/sdhd/assets/data/A000/shader/shadercollect.xml` in place of the
single key compiles the complete Simple collection and writes `manifest.tsv`.

After the complete probe has produced 82 successful VS/PS pairs, the editor's
checked-in Rust table is regenerated mechanically with:

```powershell
rustc --edition 2024 tools/package_simple_shaders.rs -O -o target/package_simple_shaders.exe
target/package_simple_shaders.exe `
  D:/Downloads/sessions/WORK/shader_probe_d3d9_all `
  src/shader_bytecode_generated.rs
```

The packager rejects missing/duplicate stages, non-`ok` manifest rows and
non-DWORD-aligned bytecode. It deduplicates identical stage blobs without
coalescing their selector keys.
