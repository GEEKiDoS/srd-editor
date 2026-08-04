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

