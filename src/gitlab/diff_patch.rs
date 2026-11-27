use super::diff::RawDiff;

fn validate_path(path: &str) -> Result<&str, String> {
    if path.is_empty() {
        return Err("empty diff path".into());
    }
    if path.bytes().any(|b| matches!(b, b'\r' | b'\n' | b'\0')) {
        return Err("diff path contains disallowed control characters".into());
    }
    Ok(path)
}

fn format_git_path(prefix: &str, path: &str) -> String {
    let full = format!("{prefix}{path}");
    let needs_quote = full
        .bytes()
        .any(|b| b <= 0x20 || matches!(b, b'"' | b'\\' | 0x7f));
    if !needs_quote {
        return full;
    }
    let mut out = String::with_capacity(full.len() + 4);
    out.push('"');
    for ch in full.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl RawDiff {
    pub fn display_path(&self) -> &str {
        if !self.new_path.is_empty() {
            &self.new_path
        } else {
            &self.old_path
        }
    }

    pub fn to_single_file_patch(&self) -> Result<String, String> {
        let old_raw = if self.old_path.is_empty() && self.new_file {
            &self.new_path
        } else {
            &self.old_path
        };
        let new_raw = if self.new_path.is_empty() && self.deleted_file {
            &self.old_path
        } else {
            &self.new_path
        };
        let old = validate_path(old_raw)?;
        let new = validate_path(new_raw)?;
        let header_a = format_git_path("a/", old);
        let header_b = format_git_path("b/", new);
        let mut patch = format!("diff --git {header_a} {header_b}\n");
        if self.new_file {
            patch.push_str("new file mode 100644\n");
        } else if self.deleted_file {
            patch.push_str("deleted file mode 100644\n");
        } else if self.renamed_file && old != new {
            patch.push_str(&format!("rename from {old}\nrename to {new}\n"));
        }

        if self.too_large || self.collapsed {
            patch.push_str(&format!("Binary files {header_a} and {header_b} differ\n"));
            return Ok(patch);
        }

        let body = sanitize_diff_body(&self.diff);
        if body.trim().is_empty() {
            if self.diff.contains("Binary files") {
                patch.push_str(&format!("Binary files {header_a} and {header_b} differ\n"));
            }
            return Ok(patch);
        }

        let minus = if self.new_file { "/dev/null".to_string() } else { header_a };
        let plus = if self.deleted_file { "/dev/null".to_string() } else { header_b };
        patch.push_str(&format!("--- {minus}\n+++ {plus}\n"));
        patch.push_str(&body);
        if !patch.ends_with('\n') {
            patch.push('\n');
        }
        Ok(patch)
    }
}

fn parse_hunk_counts(header: &str) -> Option<(usize, usize)> {
    let body = header.strip_prefix("@@ ")?;
    let end = body.find(" @@")?;
    let mut parts = body[..end].split_whitespace();
    let old_part = parts.next()?.strip_prefix('-')?;
    let new_part = parts.next()?.strip_prefix('+')?;
    let count = |s: &str| match s.split_once(',') {
        Some((_, c)) => c.parse::<usize>().ok(),
        None => Some(1),
    };
    Some((count(old_part)?, count(new_part)?))
}

fn sanitize_diff_body(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut old_rem = 0usize;
    let mut new_rem = 0usize;
    let mut in_hunk = false;
    for line in raw.lines() {
        if let Some((old_c, new_c)) = parse_hunk_counts(line) {
            old_rem = old_c;
            new_rem = new_c;
            in_hunk = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with("diff --git ") {
            break;
        }
        if old_rem == 0 && new_rem == 0 {
            if line.starts_with('\\') {
                out.push_str(line);
                out.push('\n');
            }
            in_hunk = false;
            continue;
        }
        match line.as_bytes().first().copied() {
            Some(b' ') | None => {
                old_rem = old_rem.saturating_sub(1);
                new_rem = new_rem.saturating_sub(1);
                out.push_str(line);
                out.push('\n');
            }
            Some(b'-') => {
                old_rem = old_rem.saturating_sub(1);
                out.push_str(line);
                out.push('\n');
            }
            Some(b'+') => {
                new_rem = new_rem.saturating_sub(1);
                out.push_str(line);
                out.push('\n');
            }
            Some(b'\\') => {
                out.push_str(line);
                out.push('\n');
            }
            _ => in_hunk = false,
        }
    }
    out
}
