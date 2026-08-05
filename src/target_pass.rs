use std::fmt;

use crate::render::CeylonDrawPacketPresetState;

/// The target renderer walks exactly 32 EntryInfo records when it flushes a
/// scene model module.
pub const SCENE_TARGET_ENTRY_COUNT: usize = 32;

/// Low byte carried by packet `+0x84` on the proven SRD submission path:
/// constructor `0x00`, SrRenderer `| 0x10`, submit `| 0x01`.
pub const SRD_COMMAND_PACKET_84_LOW: u8 = 0x11;

/// `sea::BasePass` property values used to build one active scene-pass rule.
/// `pass_index` keeps the binary property's original name: it indexes the
/// target's 32-entry `EntryInfo` table, while classification uses the compact
/// active-rule index produced after disabled entries are skipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvidenceBasePassProfile {
    pub name: &'static str,
    pub pass_index: u32,
    pub entry: bool,
    pub rule: EvidenceScenePassRule,
}

/// Five `sea::PassBasic` instances installed by the common `air::Scene`
/// constructor at `sub_6CF9A0`. Chusan's MainScene and BgScene both use this
/// constructor and do not replace these values in their concrete setup path.
pub const EVIDENCE_AIR_SCENE_BASE_PASSES: [EvidenceBasePassProfile; 5] = [
    EvidenceBasePassProfile {
        name: "Back2DPass",
        pass_index: 4,
        entry: true,
        rule: EvidenceScenePassRule {
            class_selector: 3,
            attribute_group: 0,
            condition_mode: 4,
            depth_store_selector: 5,
            depth_threshold: 0.0,
            order_threshold: 8_388_608,
        },
    },
    EvidenceBasePassProfile {
        name: "OpaquePass",
        pass_index: 8,
        entry: true,
        rule: EvidenceScenePassRule {
            class_selector: 0,
            attribute_group: 0,
            condition_mode: 0,
            depth_store_selector: 3,
            depth_threshold: 0.0,
            order_threshold: 0,
        },
    },
    EvidenceBasePassProfile {
        name: "PunchPass",
        pass_index: 12,
        entry: true,
        rule: EvidenceScenePassRule {
            class_selector: 1,
            attribute_group: 0,
            condition_mode: 0,
            depth_store_selector: 3,
            depth_threshold: 0.0,
            order_threshold: 0,
        },
    },
    EvidenceBasePassProfile {
        // The spelling is copied exactly from the binary string table.
        name: "TrancePass",
        pass_index: 16,
        entry: true,
        rule: EvidenceScenePassRule {
            class_selector: 2,
            attribute_group: 0,
            condition_mode: 0,
            depth_store_selector: 6,
            depth_threshold: 0.0,
            order_threshold: 0,
        },
    },
    EvidenceBasePassProfile {
        name: "Front2DPass",
        pass_index: 24,
        entry: true,
        rule: EvidenceScenePassRule {
            class_selector: 3,
            attribute_group: 0,
            condition_mode: 3,
            depth_store_selector: 5,
            depth_threshold: 0.0,
            order_threshold: 8_388_608,
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvidenceScenePassRule {
    pub class_selector: u32,
    pub attribute_group: u32,
    pub condition_mode: u32,
    pub depth_store_selector: i32,
    pub depth_threshold: f32,
    pub order_threshold: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvidenceScenePassClassificationInput {
    pub command_class: u32,
    pub attribute_group: u32,
    pub depth: f32,
    pub order: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceSelectedScenePass {
    pub pass_index: usize,
    pub stores_depth: bool,
}

/// Reproduces the type-1 command classifier at `sub_63E760`.
pub const fn classify_evidence_type1_command(flags_60: u32, packet_84_low: u8) -> u32 {
    let mut class = if flags_60 & 0x20 != 0 {
        2
    } else if flags_60 & 0x40 != 0 {
        1
    } else {
        0
    };
    if flags_60 & 0x80 != 0 || packet_84_low & 0x10 != 0 {
        class = 3;
    }
    if flags_60 & 0x2000 != 0 {
        class = 4;
    }
    class
}

pub const fn classify_evidence_srd_type1_command(flags_60: u32) -> u32 {
    classify_evidence_type1_command(flags_60, SRD_COMMAND_PACKET_84_LOW)
}

pub const fn evidence_type1_attribute_group(packet_64: u32) -> u32 {
    (packet_64 >> 25) & 0x0f
}

/// Reproduces `sub_64BAB0`'s forward, first-match rule scan.
pub fn select_evidence_scene_pass(
    rules: &[EvidenceScenePassRule],
    input: EvidenceScenePassClassificationInput,
) -> Option<EvidenceSelectedScenePass> {
    if input.command_class >= 8 {
        return None;
    }

    rules.iter().enumerate().find_map(|(pass_index, rule)| {
        if rule.attribute_group != input.attribute_group
            || !class_selector_matches(rule.class_selector, input.command_class)
            || !condition_matches(*rule, input)
        {
            return None;
        }
        Some(EvidenceSelectedScenePass {
            pass_index,
            stores_depth: rule.depth_store_selector < 8,
        })
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceScenePassInvariantError(pub String);

impl fmt::Display for EvidenceScenePassInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EvidenceScenePassInvariantError {}

/// Selects a pass only when the forward rule scan has a provably identical
/// result for every possible command depth and every `u16` order value.
///
/// This is intentionally conservative. A depth-dependent condition, or an
/// order condition that divides the `u16` domain, stops the proof instead of
/// substituting an editor-owned value. Rules that are impossible over the
/// complete domain are skipped; the first universally true rule is selected.
pub fn select_evidence_scene_pass_without_depth_or_order(
    rules: &[EvidenceScenePassRule],
    command_class: u32,
    attribute_group: u32,
) -> Result<Option<EvidenceSelectedScenePass>, EvidenceScenePassInvariantError> {
    if command_class >= 8 {
        return Ok(None);
    }

    for (pass_index, rule) in rules.iter().copied().enumerate() {
        if rule.attribute_group != attribute_group
            || !class_selector_matches(rule.class_selector, command_class)
        {
            continue;
        }

        match condition_domain(rule) {
            EvidenceConditionDomain::Never => continue,
            EvidenceConditionDomain::Always => {
                return Ok(Some(EvidenceSelectedScenePass {
                    pass_index,
                    stores_depth: rule.depth_store_selector < 8,
                }));
            }
            EvidenceConditionDomain::InputDependent => {
                return Err(EvidenceScenePassInvariantError(format!(
                    "matching rule {pass_index} depends on unprovided depth/order input"
                )));
            }
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceConditionDomain {
    Never,
    Always,
    InputDependent,
}

fn condition_domain(rule: EvidenceScenePassRule) -> EvidenceConditionDomain {
    match rule.condition_mode {
        0 => EvidenceConditionDomain::Always,
        // A NaN threshold makes either comparison false for every f32 input.
        // All other depth thresholds retain at least one input-dependent edge,
        // including infinities and NaN command depths.
        1 | 2 if rule.depth_threshold.is_nan() => EvidenceConditionDomain::Never,
        1 | 2 => EvidenceConditionDomain::InputDependent,
        3 if rule.order_threshold == 0 => EvidenceConditionDomain::Always,
        3 if rule.order_threshold > u32::from(u16::MAX) => EvidenceConditionDomain::Never,
        3 => EvidenceConditionDomain::InputDependent,
        4 if rule.order_threshold == 0 => EvidenceConditionDomain::Never,
        4 if rule.order_threshold > u32::from(u16::MAX) => EvidenceConditionDomain::Always,
        4 => EvidenceConditionDomain::InputDependent,
        _ => EvidenceConditionDomain::Never,
    }
}

const fn class_selector_matches(selector: u32, command_class: u32) -> bool {
    selector == command_class
        || selector == 6
        || selector == 7 && command_class < 2
        || selector == 5 && command_class <= 2
}

fn condition_matches(
    rule: EvidenceScenePassRule,
    input: EvidenceScenePassClassificationInput,
) -> bool {
    match rule.condition_mode {
        0 => true,
        1 => rule.depth_threshold > input.depth,
        2 => input.depth >= rule.depth_threshold,
        3 => u32::from(input.order) >= rule.order_threshold,
        4 => u32::from(input.order) < rule.order_threshold,
        _ => false,
    }
}

/// Inclusive SceneModelModule pass-index range stored by one target EntryInfo.
/// The binary initializes unused entries to `(-1, -1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceScenePassRange {
    pub first: i32,
    pub last: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceScenePassProfile {
    pub rules: Vec<EvidenceScenePassRule>,
    pub target_entries: [EvidenceScenePassRange; SCENE_TARGET_ENTRY_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceScenePassProfileError(pub String);

impl fmt::Display for EvidenceScenePassProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EvidenceScenePassProfileError {}

/// Reproduces the `sub_64BDA0 -> sub_64F970` profile build:
///
/// - disabled BasePass entries produce neither a rule nor an EntryInfo update;
/// - enabled rules receive compact indices in registration order;
/// - the BasePass `PassIndex` property selects one of 32 EntryInfo slots;
/// - the slot keeps the first compact rule index and always updates the last.
pub fn build_evidence_scene_pass_profile(
    base_passes: &[EvidenceBasePassProfile],
) -> Result<EvidenceScenePassProfile, EvidenceScenePassProfileError> {
    let mut rules = Vec::new();
    let mut target_entries = [EvidenceScenePassRange::DISABLED; SCENE_TARGET_ENTRY_COUNT];

    for base_pass in base_passes {
        if !base_pass.entry {
            continue;
        }
        let compact_rule_index = i32::try_from(rules.len()).map_err(|_| {
            EvidenceScenePassProfileError("active BasePass rule count exceeds i32".to_string())
        })?;
        let entry_index = usize::try_from(base_pass.pass_index).map_err(|_| {
            EvidenceScenePassProfileError(format!(
                "BasePass {:?} PassIndex {} does not fit usize",
                base_pass.name, base_pass.pass_index
            ))
        })?;
        let Some(entry) = target_entries.get_mut(entry_index) else {
            return Err(EvidenceScenePassProfileError(format!(
                "BasePass {:?} PassIndex {} is outside the binary's 32 EntryInfo slots",
                base_pass.name, base_pass.pass_index
            )));
        };

        rules.push(base_pass.rule);
        if entry.first == -1 {
            entry.first = compact_rule_index;
        }
        entry.last = compact_rule_index;
    }

    Ok(EvidenceScenePassProfile {
        rules,
        target_entries,
    })
}

impl EvidenceScenePassRange {
    pub const DISABLED: Self = Self {
        first: -1,
        last: -1,
    };

    pub const fn inclusive(first: i32, last: i32) -> Self {
        Self { first, last }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceScenePassOrderError(pub String);

impl fmt::Display for EvidenceScenePassOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EvidenceScenePassOrderError {}

/// Reproduces the proven SceneModelModule queue/flush ordering after every
/// command has already been classified to a pass-rule index.
///
/// `classified_pass_indices` is in target classification order. Each item is
/// appended to its pass vector, retaining that order within the pass. The 32
/// target EntryInfo ranges are then visited in array order; every enabled
/// inclusive range visits pass indices in ascending order. Repeated or
/// overlapping ranges therefore repeat the same queued item indices.
///
/// This deliberately does not classify packets or invent a target profile.
pub fn build_evidence_scene_model_submission_indices(
    classified_pass_indices: &[usize],
    pass_count: usize,
    target_entries: &[EvidenceScenePassRange; SCENE_TARGET_ENTRY_COUNT],
) -> Result<Vec<usize>, EvidenceScenePassOrderError> {
    let mut pass_items = vec![Vec::new(); pass_count];
    for (item_index, &pass_index) in classified_pass_indices.iter().enumerate() {
        let Some(pass) = pass_items.get_mut(pass_index) else {
            return Err(EvidenceScenePassOrderError(format!(
                "classified item {item_index} uses pass {pass_index}, but the SceneModelModule has only {pass_count} passes"
            )));
        };
        pass.push(item_index);
    }

    let mut submission = Vec::new();
    for entry in target_entries {
        if entry.first < 0 || entry.first > entry.last {
            continue;
        }

        let first = entry.first as usize;
        if first >= pass_count {
            continue;
        }
        let last = (entry.last as usize).min(pass_count.saturating_sub(1));
        for pass_index in first..=last {
            submission.extend_from_slice(&pass_items[pass_index]);
        }
    }
    Ok(submission)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSrdSceneSubmissionError(pub String);

impl fmt::Display for EvidenceSrdSceneSubmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EvidenceSrdSceneSubmissionError {}

/// Classifies already target-admitted normal SRD/Fennel commands and applies
/// the target's exact SceneModelModule flush order without inventing depth or
/// order values. Each packet must reach a pass invariant over those two inputs.
///
/// Target-filter activation and adjacent packet merging happen outside this
/// helper; the returned indices preserve the logical command order on either
/// side of a merge but do not claim a final GPU packet count.
pub fn build_evidence_srd_scene_submission_indices(
    packets: &[CeylonDrawPacketPresetState],
    profile: &EvidenceScenePassProfile,
) -> Result<Vec<usize>, EvidenceSrdSceneSubmissionError> {
    let mut classified_pass_indices = Vec::with_capacity(packets.len());
    for (command_index, packet) in packets.iter().copied().enumerate() {
        let command_class = classify_evidence_srd_type1_command(packet.flags_60);
        let attribute_group = evidence_type1_attribute_group(packet.flags_64);
        let selected = select_evidence_scene_pass_without_depth_or_order(
            &profile.rules,
            command_class,
            attribute_group,
        )
        .map_err(|error| {
            EvidenceSrdSceneSubmissionError(format!(
                "SRD command {command_index} cannot be classified without guessing: {error}"
            ))
        })?
        .ok_or_else(|| {
            EvidenceSrdSceneSubmissionError(format!(
                "SRD command {command_index} with class {command_class} and attribute group {attribute_group} matches no scene pass"
            ))
        })?;
        classified_pass_indices.push(selected.pass_index);
    }

    build_evidence_scene_model_submission_indices(
        &classified_pass_indices,
        profile.rules.len(),
        &profile.target_entries,
    )
    .map_err(|error| EvidenceSrdSceneSubmissionError(error.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type1_classifier_preserves_binary_override_order() {
        assert_eq!(classify_evidence_type1_command(0, 0), 0);
        assert_eq!(classify_evidence_type1_command(0x40, 0), 1);
        assert_eq!(classify_evidence_type1_command(0x20 | 0x40, 0), 2);
        assert_eq!(classify_evidence_type1_command(0x20, 0x10), 3);
        assert_eq!(classify_evidence_type1_command(0x80 | 0x2000, 0), 4);
        assert_eq!(classify_evidence_srd_type1_command(0x4000), 3);
        assert_eq!(classify_evidence_srd_type1_command(0x6000), 4);
    }

    #[test]
    fn packet_64_attribute_group_uses_only_bits_25_through_28() {
        assert_eq!(evidence_type1_attribute_group(0), 0);
        assert_eq!(evidence_type1_attribute_group(0x1e00_0000), 0x0f);
        assert_eq!(evidence_type1_attribute_group(0xe1ff_ffff), 0);
    }

    #[test]
    fn rule_scan_uses_first_matching_selector_and_attribute_group() {
        let rules = [
            EvidenceScenePassRule {
                class_selector: 6,
                attribute_group: 1,
                condition_mode: 0,
                depth_store_selector: 0,
                depth_threshold: 0.0,
                order_threshold: 0,
            },
            EvidenceScenePassRule {
                class_selector: 6,
                attribute_group: 0,
                condition_mode: 0,
                depth_store_selector: 8,
                depth_threshold: 0.0,
                order_threshold: 0,
            },
            EvidenceScenePassRule {
                class_selector: 3,
                attribute_group: 0,
                condition_mode: 0,
                depth_store_selector: 0,
                depth_threshold: 0.0,
                order_threshold: 0,
            },
        ];

        assert_eq!(
            select_evidence_scene_pass(
                &rules,
                EvidenceScenePassClassificationInput {
                    command_class: 3,
                    attribute_group: 0,
                    depth: 0.0,
                    order: 0,
                }
            ),
            Some(EvidenceSelectedScenePass {
                pass_index: 1,
                stores_depth: false,
            })
        );
    }

    #[test]
    fn rule_class_selectors_five_and_seven_keep_their_exact_ranges() {
        let rule = |class_selector| EvidenceScenePassRule {
            class_selector,
            attribute_group: 0,
            condition_mode: 0,
            depth_store_selector: 0,
            depth_threshold: 0.0,
            order_threshold: 0,
        };
        let input = |command_class| EvidenceScenePassClassificationInput {
            command_class,
            attribute_group: 0,
            depth: 0.0,
            order: 0,
        };

        assert!(select_evidence_scene_pass(&[rule(7)], input(1)).is_some());
        assert!(select_evidence_scene_pass(&[rule(7)], input(2)).is_none());
        assert!(select_evidence_scene_pass(&[rule(5)], input(2)).is_some());
        assert!(select_evidence_scene_pass(&[rule(5)], input(3)).is_none());
    }

    #[test]
    fn rule_condition_modes_match_depth_order_and_nan_edges() {
        let input = EvidenceScenePassClassificationInput {
            command_class: 3,
            attribute_group: 0,
            depth: 5.0,
            order: 10,
        };
        let rule = |condition_mode, depth_threshold, order_threshold| EvidenceScenePassRule {
            class_selector: 3,
            attribute_group: 0,
            condition_mode,
            depth_store_selector: 0,
            depth_threshold,
            order_threshold,
        };

        assert!(select_evidence_scene_pass(&[rule(1, 6.0, 0)], input).is_some());
        assert!(select_evidence_scene_pass(&[rule(1, 5.0, 0)], input).is_none());
        assert!(select_evidence_scene_pass(&[rule(2, 5.0, 0)], input).is_some());
        assert!(select_evidence_scene_pass(&[rule(2, 6.0, 0)], input).is_none());
        assert!(select_evidence_scene_pass(&[rule(3, 0.0, 10)], input).is_some());
        assert!(select_evidence_scene_pass(&[rule(3, 0.0, 11)], input).is_none());
        assert!(select_evidence_scene_pass(&[rule(4, 0.0, 11)], input).is_some());
        assert!(select_evidence_scene_pass(&[rule(4, 0.0, 10)], input).is_none());
        assert!(select_evidence_scene_pass(&[rule(1, f32::NAN, 0)], input).is_none());
        assert!(
            select_evidence_scene_pass(
                &[rule(2, 0.0, 0)],
                EvidenceScenePassClassificationInput {
                    depth: f32::NAN,
                    ..input
                }
            )
            .is_none()
        );
    }

    #[test]
    fn air_scene_default_base_pass_table_matches_the_binary_records() {
        assert_eq!(
            EVIDENCE_AIR_SCENE_BASE_PASSES
                .iter()
                .map(|pass| (pass.name, pass.pass_index))
                .collect::<Vec<_>>(),
            vec![
                ("Back2DPass", 4),
                ("OpaquePass", 8),
                ("PunchPass", 12),
                ("TrancePass", 16),
                ("Front2DPass", 24),
            ]
        );
        assert!(
            EVIDENCE_AIR_SCENE_BASE_PASSES
                .iter()
                .all(|pass| pass.entry && pass.rule.attribute_group == 0)
        );

        let back = EVIDENCE_AIR_SCENE_BASE_PASSES[0].rule;
        assert_eq!((back.class_selector, back.condition_mode), (3, 4));
        assert_eq!(back.depth_store_selector, 5);
        assert_eq!(back.order_threshold, 8_388_608);

        let front = EVIDENCE_AIR_SCENE_BASE_PASSES[4].rule;
        assert_eq!((front.class_selector, front.condition_mode), (3, 3));
        assert_eq!(front.depth_store_selector, 5);
        assert_eq!(front.order_threshold, 8_388_608);
    }

    #[test]
    fn air_scene_profile_uses_pass_index_as_entry_slot_not_rule_index() {
        let profile = build_evidence_scene_pass_profile(&EVIDENCE_AIR_SCENE_BASE_PASSES)
            .expect("binary PassIndex values are within 0..31");

        assert_eq!(profile.rules.len(), 5);
        for (entry_index, compact_rule_index) in [(4, 0), (8, 1), (12, 2), (16, 3), (24, 4)] {
            assert_eq!(
                profile.target_entries[entry_index],
                EvidenceScenePassRange::inclusive(compact_rule_index, compact_rule_index)
            );
        }
        for (entry_index, entry) in profile.target_entries.iter().enumerate() {
            if ![4, 8, 12, 16, 24].contains(&entry_index) {
                assert_eq!(*entry, EvidenceScenePassRange::DISABLED);
            }
        }
    }

    #[test]
    fn profile_builder_skips_disabled_entries_and_extends_duplicate_slots() {
        let mut passes = EVIDENCE_AIR_SCENE_BASE_PASSES;
        passes[1].entry = false;
        passes[2].pass_index = 4;

        let profile = build_evidence_scene_pass_profile(&passes).unwrap();
        assert_eq!(profile.rules.len(), 4);
        assert_eq!(
            profile.target_entries[4],
            EvidenceScenePassRange::inclusive(0, 1)
        );
        assert_eq!(profile.target_entries[8], EvidenceScenePassRange::DISABLED);
        assert_eq!(
            profile.target_entries[16],
            EvidenceScenePassRange::inclusive(2, 2)
        );
        assert_eq!(
            profile.target_entries[24],
            EvidenceScenePassRange::inclusive(3, 3)
        );
    }

    #[test]
    fn profile_builder_rejects_pass_index_outside_the_proven_property_range() {
        let invalid = EvidenceBasePassProfile {
            pass_index: SCENE_TARGET_ENTRY_COUNT as u32,
            ..EVIDENCE_AIR_SCENE_BASE_PASSES[0]
        };
        let error = build_evidence_scene_pass_profile(&[invalid]).unwrap_err();
        assert!(error.0.contains("outside the binary's 32 EntryInfo slots"));
    }

    #[test]
    fn default_srd_class_three_routes_to_back_2d_first() {
        let profile = build_evidence_scene_pass_profile(&EVIDENCE_AIR_SCENE_BASE_PASSES).unwrap();
        for order in [0, u16::MAX] {
            assert_eq!(
                select_evidence_scene_pass(
                    &profile.rules,
                    EvidenceScenePassClassificationInput {
                        command_class: 3,
                        attribute_group: 0,
                        depth: 0.0,
                        order,
                    }
                ),
                Some(EvidenceSelectedScenePass {
                    pass_index: 0,
                    stores_depth: true,
                })
            );
        }
    }

    #[test]
    fn default_air_scene_selection_is_proven_without_depth_or_order_inputs() {
        let profile = build_evidence_scene_pass_profile(&EVIDENCE_AIR_SCENE_BASE_PASSES).unwrap();
        assert_eq!(
            select_evidence_scene_pass_without_depth_or_order(&profile.rules, 3, 0),
            Ok(Some(EvidenceSelectedScenePass {
                pass_index: 0,
                stores_depth: true,
            }))
        );
    }

    #[test]
    fn invariant_selector_rejects_a_rule_that_splits_the_order_domain() {
        let rule = EvidenceScenePassRule {
            class_selector: 3,
            attribute_group: 0,
            condition_mode: 4,
            depth_store_selector: 5,
            depth_threshold: 0.0,
            order_threshold: 100,
        };
        let error = select_evidence_scene_pass_without_depth_or_order(&[rule], 3, 0).unwrap_err();
        assert!(error.0.contains("depends on unprovided depth/order input"));
    }

    #[test]
    fn normal_image_and_fennel_packets_share_the_exact_default_air_pass() {
        let profile = build_evidence_scene_pass_profile(&EVIDENCE_AIR_SCENE_BASE_PASSES).unwrap();
        let mut image = CeylonDrawPacketPresetState::srd_renderer_initial();
        image.set_render_preset_id(4);
        image.set_srd_quad_is_2d(false);
        let fennel = CeylonDrawPacketPresetState {
            draw_flags_00: 0x02af_e003,
            flags_58: 0xff,
            flags_60: 0x40a0,
            ..CeylonDrawPacketPresetState::default()
        };

        assert_eq!(image.flags_64, 0);
        assert_eq!(fennel.flags_64, 0);
        assert_eq!(
            build_evidence_srd_scene_submission_indices(&[image, fennel], &profile).unwrap(),
            vec![0, 1]
        );
    }

    #[test]
    fn class_four_packet_is_not_forced_into_an_unproven_default_pass() {
        let profile = build_evidence_scene_pass_profile(&EVIDENCE_AIR_SCENE_BASE_PASSES).unwrap();
        let packet = CeylonDrawPacketPresetState {
            flags_60: 0x6000,
            ..CeylonDrawPacketPresetState::srd_renderer_initial()
        };
        let error = build_evidence_srd_scene_submission_indices(&[packet], &profile).unwrap_err();
        assert!(error.0.contains("matches no scene pass"));
    }

    #[test]
    fn scene_model_passes_keep_stable_insertion_and_ascending_range_order() {
        let mut entries = [EvidenceScenePassRange::DISABLED; SCENE_TARGET_ENTRY_COUNT];
        entries[0] = EvidenceScenePassRange::inclusive(1, 2);
        entries[1] = EvidenceScenePassRange::inclusive(0, 0);

        let order = build_evidence_scene_model_submission_indices(&[2, 0, 2, 1], 3, &entries)
            .expect("valid classified passes");

        assert_eq!(order, vec![3, 0, 2, 1]);
    }

    #[test]
    fn overlapping_target_ranges_repeat_the_same_pass_records() {
        let mut entries = [EvidenceScenePassRange::DISABLED; SCENE_TARGET_ENTRY_COUNT];
        entries[0] = EvidenceScenePassRange::inclusive(2, 2);
        entries[1] = EvidenceScenePassRange::inclusive(1, 2);

        let order = build_evidence_scene_model_submission_indices(&[2, 1, 2], 3, &entries)
            .expect("valid classified passes");

        assert_eq!(order, vec![0, 2, 1, 0, 2]);
    }

    #[test]
    fn disabled_reversed_and_out_of_range_entry_passes_are_no_ops() {
        let mut entries = [EvidenceScenePassRange::DISABLED; SCENE_TARGET_ENTRY_COUNT];
        entries[0] = EvidenceScenePassRange::inclusive(2, 1);
        entries[1] = EvidenceScenePassRange::inclusive(8, 12);
        entries[2] = EvidenceScenePassRange::inclusive(1, 12);

        let order = build_evidence_scene_model_submission_indices(&[0, 1, 2], 3, &entries)
            .expect("valid classified passes");

        assert_eq!(order, vec![1, 2]);
    }

    #[test]
    fn invalid_classification_is_rejected_instead_of_being_guessed() {
        let entries = [EvidenceScenePassRange::DISABLED; SCENE_TARGET_ENTRY_COUNT];
        let error = build_evidence_scene_model_submission_indices(&[0, 3], 3, &entries)
            .expect_err("pass index equal to pass count must fail");

        assert!(error.0.contains("classified item 1 uses pass 3"));
    }
}
