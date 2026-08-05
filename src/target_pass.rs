use std::fmt;

/// The target renderer walks exactly 32 EntryInfo records when it flushes a
/// scene model module.
pub const SCENE_TARGET_ENTRY_COUNT: usize = 32;

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
