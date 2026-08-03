use std::fs;
use std::path::{Path, PathBuf};

use srd_editor::animation::{Evaluation, Motion, ScalarValue, Track};
use srd_editor::image::{ImageDefinition, ImageReferenceChannel};
use srd_editor::number::NumberDefinition;
use srd_editor::scene::Layer;
use srd_editor::texture::TextureList;
use srd_editor::transform::Affine3x4;
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

#[test]
fn parses_binary_selected_layer_transform_records() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();

    let mut layer_count = 0usize;
    let mut node_count = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        for block in file
            .blocks_depth_first()
            .filter(|block| block.is_tag(b"LAYR"))
        {
            let layer = Layer::from_block(&file, block).unwrap_or_else(|error| {
                panic!("{} at {:#x}: {error}", path.display(), block.offset)
            });
            assert_eq!(layer.nodes.len(), layer.transforms.len());
            let hierarchy = layer.build_hierarchy().unwrap_or_else(|error| {
                panic!("{} at {:#x}: {error}", path.display(), block.offset)
            });
            assert_eq!(hierarchy.parents.len(), layer.nodes.len());
            assert_eq!(hierarchy.children.len(), layer.nodes.len());
            let transforms = layer
                .transforms
                .iter()
                .copied()
                .map(|transform| transform.spatial())
                .collect::<Vec<_>>();
            let offsets = vec![[0.0, 0.0]; layer.nodes.len()];
            let worlds = layer
                .compose_world_matrices(&transforms, Affine3x4::IDENTITY, false, &offsets)
                .unwrap_or_else(|error| {
                    panic!("{} at {:#x}: {error}", path.display(), block.offset)
                });
            assert_eq!(worlds.len(), layer.nodes.len());
            layer_count += 1;
            node_count += layer.nodes.len();
        }
    }
    assert!(layer_count > 0);
    assert!(node_count > 0);
}

#[test]
fn parses_and_links_binary_proven_csli_grids() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();

    let mut csli_count = 0usize;
    let mut cref_count = 0usize;
    let mut resolved_cell_crefs = 0usize;
    let mut active_color_cells = 0usize;
    let mut missing_3a = 0usize;
    let mut missing_33 = 0usize;
    let mut non_four_44 = 0usize;
    let mut texture_count = 0usize;
    let mut crop_count = 0usize;
    let mut nonnegative_cell_crefs = 0usize;
    let mut resolved_cell_textures = 0usize;
    let mut indexed_children = 0usize;
    let mut active_indexed_children = 0usize;
    let mut image_count = 0usize;
    let mut image_cref_count = 0usize;
    let mut image_cre1_count = 0usize;
    let mut resolved_image_channels = 0usize;
    let mut text_cast_count = 0usize;
    let mut number_count = 0usize;
    let mut number_cref_count = 0usize;
    let mut number_cre1_count = 0usize;
    let mut resolved_number_channels = 0usize;
    let mut valid_number_special_glyphs = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let textures = TextureList::from_file(&file).unwrap();
        if let Some(textures) = &textures {
            texture_count += textures.textures.len();
            crop_count += textures
                .textures
                .iter()
                .map(|texture| texture.crops.len())
                .sum::<usize>();
            for texture in &textures.textures {
                assert!(texture.filename.len() <= 255);
                assert_eq!(texture.crops.len(), texture.crop_count as usize);
            }
        }
        for block in file
            .blocks_depth_first()
            .filter(|block| block.is_tag(b"LAYR"))
        {
            let layer = Layer::from_block(&file, block).unwrap_or_else(|error| {
                panic!("{} at {:#x}: {error}", path.display(), block.offset)
            });
            let hierarchy = layer.build_hierarchy().unwrap();
            for (node_index, definition) in layer.csli_by_node.iter().enumerate() {
                let Some(definition) = definition else {
                    continue;
                };
                assert_eq!(layer.nodes[node_index].cast_type(), Some(2));
                assert_eq!(definition.cells.len(), definition.expected_cell_count());
                assert_eq!(definition.crefs.len(), usize::from(definition.cref_count));
                cref_count += definition.crefs.len();
                resolved_cell_crefs += (0..definition.cells.len())
                    .filter(|cell_index| definition.cell_cref(*cell_index).is_some())
                    .count();
                for cell_index in 0..definition.cells.len() {
                    let Some(cref) = definition.cell_cref(cell_index) else {
                        continue;
                    };
                    if cref.image_index >= 0 && cref.rectangle_index >= 0 {
                        nonnegative_cell_crefs += 1;
                        resolved_cell_textures += usize::from(
                            textures
                                .as_ref()
                                .and_then(|textures| {
                                    textures.resolve_slice_cell(definition, cell_index)
                                })
                                .is_some(),
                        );
                    }
                }
                for cell in definition
                    .cells
                    .iter()
                    .filter(|cell| (cell.flags >> 8) & 1 != 0)
                {
                    missing_3a += usize::from(cell.field_3a.is_none());
                    missing_33 += usize::from(cell.field_33.is_none());
                    non_four_44 += usize::from(cell.field_44.len() != 4);
                    active_color_cells += 1;
                }
                let explicit_first_row_widths = definition
                    .cells
                    .iter()
                    .take(usize::from(definition.columns))
                    .filter(|cell| cell.flags & 0x01 != 0)
                    .count();
                let explicit_first_column_heights = definition
                    .cells
                    .iter()
                    .step_by(usize::from(definition.columns).max(1))
                    .take(usize::from(definition.rows))
                    .filter(|cell| cell.flags & 0x02 != 0)
                    .count();
                assert_eq!(
                    usize::from(definition.explicit_width_cell_count),
                    explicit_first_row_widths
                );
                assert_eq!(
                    usize::from(definition.explicit_height_cell_count),
                    explicit_first_column_heights
                );
                assert_eq!(
                    definition.generate_cell_rects().unwrap().len(),
                    definition.expected_cell_count()
                );
                csli_count += 1;
            }
            for (node_index, definition) in layer.image_by_node.iter().enumerate() {
                let Some(definition) = definition else {
                    continue;
                };
                assert_eq!(layer.nodes[node_index].cast_type(), Some(1));
                image_count += 1;
                image_cref_count += definition.crefs.len();
                image_cre1_count += definition.cre1s.len();
                text_cast_count += usize::from(definition.creates_text_cast());
                for channel in [ImageReferenceChannel::Cref, ImageReferenceChannel::Cre1] {
                    let state = definition.initial_coordinate_state(channel);
                    let (declared_count, references) = match channel {
                        ImageReferenceChannel::Cref => {
                            (definition.cref_count, definition.crefs.as_slice())
                        }
                        ImageReferenceChannel::Cre1 => {
                            (definition.cre1_count, definition.cre1s.as_slice())
                        }
                    };
                    if state.reference_index < 0
                        || u32::from(state.reference_index as u16) >= u32::from(declared_count)
                        || references.is_empty()
                    {
                        continue;
                    }
                    let reference = references[state.reference_index as usize];
                    if reference.image_index < 0 || reference.rectangle_index < 0 {
                        continue;
                    }
                    definition
                        .resolve_coordinates(
                            channel,
                            state,
                            textures.as_ref().expect("CIMG reference requires TEXL"),
                            ImageDefinition::INITIAL_COORDINATE_OFFSET_SCALE,
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "{} CIMG NODE {node_index} {channel:?}: {error}",
                                path.display()
                            )
                        });
                    resolved_image_channels += 1;
                }
            }
            for (node_index, definition) in layer.number_by_node.iter().enumerate() {
                let Some(definition) = definition else {
                    continue;
                };
                assert_eq!(layer.nodes[node_index].cast_type(), Some(4));
                number_count += 1;
                number_cref_count += definition.crefs.len();
                number_cre1_count += definition.cre1s.len();
                let special = definition.special_glyphs();
                for glyph in [
                    special.plus,
                    special.minus,
                    special.comma,
                    special.decimal_point,
                ] {
                    if glyph >= 0 && u32::from(glyph as u16) < u32::from(definition.cref_count) {
                        valid_number_special_glyphs += 1;
                    }
                }
                let base = definition.image_base();
                for channel in [ImageReferenceChannel::Cref, ImageReferenceChannel::Cre1] {
                    let state = base.initial_coordinate_state(channel);
                    let (declared_count, references) = match channel {
                        ImageReferenceChannel::Cref => {
                            (definition.cref_count, definition.crefs.as_slice())
                        }
                        ImageReferenceChannel::Cre1 => {
                            (definition.cre1_count, definition.cre1s.as_slice())
                        }
                    };
                    if declared_count == 0 || references.is_empty() {
                        continue;
                    }
                    let reference = references[0];
                    if reference.image_index < 0 || reference.rectangle_index < 0 {
                        continue;
                    }
                    base.resolve_coordinates(
                        channel,
                        state,
                        textures.as_ref().expect("CNUM reference requires TEXL"),
                        NumberDefinition::INITIAL_COORDINATE_OFFSET_SCALE,
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} CNUM NODE {node_index} {channel:?}: {error}",
                            path.display()
                        )
                    });
                    resolved_number_channels += 1;
                }
            }
            for (node_index, node) in layer.nodes.iter().enumerate() {
                let Some(cell_index) = node.parent_csli_cell_index.filter(|index| *index >= 0)
                else {
                    continue;
                };
                let parent_index = hierarchy.parents[node_index].unwrap_or_else(|| {
                    panic!(
                        "{} NODE {node_index} has CSLI cell index but no parent",
                        path.display()
                    )
                });
                assert_eq!(layer.nodes[parent_index].cast_type(), Some(2));
                let definition = layer.csli_by_node[parent_index]
                    .as_ref()
                    .unwrap_or_else(|| {
                        panic!(
                            "{} parent NODE {parent_index} has no linked CSLI",
                            path.display()
                        )
                    });
                let cell = definition
                    .cells
                    .get(cell_index as usize)
                    .unwrap_or_else(|| {
                        panic!(
                            "{} NODE {node_index} indexes CSLI cell {cell_index} outside {} cells",
                            path.display(),
                            definition.cells.len()
                        )
                    });
                indexed_children += 1;
                active_indexed_children += usize::from((cell.flags >> 8) & 1 != 0);
            }
            let offsets = layer.compute_parent_csli_offsets().unwrap();
            assert_eq!(offsets.len(), layer.nodes.len());
            let transforms = layer
                .transforms
                .iter()
                .copied()
                .map(|transform| transform.spatial())
                .collect::<Vec<_>>();
            let worlds = layer
                .compose_world_matrices_with_csli_layout(&transforms, Affine3x4::IDENTITY, false)
                .unwrap();
            assert_eq!(worlds.len(), layer.nodes.len());
        }
    }
    eprintln!(
        "CSLI definitions={csli_count}, CREF records={cref_count}, resolved cell CREFs={resolved_cell_crefs}, CIMG definitions={image_count}, CIMG CREF records={image_cref_count}, CIMG CRE1 records={image_cre1_count}, resolved CIMG channels={resolved_image_channels}, text casts={text_cast_count}, CNUM definitions={number_count}, CNUM CREF records={number_cref_count}, CNUM CRE1 records={number_cre1_count}, resolved CNUM channels={resolved_number_channels}, valid CNUM special glyphs={valid_number_special_glyphs}, TEX records={texture_count}, CROP records={crop_count}, nonnegative cell CREFs={nonnegative_cell_crefs}, resolved cell textures={resolved_cell_textures}, active color cells={active_color_cells}, missing 0x3A={missing_3a}, missing 0x33={missing_33}, non-four 0x44={non_four_44}, indexed children={indexed_children}, active indexed children={active_indexed_children}"
    );
    assert!(csli_count > 0);
    assert!(cref_count > 0);
    assert!(resolved_cell_crefs > 0);
    assert!(image_count > 0);
    assert!(image_cref_count > 0);
    assert!(image_cre1_count > 0);
    assert!(resolved_image_channels > 0);
    assert!(number_count > 0);
    assert!(number_cref_count > 0);
    assert!(number_cre1_count > 0);
    assert!(resolved_number_channels > 0);
    assert!(valid_number_special_glyphs > 0);
    assert!(texture_count > 0);
    assert!(crop_count > 0);
    assert!(nonnegative_cell_crefs > 0);
    assert_eq!(resolved_cell_textures, nonnegative_cell_crefs);
    assert!(active_color_cells > 0);
    assert_eq!(missing_3a, 0);
    assert_eq!(missing_33, active_color_cells);
    assert_eq!(non_four_44, 0);
    assert!(indexed_children > 0);
}

#[test]
fn avatar_motion_targets_runtime_cast_index() {
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
    let motion = Motion::from_block(&file, motion).unwrap();
    assert_eq!(motion.target, 61);
    assert!(motion.tracks.iter().any(|track| track.target == 5));

    let layer_block = file
        .blocks_depth_first()
        .find(|block| {
            block.is_tag(b"LAYR")
                && block.children.iter().any(|child| {
                    child.is_tag(b"ANIM")
                        && child
                            .last_property(0x03)
                            .and_then(|property| property.string_bytes(&file))
                            == Some(b"001_Default_loop".as_slice())
                })
        })
        .expect("owning layer not found");
    let layer = Layer::from_block(&file, layer_block).unwrap();
    let mut transform = layer.transforms[61].spatial();
    assert!(motion.apply_proven_common_channels(&mut transform, 50.0) > 0);
    assert_eq!(transform.rotation[2], 349);
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
