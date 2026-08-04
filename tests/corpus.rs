use std::fs;
use std::path::{Path, PathBuf};

use srd_editor::animation::{Evaluation, KeyData, Motion, ScalarValue, Track};
use srd_editor::attribute::CastAttributeValue;
use srd_editor::csli::CsliDefinition;
use srd_editor::dds::{
    D3d9Direct2dUpload, D3d9TextureCreation, DdsDescriptor, DdsLoadPolicy, GameTextureFormat,
};
use srd_editor::editor_document::EditorDocument;
use srd_editor::image::{ImageDefinition, ImageReferenceChannel};
use srd_editor::number::NumberDefinition;
use srd_editor::reference_runtime::{ProjectRuntime, ReferenceLayerRuntimeState};
use srd_editor::render::{
    CeylonDrawPacketPresetState, apply_srd_image_field_0c_shader_bits,
    select_srd_image_render_preset,
};
use srd_editor::scene::{Layer, Project, ReferenceTarget};
use srd_editor::shader::{CEYLON_SIMPLE_SHADER_KEY_LENGTH, CeylonSimpleShaderBits};
use srd_editor::shader_bytecode::FIRST_FIXTURE_SIMPLE_KEY;
use srd_editor::srd_draw::build_evidence_complete_initial_image_draws;
use srd_editor::texture::TextureList;
use srd_editor::transform::Affine3x4;
use srd_editor::vtbf::{Block, SrdFile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CorpusProfile {
    Legacy53,
    Complete91,
}

fn srd_corpus_profile(file_count: usize) -> CorpusProfile {
    match file_count {
        53 => CorpusProfile::Legacy53,
        91 => CorpusProfile::Complete91,
        _ => panic!("unexpected local SRD corpus size: {file_count}"),
    }
}

fn dds_corpus_profile(file_count: usize) -> CorpusProfile {
    match file_count {
        97 => CorpusProfile::Legacy53,
        360 => CorpusProfile::Complete91,
        _ => panic!("unexpected local DDS corpus size: {file_count}"),
    }
}

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

fn collect_dds_files(path: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_dds_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "dds") {
            output.push(path);
        }
    }
}

#[test]
fn validates_complete_game_simple_shader_key_collection() {
    let Some(root) = std::env::var_os("GAME_DATA_CORPUS").map(PathBuf::from) else {
        eprintln!("skipping: GAME_DATA_CORPUS is not set");
        return;
    };
    let xml = fs::read_to_string(root.join("A000/shader/shadercollect.xml")).unwrap();
    let mut inside_simple_group = false;
    let mut keys = Vec::new();

    for line in xml.lines().map(str::trim) {
        if line.starts_with("<SimpleShaderVSSimpleShaderPS_") {
            inside_simple_group = true;
            continue;
        }
        if line.starts_with("</SimpleShaderVSSimpleShaderPS_") {
            inside_simple_group = false;
            continue;
        }
        if !inside_simple_group || !line.starts_with('<') || !line.ends_with("/>") {
            continue;
        }

        let encoded = &line.as_bytes()[1..line.len() - 2];
        let key: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] = encoded.try_into().unwrap();
        let bits = CeylonSimpleShaderBits::from_compact_key(key);
        assert_eq!(bits.compact_key(), key);
        keys.push(bits);
    }

    assert_eq!(keys.len(), 82);
    let multi_tex0_counts = keys.into_iter().fold([0_usize; 16], |mut counts, bits| {
        let mut value = 0;
        for bit in 0..4 {
            value |= usize::from(bits.contains(47 + bit)) << bit;
        }
        counts[value] += 1;
        counts
    });
    assert_eq!(multi_tex0_counts[0], 58);
    assert_eq!(multi_tex0_counts[6], 1);
    assert_eq!(multi_tex0_counts[9], 1);
    assert_eq!(multi_tex0_counts[10], 1);
    assert_eq!(multi_tex0_counts[11], 10);
    assert_eq!(multi_tex0_counts[12], 11);
}

#[test]
fn builds_the_first_evidence_complete_srd_draw() {
    let Some(root) = std::env::var_os("GAME_DATA_CORPUS").map(PathBuf::from) else {
        eprintln!("skipping: GAME_DATA_CORPUS is not set");
        return;
    };
    let document =
        EditorDocument::load(root.join("surfboard/system/CHU_UI_System_00_v10.srd")).unwrap();
    let draws = build_evidence_complete_initial_image_draws(
        &document.project,
        &document.textures,
        0,
        Affine3x4::IDENTITY,
        1920.0,
    )
    .unwrap();
    assert_eq!(draws.len(), 1);
    let draw = draws[0];
    assert_eq!((draw.layer_index, draw.node_index), (0, 1));
    assert_eq!(draw.shader_key, FIRST_FIXTURE_SIMPLE_KEY);
    assert_eq!(
        draw.quad.vertices.map(|vertex| vertex.position),
        [
            [0.0, 0.0, 0.0],
            [0.0, 1080.0, 0.0],
            [1920.0, 0.0, 0.0],
            [1920.0, 1080.0, 0.0],
        ]
    );
    assert!(
        draw.quad
            .vertices
            .iter()
            .all(|vertex| vertex.primary_color == [0, 0, 0, 255])
    );
    assert!(
        draw.quad
            .vertices
            .iter()
            .all(|vertex| vertex.secondary_color == [0; 4])
    );
    assert_eq!(draw.raster.color_write_mask, 0x0f);
    assert!(!draw.depth.z_enabled);
    assert!(!draw.blend.alpha_test_enabled);
    assert_eq!(draw.packet.flags_0c & 0x100, 0);
}

#[test]
fn parses_complete_game_dds_corpus_with_binary_resource_rules() {
    let Some(root) = std::env::var_os("GAME_DATA_CORPUS").map(PathBuf::from) else {
        eprintln!("skipping: GAME_DATA_CORPUS is not set");
        return;
    };
    let mut files = Vec::new();
    collect_dds_files(&root, &mut files);
    files.sort();

    let mut format_counts = std::collections::BTreeMap::new();
    let mut mip_counts = std::collections::BTreeMap::new();
    let mut cube_counts = std::collections::BTreeMap::new();
    let mut plan_counts = std::collections::BTreeMap::new();
    for path in &files {
        let bytes = fs::read(path).unwrap();
        let descriptor = DdsDescriptor::parse(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        descriptor
            .validate_data_len(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(descriptor.required_data_len().unwrap(), bytes.len());
        *format_counts.entry(descriptor.format.0).or_insert(0usize) += 1;
        *mip_counts.entry(descriptor.mip_count).or_insert(0usize) += 1;
        *cube_counts.entry(descriptor.is_cube).or_insert(0usize) += 1;
        let plan = match descriptor.creation_plan(DdsLoadPolicy::default()) {
            D3d9TextureCreation::Direct2d { .. } => "direct_2d",
            D3d9TextureCreation::DirectCube { .. } => "direct_cube",
            D3d9TextureCreation::D3dx2d { .. } => "game_d3dx_2d",
            D3d9TextureCreation::D3dxCube { .. } => "game_d3dx_cube",
        };
        *plan_counts.entry(plan).or_insert(0usize) += 1;
    }

    eprintln!(
        "complete game DDS files={}, formats={format_counts:?}, mips={mip_counts:?}, cubes={cube_counts:?}, plans={plan_counts:?}",
        files.len()
    );
    assert_eq!(files.len(), 14_694);
    assert_eq!(
        format_counts,
        [
            (GameTextureFormat::A8_R8_G8_B8.0, 17),
            (GameTextureFormat::DXT1.0, 1_830),
            (GameTextureFormat::DXT5.0, 12_847),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(mip_counts, [(1, 14_690), (8, 4)].into_iter().collect());
    assert_eq!(cube_counts, [(false, 14_694)].into_iter().collect());
    assert_eq!(
        plan_counts,
        [("direct_2d", 7_490), ("game_d3dx_2d", 7_204)]
            .into_iter()
            .collect()
    );
}

#[test]
fn parses_local_dds_corpus_with_the_binary_resource_rules() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_dds_files(&root, &mut files);
    files.sort();
    let profile = dds_corpus_profile(files.len());

    let mut format_counts = std::collections::BTreeMap::new();
    let mut mip_counts = std::collections::BTreeMap::new();
    let mut direct_count = 0usize;
    let mut d3dx_count = 0usize;
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let descriptor = DdsDescriptor::parse(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        descriptor
            .validate_data_len(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(descriptor.required_data_len().unwrap(), bytes.len());
        assert!(!descriptor.is_cube);
        *format_counts.entry(descriptor.format.0).or_insert(0usize) += 1;
        *mip_counts.entry(descriptor.mip_count).or_insert(0usize) += 1;
        match descriptor.creation_plan(DdsLoadPolicy::default()) {
            D3d9TextureCreation::Direct2d { .. } => {
                direct_count += 1;
                let uploads = descriptor
                    .direct_2d_upload_plan()
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                assert!(!uploads.is_empty(), "{}", path.display());
                for upload in uploads {
                    match upload {
                        D3d9Direct2dUpload::UpdateSurface(upload) => {
                            let end = usize::try_from(upload.source_offset).unwrap()
                                + usize::try_from(upload.source_byte_len).unwrap();
                            assert!(end <= bytes.len(), "{}", path.display());
                            assert_eq!(upload.staging_pool, 2);
                            assert_eq!(upload.staging_lock_flags, 0);
                        }
                        D3d9Direct2dUpload::CompressedLevelBelowFourSkipped {
                            width,
                            height,
                            ..
                        } => assert!(width < 4 || height < 4),
                    }
                }
            }
            D3d9TextureCreation::D3dx2d { .. } => d3dx_count += 1,
            plan => panic!("{} unexpectedly produced {plan:?}", path.display()),
        }
    }

    eprintln!(
        "DDS profile={profile:?}, formats={format_counts:?}, mips={mip_counts:?}, direct={direct_count}, fallback={d3dx_count}"
    );
    let expected_formats = match profile {
        CorpusProfile::Legacy53 => [
            (GameTextureFormat::A8_R8_G8_B8.0, 70),
            (GameTextureFormat::DXT5.0, 27),
        ]
        .into_iter()
        .collect(),
        CorpusProfile::Complete91 => [
            (GameTextureFormat::A8_R8_G8_B8.0, 2),
            (GameTextureFormat::DXT1.0, 7),
            (GameTextureFormat::DXT5.0, 351),
        ]
        .into_iter()
        .collect(),
    };
    assert_eq!(format_counts, expected_formats);
    match profile {
        CorpusProfile::Legacy53 => {
            assert_eq!(mip_counts, [(1, 96), (10, 1)].into_iter().collect());
            assert_eq!(direct_count, 93);
            assert_eq!(d3dx_count, 4);
        }
        CorpusProfile::Complete91 => {
            assert_eq!(mip_counts, [(1, 360)].into_iter().collect());
            assert_eq!(direct_count, 234);
            assert_eq!(d3dx_count, 126);
        }
    }
}

#[test]
fn image_cast_flags_select_only_binary_proven_render_presets_in_the_local_corpus() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut cast_type_counts = [0usize; 3];
    let mut normal_preset_counts = std::collections::BTreeMap::new();
    let mut special_preset_counts = std::collections::BTreeMap::new();
    let mut invalid_low_nibbles = std::collections::BTreeMap::new();
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        for block in file.blocks_depth_first() {
            let (kind, flags, runtime) = if block.is_tag(b"CIMG") {
                let definition = ImageDefinition::from_block(&file, block).unwrap();
                (0, definition.flags, definition.initial_runtime_state())
            } else if block.is_tag(b"CSLI") {
                let definition = CsliDefinition::from_block(&file, block).unwrap();
                let image = ImageDefinition::from_csli_runtime_base(&definition);
                (1, image.flags, image.initial_runtime_state())
            } else if block.is_tag(b"CNUM") {
                let definition = NumberDefinition::from_block(&file, block).unwrap();
                let image = definition.image_base();
                (2, image.flags, image.initial_runtime_state())
            } else {
                continue;
            };
            cast_type_counts[kind] += 1;
            assert_eq!(runtime.field_10, 0);
            assert_eq!(runtime.field_14, 0);
            assert_eq!(runtime.field_18, 0);
            assert_eq!(runtime.render_preset_override, -1);
            assert_eq!(runtime.field_1c, -1);

            match select_srd_image_render_preset(flags, runtime.render_preset_override, false) {
                Some(preset) => *normal_preset_counts.entry(preset).or_insert(0usize) += 1,
                None => *invalid_low_nibbles.entry(flags & 0x0f).or_insert(0usize) += 1,
            }
            if let Some(preset) =
                select_srd_image_render_preset(flags, runtime.render_preset_override, true)
            {
                *special_preset_counts.entry(preset).or_insert(0usize) += 1;
            }
        }
    }

    eprintln!(
        "profile={profile:?}, CIMG/CSLI/CNUM={cast_type_counts:?}, normal presets={normal_preset_counts:?}, special presets={special_preset_counts:?}, unchanged low nibbles={invalid_low_nibbles:?}"
    );
    assert_eq!(
        cast_type_counts,
        match profile {
            CorpusProfile::Legacy53 => [13_773, 799, 552],
            CorpusProfile::Complete91 => [17_867, 983, 634],
        }
    );
    assert_eq!(
        normal_preset_counts,
        match profile {
            CorpusProfile::Legacy53 => [(3, 10_659), (4, 4_242), (5, 43), (9, 180)],
            CorpusProfile::Complete91 => [(3, 13_779), (4, 5_436), (5, 48), (9, 221)],
        }
        .into_iter()
        .collect()
    );
    assert_eq!(special_preset_counts, normal_preset_counts);
    assert!(invalid_low_nibbles.is_empty());
}

#[test]
fn cast_channel_23_only_targets_reference_casts() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut count = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        for block in file
            .blocks_depth_first()
            .filter(|block| block.is_tag(b"LAYR"))
        {
            let layer = Layer::from_block(&file, block).unwrap();
            for animation in block.children.iter().filter(|child| child.is_tag(b"ANIM")) {
                for motion_block in animation
                    .children
                    .iter()
                    .filter(|child| child.is_tag(b"MOT "))
                {
                    let motion = Motion::from_block(&file, motion_block).unwrap();
                    let Ok(node_index) = usize::try_from(motion.target) else {
                        continue;
                    };
                    let Some(node) = layer.nodes.get(node_index) else {
                        continue;
                    };
                    for track in motion.tracks.iter().filter(|track| track.target == 23) {
                        assert_eq!(node.cast_type(), Some(3));
                        assert_eq!(track.format, 0x13);
                        assert!(matches!(track.keys, KeyData::Key20F32(_)));
                        count += 1;
                    }
                }
            }
        }
    }
    assert_eq!(
        count,
        match profile {
            CorpusProfile::Legacy53 => 111,
            CorpusProfile::Complete91 => 118,
        }
    );
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
    srd_corpus_profile(files.len());

    for path in files {
        let bytes = fs::read(&path).unwrap();
        SrdFile::parse(bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}

#[test]
fn parses_cast_attribute_lists_and_ext_params() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut list_count = 0usize;
    let mut attached_node_count = 0usize;
    let mut attribute_count = 0usize;
    let mut ext_param_count = 0usize;
    let mut render_preset_overrides = std::collections::BTreeMap::new();
    let mut image_override_counts = std::collections::BTreeMap::new();
    let mut effective_image_presets = std::collections::BTreeMap::new();
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let project = Project::from_file(&file).unwrap();
        for layer in project.scenes.iter().flat_map(|scene| &scene.layers) {
            list_count += layer.cast_attribute_lists.len();
            attached_node_count += layer.cast_attribute_list_by_node.iter().flatten().count();
            for list in &layer.cast_attribute_lists {
                assert!(list.attributes.len() <= list.declared_count as usize);
                attribute_count += list.attributes.len();
                for attribute in &list.attributes {
                    if let CastAttributeValue::ExtParam { source, parsed } = &attribute.value {
                        assert_eq!(*parsed, srd_editor::attribute::ExtParamData::parse(source));
                        ext_param_count += 1;
                        *render_preset_overrides
                            .entry(parsed.render_preset_override)
                            .or_insert(0usize) += 1;
                    }
                }
            }
            for (node_index, list_index) in layer.cast_attribute_list_by_node.iter().enumerate() {
                if let Some(list_index) = list_index {
                    assert_eq!(
                        layer.ext_param_for_node(node_index),
                        layer.cast_attribute_lists[*list_index].ext_param()
                    );
                }
                let image = match layer.nodes[node_index].cast_type() {
                    Some(1) => layer.image_by_node[node_index].clone(),
                    Some(2) => layer.csli_by_node[node_index]
                        .as_ref()
                        .map(ImageDefinition::from_csli_runtime_base),
                    Some(4) => layer.number_by_node[node_index]
                        .as_ref()
                        .map(NumberDefinition::image_base),
                    _ => None,
                };
                let Some(image) = image else {
                    continue;
                };
                let override_value = layer
                    .ext_param_for_node(node_index)
                    .map_or(-1, |ext_param| ext_param.render_preset_override);
                *image_override_counts
                    .entry(override_value)
                    .or_insert(0usize) += 1;
                if let Some(preset) =
                    select_srd_image_render_preset(image.flags, override_value, false)
                {
                    *effective_image_presets.entry(preset).or_insert(0usize) += 1;
                }
            }
        }
    }

    let (expected_lists, expected_attributes, expected_overrides) = match profile {
        CorpusProfile::Legacy53 => (
            22_579,
            53_179,
            [
                (-1, 22_398),
                (41, 7),
                (42, 1),
                (43, 3),
                (53, 24),
                (54, 11),
                (57, 1),
                (58, 20),
                (60, 114),
            ]
            .into_iter()
            .collect(),
        ),
        CorpusProfile::Complete91 => (
            29_138,
            68_511,
            [
                (-1, 28_863),
                (34, 2),
                (35, 2),
                (36, 3),
                (37, 2),
                (38, 2),
                (39, 2),
                (40, 2),
                (41, 9),
                (42, 3),
                (43, 5),
                (44, 2),
                (45, 2),
                (46, 2),
                (47, 2),
                (48, 2),
                (49, 2),
                (50, 2),
                (51, 2),
                (52, 2),
                (53, 32),
                (54, 13),
                (55, 2),
                (56, 2),
                (57, 4),
                (58, 22),
                (60, 150),
            ]
            .into_iter()
            .collect(),
        ),
    };
    assert_eq!(list_count, expected_lists);
    assert_eq!(attached_node_count, expected_lists);
    assert_eq!(attribute_count, expected_attributes);
    assert_eq!(ext_param_count, expected_lists);
    assert_eq!(render_preset_overrides, expected_overrides);
    eprintln!(
        "CATR profile={profile:?}, lists={list_count}, attached nodes={attached_node_count}, attributes={attribute_count}, ExtParamData={ext_param_count}, overrides={render_preset_overrides:?}, image overrides={image_override_counts:?}, effective image presets={effective_image_presets:?}"
    );
}

#[test]
fn initial_srd_image_draws_select_binary_shader_keys() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut image_count = 0usize;
    let mut field_0c_counts = std::collections::BTreeMap::new();
    let mut texture_presence_counts = std::collections::BTreeMap::new();
    let mut shader_keys = std::collections::BTreeMap::new();
    let mut direct_simple_keys = std::collections::BTreeMap::new();
    let mut position_2_only_simple_keys = std::collections::BTreeMap::new();
    let mut uncovered_simple_keys = std::collections::BTreeMap::new();
    let mut base_variants = std::collections::BTreeMap::new();
    let mut optional_modules = std::collections::BTreeMap::new();
    let mut multi_tex0_variants = std::collections::BTreeMap::new();
    let mut multi_tex1_variants = std::collections::BTreeMap::new();
    let shader_collection = std::env::var_os("GAME_DATA_CORPUS").map(|root| {
        let xml =
            fs::read_to_string(PathBuf::from(root).join("A000/shader/shadercollect.xml")).unwrap();
        let mut inside_simple_group = false;
        xml.lines()
            .map(str::trim)
            .filter_map(|line| {
                if line.starts_with("<SimpleShaderVSSimpleShaderPS_") {
                    inside_simple_group = true;
                    return None;
                }
                if line.starts_with("</SimpleShaderVSSimpleShaderPS_") {
                    inside_simple_group = false;
                    return None;
                }
                if !inside_simple_group || !line.starts_with('<') || !line.ends_with("/>") {
                    return None;
                }
                let key: [u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH] =
                    line.as_bytes()[1..line.len() - 2].try_into().ok()?;
                Some(key)
            })
            .collect::<std::collections::BTreeSet<_>>()
    });
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let textures = TextureList::from_file(&file)
            .unwrap()
            .unwrap_or(TextureList {
                declared_count: 0,
                textures: Vec::new(),
            });
        let project = Project::from_file(&file).unwrap();
        for layer in project.scenes.iter().flat_map(|scene| &scene.layers) {
            for node_index in 0..layer.nodes.len() {
                let Some(image) = (match layer.nodes[node_index].cast_type() {
                    Some(1) => layer.image_by_node[node_index].clone(),
                    Some(2) => layer.csli_by_node[node_index]
                        .as_ref()
                        .map(ImageDefinition::from_csli_runtime_base),
                    Some(4) => layer.number_by_node[node_index]
                        .as_ref()
                        .map(NumberDefinition::image_base),
                    _ => None,
                }) else {
                    continue;
                };
                let mut state = image.initial_runtime_state();
                if let Some(ext_param) = layer.ext_param_for_node(node_index) {
                    state.render_preset_override = ext_param.render_preset_override;
                }
                let Some(preset) = select_srd_image_render_preset(
                    image.flags,
                    state.render_preset_override,
                    false,
                ) else {
                    continue;
                };
                let slots = image
                    .resolve_texture_slots(
                        &state,
                        &textures,
                        ImageDefinition::INITIAL_COORDINATE_OFFSET_SCALE,
                        [false; 2],
                    )
                    .unwrap_or_else(|error| {
                        panic!("{} NODE {node_index}: {error}", path.display())
                    });
                let texture_present = slots.texture_present();
                let presence_mask = texture_present
                    .iter()
                    .enumerate()
                    .fold(0u8, |mask, (slot, present)| {
                        mask | (u8::from(*present) << slot)
                    });

                let mut packet = CeylonDrawPacketPresetState::srd_renderer_initial();
                packet.set_render_preset_id(preset);
                apply_srd_image_field_0c_shader_bits(&mut packet, state.field_0c as i32);
                let key = packet.srd_quad_shader_key(texture_present);
                let direct_simple_key = key
                    .srd_simple_shader_direct_contributions()
                    .unwrap()
                    .compact_key();
                *direct_simple_keys
                    .entry(direct_simple_key)
                    .or_insert(0usize) += 1;
                // The collection is an asset inventory, not proof that this
                // draw reaches a particular runtime context. Keep membership
                // differences diagnostic-only until the executable provider
                // chain has established every runtime contribution.
                if let Some(shader_collection) = &shader_collection
                    && !shader_collection.contains(&direct_simple_key)
                {
                    let mut position_2_bits =
                        CeylonSimpleShaderBits::from_compact_key(direct_simple_key);
                    position_2_bits.set(2, true);
                    let position_2_key = position_2_bits.compact_key();
                    let target = if shader_collection.contains(&position_2_key) {
                        &mut position_2_only_simple_keys
                    } else {
                        &mut uncovered_simple_keys
                    };
                    let entry = target
                        .entry(direct_simple_key)
                        .or_insert_with(|| (0usize, path.clone(), node_index));
                    entry.0 += 1;
                }
                *field_0c_counts.entry(state.field_0c).or_insert(0usize) += 1;
                *texture_presence_counts
                    .entry(presence_mask)
                    .or_insert(0usize) += 1;
                *shader_keys.entry(key).or_insert(0usize) += 1;
                *base_variants.entry(key.low & 7).or_insert(0usize) += 1;
                *optional_modules.entry((key.low >> 3) & 1).or_insert(0usize) += 1;
                *multi_tex0_variants
                    .entry((key.low >> 26) & 0x0f)
                    .or_insert(0usize) += 1;
                *multi_tex1_variants.entry(key.high & 7).or_insert(0usize) += 1;
                image_count += 1;
            }
        }
    }

    let (expected_images, expected_field_0c, expected_texture_masks, expected_key_count) =
        match profile {
            CorpusProfile::Legacy53 => (
                15_124,
                [(0, 14_534), (1, 3), (2, 3), (3, 79), (4, 505)]
                    .into_iter()
                    .collect(),
                [(0, 2_125), (1, 12_433), (2, 12), (3, 554)]
                    .into_iter()
                    .collect(),
                23,
            ),
            CorpusProfile::Complete91 => (
                19_484,
                [(0, 18_720), (1, 3), (2, 3), (3, 89), (4, 669)]
                    .into_iter()
                    .collect(),
                [(0, 2_387), (1, 16_357), (2, 22), (3, 718)]
                    .into_iter()
                    .collect(),
                65,
            ),
        };
    assert_eq!(image_count, expected_images);
    assert_eq!(field_0c_counts, expected_field_0c);
    assert_eq!(texture_presence_counts, expected_texture_masks);
    assert_eq!(shader_keys.len(), expected_key_count);
    assert_eq!(base_variants, [(0, image_count)].into_iter().collect());
    assert_eq!(optional_modules, [(0, image_count)].into_iter().collect());
    assert_eq!(
        multi_tex0_variants,
        [(0, image_count - 3), (9, 3)].into_iter().collect()
    );
    assert_eq!(
        multi_tex1_variants,
        [(0, image_count)].into_iter().collect()
    );
    eprintln!(
        "shader-key profile={profile:?}, images={image_count}, field_0c={field_0c_counts:?}, texture masks={texture_presence_counts:?}, distinct ShapeEnv keys={}, distinct direct Simple keys={}, XML keys represented only by a position-2 counterpart={}, XML-unrepresented direct keys={}, MultiTex0={multi_tex0_variants:?}",
        shader_keys.len(),
        direct_simple_keys.len(),
        position_2_only_simple_keys.len(),
        uncovered_simple_keys.len(),
    );
}

#[test]
fn reference_casts_resolve_inside_the_binary_project_scene_table() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut scene_count = 0usize;
    let mut reference_count = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let project =
            Project::from_file(&file).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        scene_count += project.scenes.len();
        for scene in &project.scenes {
            for layer in &scene.layers {
                for reference in layer.reference_by_node.iter().flatten() {
                    let target = project.resolve_reference(reference).unwrap_or_else(|| {
                        panic!(
                            "{}: unresolved CRFD scene {:?}, layer {:?}",
                            path.display(),
                            String::from_utf8_lossy(&reference.source_name),
                            String::from_utf8_lossy(&reference.layer_name)
                        )
                    });
                    assert_eq!(
                        project.scenes[target.scene_index].name,
                        reference.source_name
                    );
                    assert_eq!(
                        project.scenes[target.scene_index].layers[target.layer_index].name,
                        reference.layer_name
                    );
                    reference_count += 1;
                }
            }
        }
    }

    assert!(scene_count > 0);
    assert_eq!(
        reference_count,
        match profile {
            CorpusProfile::Legacy53 => 1_090,
            CorpusProfile::Complete91 => 1_299,
        }
    );
    eprintln!("project scenes={scene_count}, resolved CRFD references={reference_count}");
}

#[test]
fn reference_runtime_construction_converges_for_the_local_corpus() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut definition_count = 0usize;
    let mut instance_count = 0usize;
    let mut multiply_instanced_targets = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let project =
            Project::from_file(&file).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        definition_count += project
            .scenes
            .iter()
            .flat_map(|scene| &scene.layers)
            .flat_map(|layer| &layer.reference_by_node)
            .flatten()
            .count();
        let plan = project
            .build_reference_runtime_plan()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(plan.unresolved.is_empty(), "{}", path.display());
        instance_count += plan.instances.len();

        let mut target_counts = std::collections::HashMap::new();
        for instance in &plan.instances {
            *target_counts.entry(instance.target).or_insert(0usize) += 1;
        }
        multiply_instanced_targets += target_counts.values().filter(|count| **count > 1).count();
    }

    assert_eq!(
        definition_count,
        match profile {
            CorpusProfile::Legacy53 => 1_090,
            CorpusProfile::Complete91 => 1_299,
        }
    );
    assert!(instance_count >= definition_count);
    assert!(multiply_instanced_targets > 0);
    eprintln!(
        "CRFD definitions={definition_count}, runtime reference layers={instance_count}, multiply-instanced targets={multiply_instanced_targets}"
    );
}

#[test]
fn reference_instances_apply_the_binary_cast_channel_dispatch() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();
    let profile = srd_corpus_profile(files.len());

    let mut instance_count = 0usize;
    let mut animation_count = 0usize;
    let mut common_channels = 0usize;
    let mut image_channels = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let project = Project::from_file(&file).unwrap();
        let textures = TextureList::from_file(&file)
            .unwrap()
            .unwrap_or(TextureList {
                declared_count: 0,
                textures: Vec::new(),
            });
        let plan = project
            .build_reference_runtime_plan()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for instance_index in 0..plan.instances.len() {
            let mut runtime =
                ReferenceLayerRuntimeState::new(&project, &plan, instance_index).unwrap();
            let target = plan.instances[instance_index].target;
            let layer = &project.scenes[target.scene_index].layers[target.layer_index];
            assert_eq!(runtime.image_bases.len(), layer.nodes.len());
            assert_eq!(runtime.image_states.len(), layer.nodes.len());
            for (node_index, state) in runtime.image_states.iter().enumerate() {
                assert_eq!(
                    state.render_preset_override,
                    layer
                        .ext_param_for_node(node_index)
                        .map_or(-1, |ext_param| ext_param.render_preset_override)
                );
            }
            let animation_names = layer
                .animations
                .iter()
                .map(|animation| animation.name.clone())
                .collect::<Vec<_>>();
            for animation_name in animation_names {
                let application = runtime
                    .apply_animation_channels(&project, &plan, &textures, &animation_name, 0.0)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} instance {instance_index} animation {:?}: {error}",
                            path.display(),
                            String::from_utf8_lossy(&animation_name)
                        )
                    })
                    .expect("animation disappeared from its owning layer");
                common_channels += application.common_channels;
                image_channels += application.image_channels;
                animation_count += 1;
            }
            instance_count += 1;
        }
    }

    assert_eq!(
        instance_count,
        match profile {
            CorpusProfile::Legacy53 => 2_087,
            CorpusProfile::Complete91 => 2_365,
        }
    );
    assert!(animation_count > 0);
    assert!(common_channels > 0);
    assert!(image_channels > 0);
    eprintln!(
        "reference instances={instance_count}, animations={animation_count}, common channels={common_channels}, SrImage channels={image_channels}"
    );
}

#[test]
fn reference_channel_23_recurses_through_real_runtime_instances() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();

    let mut root_applications = 0usize;
    let mut reference_requests = 0usize;
    let mut recursively_animated_layers = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let project = Project::from_file(&file).unwrap();
        let textures = TextureList::from_file(&file)
            .unwrap()
            .unwrap_or(TextureList {
                declared_count: 0,
                textures: Vec::new(),
            });
        let mut runtime = ProjectRuntime::new(&project)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for (scene_index, scene) in project.scenes.iter().enumerate() {
            for (layer_index, layer) in scene.layers.iter().enumerate() {
                let target = ReferenceTarget {
                    scene_index,
                    layer_index,
                };
                let animation_names = layer
                    .animations
                    .iter()
                    .map(|animation| animation.name.clone())
                    .collect::<Vec<_>>();
                for animation_name in animation_names {
                    let application = runtime
                        .apply_layer_animation(&project, &textures, target, &animation_name, 0.0)
                        .unwrap_or_else(|error| {
                            panic!(
                                "{} SCN[{scene_index}]/LAYR[{layer_index}] animation {:?}: {error}",
                                path.display(),
                                String::from_utf8_lossy(&animation_name)
                            )
                        })
                        .expect("animation disappeared from its owning layer");
                    root_applications += 1;
                    reference_requests += application.reference_requests;
                    recursively_animated_layers += application.animated_layers;
                }
            }
        }
    }

    assert!(root_applications > 0);
    assert!(reference_requests > 0);
    assert!(recursively_animated_layers > root_applications);
    eprintln!(
        "root applications={root_applications}, channel 23 requests={reference_requests}, recursively animated layers={recursively_animated_layers}"
    );
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
fn parses_and_animates_common_transform_colors() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();

    let mut non_identity_multiply = 0usize;
    let mut nonzero_additive = 0usize;
    let mut channel_counts = [0usize; 4];
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        for block in file
            .blocks_depth_first()
            .filter(|block| block.is_tag(b"LAYR"))
        {
            let layer = Layer::from_block(&file, block).unwrap();
            for transform in layer.transforms.iter().copied().map(|raw| raw.spatial()) {
                non_identity_multiply += usize::from(transform.multiply_color != [255; 4]);
                nonzero_additive += usize::from(transform.additive_color != [0; 4]);
            }

            for animation in block.children.iter().filter(|child| child.is_tag(b"ANIM")) {
                for motion_block in animation
                    .children
                    .iter()
                    .filter(|child| child.is_tag(b"MOT "))
                {
                    let motion = Motion::from_block(&file, motion_block).unwrap();
                    let Ok(node_index) = usize::try_from(motion.target) else {
                        continue;
                    };
                    let Some(base) = layer.transforms.get(node_index).copied() else {
                        continue;
                    };
                    for track in &motion.tracks {
                        let channel_index = match track.target {
                            9 => 0,
                            19 => 1,
                            21 => 2,
                            22 => 3,
                            _ => continue,
                        };
                        let mut transform = base.spatial();
                        assert!(transform.apply_common_track(
                            track.target,
                            track.evaluate(track.range_start as f32)
                        ));
                        channel_counts[channel_index] += 1;
                    }
                }
            }
        }
    }

    assert!(non_identity_multiply > 0);
    assert!(nonzero_additive > 0);
    assert!(channel_counts.iter().all(|count| *count > 0));
    eprintln!(
        "non-identity multiply colors={non_identity_multiply}, nonzero additive colors={nonzero_additive}, channels 9/19/21/22={channel_counts:?}"
    );
}

#[test]
fn parses_runtime_animation_slots_names_and_durations() {
    let root = corpus_root();
    if !root.exists() {
        eprintln!("skipping: SRD corpus not found at {}", root.display());
        return;
    }
    let mut files = Vec::new();
    collect_srd_files(&root, &mut files);
    files.sort();

    let mut animation_count = 0usize;
    let mut empty_motion_slots = 0usize;
    let mut automatic_durations = 0usize;
    let mut applied_common_channels = 0usize;
    for path in files {
        let file = SrdFile::parse(fs::read(&path).unwrap()).unwrap();
        let project = Project::from_file(&file).unwrap();
        for scene in &project.scenes {
            for layer in &scene.layers {
                assert_eq!(layer.animations.len(), layer.animation_count as usize);
                for (animation_index, animation) in layer.animations.iter().enumerate() {
                    assert_eq!(
                        layer.find_animation(&animation.name).map(|entry| entry.0),
                        layer
                            .animations
                            .iter()
                            .position(|candidate| candidate.name == animation.name)
                    );
                    assert_eq!(
                        animation.motions.len(),
                        animation.declared_motion_count as usize
                    );
                    empty_motion_slots += animation
                        .motions
                        .iter()
                        .filter(|motion| motion.target < 0)
                        .count();
                    automatic_durations += usize::from(animation.duration < 0);
                    assert!(animation.runtime_duration() >= 0.0);

                    let mut transforms = layer
                        .transforms
                        .iter()
                        .copied()
                        .map(|raw| raw.spatial())
                        .collect::<Vec<_>>();
                    applied_common_channels += animation
                        .apply_common_channels(&mut transforms, 0.0)
                        .unwrap_or_else(|error| {
                            panic!("{} animation {animation_index}: {error}", path.display())
                        });
                    animation_count += 1;
                }
            }
        }
    }

    assert!(animation_count > 0);
    assert!(empty_motion_slots > 0);
    assert!(automatic_durations > 0);
    assert!(applied_common_channels > 0);
    eprintln!(
        "animations={animation_count}, empty MOT slots={empty_motion_slots}, automatic durations={automatic_durations}, applied common channels={applied_common_channels}"
    );
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
    let mut number_glyph_count = 0usize;
    let mut drawable_number_glyph_count = 0usize;
    let mut resolved_number_glyph_textures = 0usize;
    let mut animated_image_reference_evaluations = 0usize;
    let mut resolved_animated_image_references = 0usize;
    let mut animated_vertex_color_evaluations = 0usize;
    let mut animated_image_size_evaluations = 0usize;
    let mut reference_count = 0usize;
    let mut animated_reference_frame_evaluations = 0usize;
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
                let formatted = definition.initial_formatted_text();
                assert!(
                    formatted
                        .combined
                        .iter()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'+' | b'-' | b',' | b'.'))
                );
                let glyphs = definition.build_glyph_records(&formatted, true);
                assert_eq!(
                    glyphs.len(),
                    definition.build_glyph_records(&formatted, false).len()
                );
                assert!(glyphs.len() <= formatted.combined.len());
                number_glyph_count += glyphs.len();
                for glyph in glyphs {
                    assert!(i32::from(glyph.glyph_index) < i32::from(definition.cref_count));
                    if !glyph.drawable() {
                        continue;
                    }
                    drawable_number_glyph_count += 1;
                    let reference = definition.crefs[glyph.glyph_index as usize];
                    if reference.image_index < 0 || reference.rectangle_index < 0 {
                        continue;
                    }
                    base.resolve_coordinates(
                        ImageReferenceChannel::Cref,
                        definition
                            .glyph_coordinate_state(glyph.glyph_index, ImageReferenceChannel::Cref),
                        textures.as_ref().expect("CNUM glyph requires TEXL"),
                        NumberDefinition::INITIAL_COORDINATE_OFFSET_SCALE,
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} CNUM NODE {node_index} glyph {}: {error}",
                            path.display(),
                            glyph.glyph_index
                        )
                    });
                    resolved_number_glyph_textures += 1;
                }
            }
            for (node_index, definition) in layer.reference_by_node.iter().enumerate() {
                let Some(definition) = definition else {
                    continue;
                };
                assert_eq!(layer.nodes[node_index].cast_type(), Some(3));
                assert!(definition.source_name.len() <= 512);
                assert!(definition.layer_name.len() <= 512);
                assert!(definition.animation_name.len() <= 512);
                reference_count += 1;
            }
            for animation in block.children.iter().filter(|child| child.is_tag(b"ANIM")) {
                for motion_block in animation
                    .children
                    .iter()
                    .filter(|child| child.is_tag(b"MOT "))
                {
                    let motion = Motion::from_block(&file, motion_block).unwrap();
                    let Ok(node_index) = usize::try_from(motion.target) else {
                        continue;
                    };
                    for track in motion.tracks.iter().filter(|track| track.target == 23) {
                        let definition = layer
                            .reference_by_node
                            .get(node_index)
                            .and_then(|definition| definition.as_ref())
                            .unwrap_or_else(|| {
                                panic!(
                                    "{} channel 23 NODE {node_index} has no CRFD",
                                    path.display()
                                )
                            });
                        let KeyData::Key20F32(keys) = &track.keys else {
                            panic!("reference frame track has non-f32 KEY data");
                        };
                        for (index, key) in keys.iter().enumerate() {
                            let request = definition
                                .animation_request(track, key.frame as f32)
                                .expect("enabled reference track produced no request");
                            assert_eq!(request.source_name, definition.source_name);
                            assert_eq!(request.layer_name, definition.layer_name);
                            assert_eq!(request.animation_name, definition.animation_name);
                            animated_reference_frame_evaluations += 1;
                            if let Some(next) = keys.get(index + 1) {
                                let request = definition
                                    .animation_request(
                                        track,
                                        (key.frame as f32 + next.frame as f32) * 0.5,
                                    )
                                    .expect("enabled reference midpoint produced no request");
                                assert_eq!(request.source_name, definition.source_name);
                                assert_eq!(request.layer_name, definition.layer_name);
                                assert_eq!(request.animation_name, definition.animation_name);
                                animated_reference_frame_evaluations += 1;
                            }
                        }
                    }
                    let base = layer
                        .image_by_node
                        .get(node_index)
                        .cloned()
                        .flatten()
                        .or_else(|| {
                            layer
                                .number_by_node
                                .get(node_index)
                                .and_then(|definition| definition.as_ref())
                                .map(NumberDefinition::image_base)
                        });
                    let Some(base) = base else {
                        continue;
                    };
                    let mut geometry = base.initial_geometry_state();
                    for track in motion
                        .tracks
                        .iter()
                        .filter(|track| matches!(track.target, 11 | 12))
                    {
                        let KeyData::Key20F32(keys) = &track.keys else {
                            panic!("image size track has non-f32 KEY data");
                        };
                        for (index, key) in keys.iter().enumerate() {
                            assert!(base.apply_size_track(&mut geometry, track, key.frame as f32));
                            animated_image_size_evaluations += 1;
                            if let Some(next) = keys.get(index + 1) {
                                assert!(base.apply_size_track(
                                    &mut geometry,
                                    track,
                                    (key.frame as f32 + next.frame as f32) * 0.5,
                                ));
                                animated_image_size_evaluations += 1;
                            }
                        }
                    }
                    for track in motion
                        .tracks
                        .iter()
                        .filter(|track| matches!(track.target, 13..=16))
                    {
                        let KeyData::Key8Bytes4(keys) = &track.keys else {
                            panic!("vertex color track has non-byte4 KEY data");
                        };
                        let mut state = base.initial_coordinate_state(ImageReferenceChannel::Cref);
                        for (index, key) in keys.iter().enumerate() {
                            assert!(base.apply_vertex_color_track(
                                &mut state,
                                track,
                                key.frame as f32
                            ));
                            animated_vertex_color_evaluations += 1;
                            if let Some(next) = keys.get(index + 1) {
                                assert!(base.apply_vertex_color_track(
                                    &mut state,
                                    track,
                                    (key.frame as f32 + next.frame as f32) * 0.5,
                                ));
                                animated_vertex_color_evaluations += 1;
                            }
                        }
                    }
                    for track in motion
                        .tracks
                        .iter()
                        .filter(|track| matches!(track.target, 17 | 20) && track.format & 3 == 3)
                    {
                        let KeyData::Key20I32(keys) = &track.keys else {
                            panic!("coordinate track has non-i32 KEY data");
                        };
                        let channel = if track.target == 17 {
                            ImageReferenceChannel::Cref
                        } else {
                            ImageReferenceChannel::Cre1
                        };
                        let mut state = base.initial_coordinate_state(channel);
                        let mut frames = Vec::with_capacity(keys.len().saturating_mul(2));
                        for (index, key) in keys.iter().enumerate() {
                            frames.push(key.frame as f32);
                            if let Some(next) = keys.get(index + 1) {
                                frames.push((key.frame as f32 + next.frame as f32) * 0.5);
                            }
                        }
                        for frame in frames {
                            if !base
                                .apply_coordinate_track(
                                    channel,
                                    &mut state,
                                    track,
                                    frame,
                                    textures
                                        .as_ref()
                                        .expect("coordinate animation requires TEXL"),
                                )
                                .unwrap_or_else(|error| {
                                    panic!(
                                        "{} NODE {node_index} channel {} frame {frame}: {error}",
                                        path.display(),
                                        track.target
                                    )
                                })
                            {
                                continue;
                            }
                            animated_image_reference_evaluations += 1;
                            let declared_count = match channel {
                                ImageReferenceChannel::Cref => base.cref_count,
                                ImageReferenceChannel::Cre1 => base.cre1_count,
                            };
                            if state.reference_index < 0
                                || u32::from(state.reference_index as u16)
                                    >= u32::from(declared_count)
                                || state.explicit_image_index < 0
                            {
                                continue;
                            }
                            base.resolve_coordinates(
                                channel,
                                state,
                                textures
                                    .as_ref()
                                    .expect("coordinate animation requires TEXL"),
                                ImageDefinition::INITIAL_COORDINATE_OFFSET_SCALE,
                            )
                            .unwrap_or_else(|error| {
                                panic!(
                                    "{} NODE {node_index} channel {} frame {frame}: {error}",
                                    path.display(),
                                    track.target
                                )
                            });
                            resolved_animated_image_references += 1;
                        }
                    }
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
        "CSLI definitions={csli_count}, CREF records={cref_count}, resolved cell CREFs={resolved_cell_crefs}, CIMG definitions={image_count}, CIMG CREF records={image_cref_count}, CIMG CRE1 records={image_cre1_count}, resolved CIMG channels={resolved_image_channels}, text casts={text_cast_count}, CNUM definitions={number_count}, CNUM CREF records={number_cref_count}, CNUM CRE1 records={number_cre1_count}, resolved CNUM channels={resolved_number_channels}, valid CNUM special glyphs={valid_number_special_glyphs}, CNUM glyphs={number_glyph_count}, drawable CNUM glyphs={drawable_number_glyph_count}, resolved CNUM glyph textures={resolved_number_glyph_textures}, animated image reference evaluations={animated_image_reference_evaluations}, resolved animated image references={resolved_animated_image_references}, animated vertex color evaluations={animated_vertex_color_evaluations}, animated image size evaluations={animated_image_size_evaluations}, CRFD definitions={reference_count}, animated reference frame evaluations={animated_reference_frame_evaluations}, TEX records={texture_count}, CROP records={crop_count}, nonnegative cell CREFs={nonnegative_cell_crefs}, resolved cell textures={resolved_cell_textures}, active color cells={active_color_cells}, missing 0x3A={missing_3a}, missing 0x33={missing_33}, non-four 0x44={non_four_44}, indexed children={indexed_children}, active indexed children={active_indexed_children}"
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
    assert!(number_glyph_count > 0);
    assert!(drawable_number_glyph_count > 0);
    assert!(resolved_number_glyph_textures > 0);
    assert!(animated_image_reference_evaluations > 0);
    assert!(resolved_animated_image_references > 0);
    assert!(animated_vertex_color_evaluations > 0);
    assert!(animated_image_size_evaluations > 0);
    assert!(reference_count > 0);
    assert!(animated_reference_frame_evaluations > 0);
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
