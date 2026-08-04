use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use srd_editor::vtbf::{Block, SrdFile};

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: srd-vtbf-dump <file.srd>")?;
    let file = SrdFile::parse(fs::read(&path)?)?;
    for block in &file.blocks {
        dump_block(&file, block, 0);
    }
    Ok(())
}

fn dump_block(file: &SrdFile, block: &Block, depth: usize) {
    let indent = "  ".repeat(depth);
    println!(
        "{indent}{} offset={:#x} properties={} children={}",
        String::from_utf8_lossy(&block.tag),
        block.offset,
        block.properties.len(),
        block.children.len(),
    );
    for property in &block.properties {
        let value = property.value_bytes(file);
        let hex = value
            .iter()
            .take(64)
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "{indent}  code={:#04x} flags={:#04x} type={} count={} multiplier={} value=[{}{}]",
            property.code,
            property.flags,
            property.type_code,
            property.count,
            property.multiplier,
            hex,
            if value.len() > 64 { " ..." } else { "" },
        );
    }
    for child in &block.children {
        dump_block(file, child, depth + 1);
    }
}
