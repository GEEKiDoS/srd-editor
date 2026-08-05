use std::fmt;

/// The target renderer walks exactly 32 EntryInfo records when it flushes a
/// scene model module.
pub const SCENE_TARGET_ENTRY_COUNT: usize = 32;

/// Low byte carried by packet `+0x84` on the proven SRD submission path:
/// constructor `0x00`, SrRenderer `| 0x10`, submit `| 0x01`.
pub const SRD_COMMAND_PACKET_84_LOW: u8 = 0x11;

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
