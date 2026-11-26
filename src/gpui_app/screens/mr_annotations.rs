use gpui_kit::{
    component::diff::{DiffLinePosition, DiffSide},
    SharedString,
};

use crate::gitlab::{parse_unified_full, DiffKind, DiffPosition, DiffRefs, Discussion, RawDiff};

pub fn resolve_diff_position(
    raw_diffs: &[RawDiff],
    clicked: &DiffLinePosition,
    refs: &DiffRefs,
) -> Option<DiffPosition> {
    let base_sha = refs.base_sha.clone()?;
    let start_sha = refs.start_sha.clone()?;
    let head_sha = refs.head_sha.clone()?;
    let target_path = clicked.path().as_ref();
    let target_line = clicked.line() as u32;

    let raw = raw_diffs
        .iter()
        .find(|d| d.new_path == target_path || d.old_path == target_path)?;

    let lines = parse_unified_full(&raw.diff);
    let matched = lines.iter().find(|line| match clicked.side() {
        DiffSide::Original => line.old == Some(target_line),
        DiffSide::Modified => line.new == Some(target_line),
    });

    let (old_line, new_line) = match matched {
        Some(line) => match line.kind {
            DiffKind::Context => (line.old, line.new),
            DiffKind::Add => (None, line.new),
            DiffKind::Del => (line.old, None),
            DiffKind::Hunk => return None,
        },
        None => match clicked.side() {
            DiffSide::Original => (Some(target_line), None),
            DiffSide::Modified => (None, Some(target_line)),
        },
    };

    Some(DiffPosition {
        position_type: "text".into(),
        base_sha,
        start_sha,
        head_sha,
        old_path: Some(raw.old_path.clone()),
        new_path: Some(raw.new_path.clone()),
        old_line,
        new_line,
    })
}

pub fn discussion_line_position(
    discussion: &Discussion,
    active_head_sha: Option<&str>,
) -> Option<DiffLinePosition> {
    let note = discussion.notes.first()?;
    let pos = note.position.as_ref()?;
    if pos.position_type != "text" {
        return None;
    }
    if let Some(head) = active_head_sha {
        if !pos.head_sha.is_empty() && pos.head_sha != head {
            return None;
        }
    }
    let path = pos
        .new_path
        .as_deref()
        .or(pos.old_path.as_deref())
        .filter(|p| !p.is_empty())?;

    if let Some(new_line) = pos.new_line {
        return Some(DiffLinePosition::new(
            SharedString::from(path.to_string()),
            DiffSide::Modified,
            new_line as usize,
        ));
    }
    if let Some(old_line) = pos.old_line {
        return Some(DiffLinePosition::new(
            SharedString::from(path.to_string()),
            DiffSide::Original,
            old_line as usize,
        ));
    }
    None
}

pub fn is_outdated_diff_discussion(discussion: &Discussion, active_head_sha: Option<&str>) -> bool {
    let Some(pos) = discussion.notes.first().and_then(|n| n.position.as_ref()) else {
        return false;
    };
    if pos.position_type != "text" {
        return false;
    }
    match active_head_sha {
        Some(head) => !pos.head_sha.is_empty() && pos.head_sha != head,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitlab::DiscussionNote;

    fn sample_diffs() -> Vec<RawDiff> {
        vec![RawDiff {
            old_path: "src/app.rs".into(),
            new_path: "src/app.rs".into(),
            diff: "@@ -10,3 +20,4 @@ fn run\n unchanged\n-removed\n+added\n+extra\n tail\n".into(),
            new_file: false,
            renamed_file: false,
            deleted_file: false,
            too_large: false,
            collapsed: false,
            generated_file: false,
        }]
    }

    fn refs() -> DiffRefs {
        DiffRefs {
            base_sha: Some("base1".into()),
            start_sha: Some("start1".into()),
            head_sha: Some("head1".into()),
        }
    }

    #[test]
    fn context_line_emits_both_old_and_new_line_numbers() {
        let diffs = sample_diffs();
        let clicked = DiffLinePosition::new("src/app.rs", DiffSide::Modified, 20);
        let pos = resolve_diff_position(&diffs, &clicked, &refs()).unwrap();
        assert_eq!(pos.old_line, Some(10));
        assert_eq!(pos.new_line, Some(20));
    }

    #[test]
    fn added_and_deleted_lines_emit_single_side() {
        let diffs = sample_diffs();
        let added = DiffLinePosition::new("src/app.rs", DiffSide::Modified, 21);
        let pos_add = resolve_diff_position(&diffs, &added, &refs()).unwrap();
        assert_eq!(pos_add.old_line, None);
        assert_eq!(pos_add.new_line, Some(21));

        let deleted = DiffLinePosition::new("src/app.rs", DiffSide::Original, 11);
        let pos_del = resolve_diff_position(&diffs, &deleted, &refs()).unwrap();
        assert_eq!(pos_del.old_line, Some(11));
        assert_eq!(pos_del.new_line, None);
    }

    #[test]
    fn outdated_head_sha_flagged_and_not_inline() {
        let disc = Discussion {
            id: "abcd".into(),
            individual_note: false,
            notes: vec![DiscussionNote {
                id: 1,
                body: "nit".into(),
                created_at: None,
                author: None,
                system: false,
                resolvable: true,
                resolved: false,
                position: Some(DiffPosition {
                    position_type: "text".into(),
                    base_sha: "b".into(),
                    start_sha: "s".into(),
                    head_sha: "old_head".into(),
                    old_path: Some("src/app.rs".into()),
                    new_path: Some("src/app.rs".into()),
                    old_line: None,
                    new_line: Some(21),
                }),
            }],
        };
        assert!(is_outdated_diff_discussion(&disc, Some("new_head")));
        assert!(discussion_line_position(&disc, Some("new_head")).is_none());
        assert!(discussion_line_position(&disc, Some("old_head")).is_some());
    }
}
