use std::fs;
use std::path::{Path, PathBuf};

use srd_editor::animation::{Evaluation, ScalarValue, Track};
use srd_editor::vtbf::{Block, SrdFile};

fn corpus_root() -> PathBuf {
    std::env::var_os("SRD_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../diff-test/surfboard"))
}

fn collect_srd_files(path: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_srd_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "srd") {
            output.push(path);
        }
    }
}

#[test]
fn parses_local_corpus_with_binary_proven_boundaries() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    assert_eq!(files.len(), 53, "unexpected local corpus size");

    for path in files {
        let bytes = fs::read(&path).unwrap();
        SrdFile::parse(bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}

#[test]
fn avatar_track_uses_game_cubic_result() {
    let path = corpus_root()
        .join("common")
        .join("commonAvatar")
        .join("CHU_UI_Common_Avatar_Position_00.srd");
    if !path.exists() {
        eprintln!("skipping: avatar sample not found at {}", path.display());
        return;
    }
    let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
    let animation = file
        .blocks_depth_first()
        .find(|block| {
            block.is_tag(b"ANIM")
                && block
                    .last_property(0x03)
                    .and_then(|property| property.string_bytes(&file))
                    == Some(b"001_Default_loop".as_slice())
        })
        .expect("animation not found");
    let motion = animation
        .children
        .iter()
        .find(|block| block.is_tag(b"MOT ") && signed_property(&file, block, 0x51) == Some(61))
        .expect("motion not found");
    let track_block = motion
        .children
        .iter()
        .find(|block| block.is_tag(b"TRK ") && unsigned_property(&file, block, 0x53) == Some(5))
        .expect("track not found");
    let track = Track::from_block(&file, track_block).unwrap();

    assert_eq!(track.format, 0x143);
    assert_eq!(track.key_count, 4);
    assert_eq!(
        track.evaluate(50.0),
        Evaluation::Value(ScalarValue::I32(349))
    );
}

fn unsigned_property(file: &SrdFile, block: &Block, code: u8) -> Option<u32> {
    block
        .last_property(code)
        .and_then(|property| property.read_unsigned_scalar(file))
}

fn signed_property(file: &SrdFile, block: &Block, code: u8) -> Option<i32> {
    block
        .last_property(code)
        .and_then(|property| property.read_signed_scalar(file))
}
