use crate::gitlab::{
    parse_system_note, BridgeJob, Discussion, Job, MrActivityFilter, ParsedSystemNote,
};

use super::super::pipeline_graph_model::{build_stage_columns, StageColumn};

#[derive(Debug, Clone)]
pub enum ActivityEntry {
    System {
        discussion_ix: usize,
        note_ix: usize,
        parsed: Box<ParsedSystemNote>,
    },
    Thread {
        discussion_ix: usize,
    },
}

fn stage_aggregate_status(col: &StageColumn) -> &'static str {
    let (mut has_manual, mut has_skipped) = (false, false);
    for card in &col.cards {
        match card.job.status.as_str() {
            "failed" => return "failed",
            "running" | "preparing" | "canceling" => return "running",
            "pending" | "created" => return "pending",
            "canceled" | "cancelled" => return "canceled",
            "manual" => has_manual = true,
            "skipped" => has_skipped = true,
            _ => {}
        }
    }
    if has_manual {
        "manual"
    } else if has_skipped {
        "skipped"
    } else {
        "success"
    }
}

pub fn compute_stage_statuses(jobs: &[Job], bridges: &[BridgeJob]) -> Vec<&'static str> {
    build_stage_columns(jobs, bridges)
        .iter()
        .map(stage_aggregate_status)
        .collect()
}

pub fn build_activity_entries(
    discussions: &[Discussion],
    filter: MrActivityFilter,
    desc: bool,
) -> Vec<ActivityEntry> {
    let indices: Vec<usize> = if desc {
        (0..discussions.len()).rev().collect()
    } else {
        (0..discussions.len()).collect()
    };
    let mut out = Vec::new();
    for ix in indices {
        let disc = &discussions[ix];
        if disc.notes.is_empty() {
            continue;
        }
        let is_system_only = disc.notes.iter().all(|n| n.system);
        match filter {
            MrActivityFilter::CommentsOnly if is_system_only => continue,
            MrActivityFilter::HistoryOnly if !is_system_only => continue,
            _ => {}
        }
        if is_system_only {
            for (note_ix, note) in disc.notes.iter().enumerate() {
                out.push(ActivityEntry::System {
                    discussion_ix: ix,
                    note_ix,
                    parsed: Box::new(parse_system_note(&note.body)),
                });
            }
        } else {
            out.push(ActivityEntry::Thread { discussion_ix: ix });
        }
    }
    out
}
