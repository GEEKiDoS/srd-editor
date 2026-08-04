use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use srd_editor::{dds::DdsDescriptor, rfz, ruhuna::RuhunaFont, yabx::YabxFile};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<PathBuf> = env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        return Err("usage: rfz-inspect <file.rfz> [...]".into());
    }
    for path in paths {
        let compressed = fs::read(&path)?;
        let decompressed = rfz::decompress(&compressed)?;
        let preview_len = decompressed.len().min(256);
        println!(
            "{}: compressed={} decompressed={} fnv1a64={:016X} first={:02X?}",
            path.display(),
            compressed.len(),
            decompressed.len(),
            fnv1a64(&decompressed),
            &decompressed[..preview_len]
        );
        println!(
            "  ascii={:?}",
            String::from_utf8_lossy(&decompressed[..preview_len])
        );
        let yabx = YabxFile::parse(decompressed)?;
        println!(
            "  yabx: version={} flags={:#06X} database={:?} libraries={} classes={} local_ids={} object_names={} objects={} trailing={}",
            yabx.version,
            yabx.flags,
            String::from_utf8_lossy(&yabx.database_name),
            yabx.libraries.len(),
            yabx.classes.len(),
            yabx.local_ids.len(),
            yabx.object_names.len(),
            yabx.objects.len(),
            yabx.trailing.len()
        );
        for (index, class) in yabx.classes.iter().enumerate() {
            println!(
                "    class {} {:?} flags={:#04X} parent={} fields={}",
                index + 1,
                String::from_utf8_lossy(&class.name),
                class.flags,
                class.parent_index,
                class.fields.len()
            );
            for field in &class.fields {
                println!(
                    "      {:?}: flags={:#04X} storage_size={}",
                    String::from_utf8_lossy(&field.name),
                    field.flags,
                    field.storage_size
                );
            }
        }
        let mut class_counts = BTreeMap::<i16, usize>::new();
        for object in &yabx.objects {
            *class_counts.entry(object.class_index).or_default() += 1;
        }
        for (class_index, count) in class_counts {
            let class_name = yabx
                .classes
                .get(usize::try_from(class_index).unwrap_or(0).saturating_sub(1))
                .map(|class| String::from_utf8_lossy(&class.name).into_owned())
                .unwrap_or_else(|| "<unknown>".into());
            println!("    objects: class={class_index} {class_name:?} count={count}");
            for (index, object) in yabx
                .objects
                .iter()
                .enumerate()
                .filter(|(_, object)| object.class_index == class_index)
                .take(3)
            {
                let data = yabx.object_data(object);
                println!(
                    "      object {index}: bytes={} first={:02X?}",
                    data.len(),
                    &data[..data.len().min(64)]
                );
            }
            if class_name == "ruhuna::Glyph" {
                let mut size_counts = BTreeMap::<usize, usize>::new();
                for object in yabx
                    .objects
                    .iter()
                    .filter(|object| object.class_index == class_index)
                {
                    *size_counts.entry(object.data.len()).or_default() += 1;
                }
                println!("      byte-size histogram: {size_counts:?}");
                for (index, object) in yabx
                    .objects
                    .iter()
                    .enumerate()
                    .filter(|(_, object)| {
                        object.class_index == class_index && object.data.len() != 30
                    })
                    .take(12)
                {
                    let data = yabx.object_data(object);
                    println!(
                        "      extended glyph object {index}: bytes={} data={:02X?}",
                        data.len(),
                        data
                    );
                }
            }
            if class_name == "ruhuna::Database"
                && let Some((index, object)) = yabx
                    .objects
                    .iter()
                    .enumerate()
                    .find(|(_, object)| object.class_index == class_index)
            {
                let data = yabx.object_data(object);
                println!(
                    "      database object {index}: first={:02X?}",
                    &data[..data.len().min(256)]
                );
                println!(
                    "      database object {index}: last={:02X?}",
                    &data[data.len().saturating_sub(128)..]
                );
            }
        }
        println!("    trailing={:02X?}", yabx.trailing_bytes());
        let font = RuhunaFont::from_yabx(yabx)?;
        println!(
            "  ruhuna: point={} glyphs={}/{} textures={} atlas={}x{} pages={} file_bytes={:?}",
            font.database.point,
            font.glyphs.len(),
            font.database.glyph_count,
            font.textures.len(),
            font.database.texture_width,
            font.database.texture_height,
            font.database.texture_page_count,
            font.textures
                .iter()
                .map(|texture| font.texture_file_bytes(texture).len())
                .collect::<Vec<_>>()
        );
        for texture in &font.textures {
            let avts = font.texture_avts(texture)?;
            let mut dds_count = 0usize;
            println!("    avts entries={}", avts.entries.len());
            for (entry_index, entry) in avts.entries.iter().enumerate() {
                let data = avts.entry_data(entry);
                print!(
                    "      entry {entry_index}: {:?} fields={:#X}/{:#X} range={:#X}..{:#X}",
                    String::from_utf8_lossy(&entry.name),
                    entry.field_200,
                    entry.field_204,
                    entry.data.start,
                    entry.data.end
                );
                if data.starts_with(b"DDS ") {
                    let descriptor = DdsDescriptor::parse(data)?;
                    descriptor.validate_data_len(data)?;
                    dds_count += 1;
                    println!(
                        " DDS={}x{} mips={} format={:#X}",
                        descriptor.width,
                        descriptor.height,
                        descriptor.mip_count,
                        descriptor.format.0
                    );
                } else if data.starts_with(b"YABX") {
                    let metadata = YabxFile::parse(data.to_vec())?;
                    println!(
                        " YABX={:?} objects={}",
                        String::from_utf8_lossy(&metadata.database_name),
                        metadata.objects.len()
                    );
                } else {
                    println!(" magic={:02X?}", &data[..data.len().min(4)]);
                }
            }
            if dds_count != font.database.texture_page_count as usize {
                return Err(format!(
                    "AVTS DDS entry count {dds_count} does not match Database tex_page {}",
                    font.database.texture_page_count
                )
                .into());
            }
        }
    }
    Ok(())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
