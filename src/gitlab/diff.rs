use serde::{Deserialize, Deserializer};

fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Add,
    Del,
    Context,
    Hunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct MrChange {
    pub path: String,
    pub old_path: String,
    pub added: u32,
    pub removed: u32,
    pub new_file: bool,
    pub deleted_file: bool,
    pub renamed_file: bool,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct MrDiff {
    pub files: Vec<MrChange>,
    pub overflow: bool,
}

#[derive(Deserialize)]
pub struct ChangesPayload {
    #[serde(default, deserialize_with = "null_to_default")]
    pub overflow: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub changes: Vec<RawDiff>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawDiff {
    #[serde(default, deserialize_with = "null_to_default")]
    pub old_path: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub new_path: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub diff: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub new_file: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub renamed_file: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub deleted_file: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub too_large: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub collapsed: bool,
    #[serde(default, deserialize_with = "null_to_default")]
    pub generated_file: bool,
}

impl MrDiff {
    pub fn from_payload(payload: ChangesPayload) -> Self {
        Self {
            overflow: payload.overflow,
            files: payload.changes.into_iter().map(MrChange::from_raw).collect(),
        }
    }
}

impl MrChange {
    fn from_raw(raw: RawDiff) -> Self {
        let lines = parse_unified(&raw.diff);
        let added = lines.iter().filter(|line| line.kind == DiffKind::Add).count() as u32;
        let removed = lines.iter().filter(|line| line.kind == DiffKind::Del).count() as u32;
        Self {
            path: raw.new_path,
            old_path: raw.old_path,
            added,
            removed,
            new_file: raw.new_file,
            deleted_file: raw.deleted_file,
            renamed_file: raw.renamed_file,
            lines,
        }
    }
}

pub fn parse_unified(diff: &str) -> Vec<DiffLine> {
    parse_unified_full(diff)
}

pub fn parse_unified_full(diff: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old = 0u32;
    let mut new = 0u32;
    for raw in diff.split('\n') {
        if raw.starts_with("@@") {
            (old, new) = hunk_start(raw);
            lines.push(DiffLine {
                kind: DiffKind::Hunk,
                old: None,
                new: None,
                text: raw.to_string(),
            });
            continue;
        }
        if raw.starts_with("diff ")
            || raw.starts_with("index ")
            || raw.starts_with("---")
            || raw.starts_with("+++")
            || raw.starts_with('\\')
        {
            continue;
        }
        let (kind, text) = match raw.as_bytes().first() {
            Some(b'+') => (DiffKind::Add, &raw[1..]),
            Some(b'-') => (DiffKind::Del, &raw[1..]),
            Some(b' ') => (DiffKind::Context, &raw[1..]),
            _ if raw.is_empty() => continue,
            _ => (DiffKind::Context, raw),
        };
        let (old_no, new_no) = match kind {
            DiffKind::Add => {
                new += 1;
                (None, Some(new))
            }
            DiffKind::Del => {
                old += 1;
                (Some(old), None)
            }
            DiffKind::Context => {
                old += 1;
                new += 1;
                (Some(old), Some(new))
            }
            DiffKind::Hunk => (None, None),
        };
        lines.push(DiffLine {
            kind,
            old: old_no,
            new: new_no,
            text: text.to_string(),
        });
    }
    lines
}

fn hunk_start(header: &str) -> (u32, u32) {
    let mut old = 1u32;
    let mut new = 1u32;
    let bytes = header.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'-' || bytes[i] == b'+' {
            let sign = bytes[i];
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(n) = header[start..i].parse::<u32>() {
                if sign == b'-' {
                    old = n.saturating_sub(1);
                } else {
                    new = n.saturating_sub(1);
                }
            }
        } else {
            i += 1;
        }
    }
    (old, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_keeps_gitlab_line_numbers() {
        let diff = "@@ -10,3 +10,4 @@ fn run\n context\n-old\n+new\n+extra\n tail\n";
        let lines = parse_unified(diff);
        assert_eq!(lines[0].kind, DiffKind::Hunk);
        assert_eq!(lines[1].old, Some(10));
        assert_eq!(lines[1].new, Some(10));
        assert_eq!(lines[2].kind, DiffKind::Del);
        assert_eq!(lines[2].old, Some(11));
        assert_eq!(lines[2].new, None);
        assert_eq!(lines[3].text, "new");
        assert_eq!(lines[3].new, Some(11));
        assert_eq!(lines[4].new, Some(12));
        assert_eq!(lines[5].old, Some(12));
        assert_eq!(lines[5].new, Some(13));
    }
}
