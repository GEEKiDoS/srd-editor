# Ruhuna font RFZ/YABX/AVTS archives

## Scope

This note records only behavior closed against the current `chusanApp.exe` and the six complete font archives under `A000/font`. The old editor and its documentation were not used as authorities.

## RFZ BinaryLZW container

- `sub_ED4050` reads the four-byte RFZ prefix and accepts `Y`, `S`, version `2`.
- `sub_ED4220` allocates a 4096-entry dictionary (`0x8000` bytes, 8 bytes per entry), initializes the first 256 literal entries, and starts with base width 8 / code width 9.
- `sub_ED3690` reads codes MSB-first, performs the LZW special-code expansion, grows the width when `inserted_code + 2 == 1 << width`, and implicitly resets when the next code reaches 4094.
- `sub_ED42F0` is the matching compressor initializer.

Rust `rfz::decompress` implements those exact rules. All six archives decompress with stable sizes and hashes, and every result begins with `YABX`.

## Generic YABX container

`sub_EC3360`, `sub_EC2FF0`, `sub_EC3C20`, and `sub_EC5120` establish:

- magic `YABX` for little endian and `XBAY` for big endian;
- header: `u16 version`, `u16 flags`, `u32 payload_size`, `u32 payload_crc`;
- non-reflected, MSB-first CRC-32 using polynomial `0x04C11DB7`, initial all ones, final complement;
- compact strings use signed `i8` byte length including NUL; long strings use signed `i16` length including NUL;
- schema order: legacy libraries, database name, dependencies, classes and fields, local IDs, object names, `i16 object_count`, then objects as `i16 class_index + u32 byte_size + bytes`.

Object IDs are not zero-based on disk. `sub_EC52D0` returns `local_index + 10001`; `sub_EC56E0` and `sub_EC5810` resolve a positive ID as `id - 10001`. Negative IDs are local-library references and zero is null. The font database references therefore resolve exactly as follows:

- first glyph ID `10002` -> YABX object index 1;
- last glyph ID -> the last glyph object;
- texture ID -> the final `TextureResource` object.

## Ruhuna database and glyph objects

The font schema contains `ruhuna::Database`, `ruhuna::Glyph`, and `ruhuna::TextureResource`.

The implemented `Database` order is the serialized schema order:

1. five dynamic strings: `id`, `platform`, `library`, `name`, `comment`;
2. `flags: u32`;
3. `point`, ascent/descent, glyph width/height: `u16`;
4. `tex_page: u32`;
5. texture width/height/last-height, margin, glyph count: `u16`;
6. dynamic glyph and texture reference arrays, each encoded as `u32 count` followed by `i16` object IDs.

The `Glyph` object contains eleven two-byte scalar fields followed by dynamic `kerning_info`. Field registration functions around `sub_F58E50..sub_F59080` identify the `u16` and `s16` scalar types. `sub_F53FD0` constructs the custom `kerning_infoField`; its virtual methods expose count at object `+36` and the pointer array at `+40`.

Every glyph in all six shipped archives has exactly 30 serialized bytes. The final eight bytes are a dynamic-field byte size of 4 followed by a zero `u32` element count, and `kerning_info_cnt` is also zero. The Rust parser accepts and verifies this proven case. A non-empty kerning array returns an explicit unsupported error; no element layout is guessed.

`TextureResource::file` is a custom field. `sub_F599F0` constructs it and `sub_F5C110` replaces its `{size, pointer}` storage from the bounded serialized field. The YABX object therefore contains one dynamic field whose payload begins directly with `AVTS`.

## AVTS directory and embedded DDS pages

`sub_128BA30` performs the `AVTS` prefix check. The exact outer directory layout is visible in `sub_EEE8D0`:

- dword 0: magic;
- dword 1: entry count;
- entry array begins at byte `0x80`;
- entry stride is `0x400` bytes;
- each entry begins with a fixed 512-byte name buffer;
- dwords at entry `+0x200` and `+0x204` are preserved metadata;
- entry `+0x208` is data size;
- entry `+0x20C` is data offset;
- the loader copies exactly `data_size` bytes from `file_base + data_offset`.

The first entry has metadata `0/0` and contains a Stevia YABX `.svo`. Remaining entries have metadata `1/page_number` and contain complete DDS files. `sub_1297CF0` independently confirms that `SpTextureList` dispatches entry data beginning with `DDS ` to the DDS path (and has a separate `BM8 ` branch).

All directory ranges are bounds-checked and non-overlapping in Rust. Every embedded DDS is parsed and length-validated by the existing DDS implementation, so font texture decoding/upload reuses the established library-backed path rather than reimplementing texture codecs.

| Archive | Ruhuna point | Glyphs | DDS pages | Page dimensions |
|---|---:|---:|---:|---|
| 14pt | 10 | 7161 | 1 | 2048x1024 |
| 18pt | 14 | 7161 | 1 | 2048x2048 |
| 24pt | 18 | 7161 | 2 | 2048x2048, 2048x512 |
| 32pt | 24 | 7161 | 2 | 2048x2048, 2048x2048 |
| 60pt | 45 | 7161 | 7 | six 2048x2048 pages, one 2048x256 page |
| 240pt | 180 | 201 | 2 | 2048x2048, 2048x1024 |

Every page is one-mip `A4R4G4B4`. The DDS page count equals `Database.tex_page`, page width equals `tex_w`, full-page height equals `tex_h`, and the final page height equals `tex_last_h` in all six archives.

## Database lookup and 128-byte runtime glyph conversion

`sub_F47700` is the post-load Database lookup builder called by both RFZ loaders. It takes the first and last glyph codes from the serialized glyph-reference order, allocates one `u16` entry for every code in that inclusive range, zero-fills the table, and writes each glyph index at `code - minimum_code`. Consequently an in-range hole remains index zero; Rust preserves that value rather than inventing a missing-glyph sentinel.

`sub_7CB9B0` is called after the AVTS texture archive has loaded. It copies the dense table and converts every referenced Ruhuna glyph into a fixed 128-byte record. The record boundaries are independently confirmed by allocation `glyph_count << 7`, construction stride 128, and the zeroing constructor `sub_7CB6F0`.

Important proven fields are:

| Offset | Runtime value |
| ---: | --- |
| `+0x06` | glyph code |
| `+0x0C/+0x0E` | Database point twice |
| `+0x10/+0x14` | `box_x1/box_y1` promoted to u32 |
| `+0x18` | page texture handle, or zero for page `0xFFFF`/unavailable page |
| `+0x1C/+0x20` | reciprocal texture width/page height; the last page uses `tex_last_h` |
| `+0x28/+0x2C` | glyph bearing/origin-derived offsets |
| `+0x30/+0x34` | inclusive glyph width/height |
| `+0x38` | `max_ascent + max_descent` |
| `+0x3C/+0x40` | cell advances; swapped in the rotated branch, with rotated Y advance `cell_inc_x + 1` |
| `+0x44/+0x48` | `trunc(point * 1.3333334 + 0.5)` |
| `+0x4C` | `-1` |
| `+0x50` | `(flags & 1) * 2` |
| `+0x58` | signed `-4096` |
| `+0x5C..+0x7B` | four UV pairs |
| `+0x7C` | rotated marker (`0` or `1`) |

For the normal branch (`flags & 2 == 0`), width/height are the inclusive X/Y box extents, bearing is `(origin_x, max_ascent - origin_y)`, and UVs use the exact one-pixel outer border: `(x1-1,y1-1)`, `(x1+width+1,y1-1)`, `(x1-1,y1+height+1)`, `(x1+width+1,y1+height+1)`.

For the rotated branch, width and height are swapped, bearing becomes `(max_descent + origin_y, origin_x)`, advances become `(cell_inc_y, cell_inc_x + 1)`, and UV order is `(x1-1,y2+1)`, `(x1-1,y1-1)`, `(x2+1,y2+1)`, `(x2+1,y1-1)`.

The final assembly at `0x7CBE48..0x7CBE61` proves a subtle common adjustment: bit zero is doubled into `+0x50`, doubled again, then added to bearing Y. Thus `flags & 1` adds four, not two. When the page handle is zero, geometry and UVs are first cleared, after which this common four-pixel adjustment still applies.

Rust now exposes this conversion as a host-independent runtime font. Its exact game-layout record uses opaque `u32` tokens for the x86 pointer slots, so the editor does not truncate or reinterpret D3D objects on an x64 host. Unit tests lock all offsets and both orientation branches; the six real archives additionally validate dense lookup entries, page selection, and last-page reciprocal height.
