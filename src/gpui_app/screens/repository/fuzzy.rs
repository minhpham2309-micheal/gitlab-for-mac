pub fn fuzzy_score(path: &str, query: &str) -> Option<i32> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() { return Some(0); }
    let hay = path.to_ascii_lowercase();
    if let Some(pos) = hay.find(&q) {
        let filename_start = hay.rfind('/').map(|i| i + 1).unwrap_or(0);
        let bonus = if pos >= filename_start { 400 } else { 200 };
        return Some(bonus - (hay.len() as i32) - (pos as i32));
    }
    let mut score = 0i32;
    let mut last_match: Option<usize> = None;
    let bytes = hay.as_bytes();
    let mut cursor = 0usize;
    for qc in q.bytes() {
        let found = bytes[cursor..].iter().position(|&b| b == qc).map(|off| cursor + off)?;
        score += 10;
        if last_match.is_some_and(|prev| prev + 1 == found) { score += 15; }
        if found == 0 || matches!(bytes[found - 1], b'/' | b'_' | b'-' | b'.') { score += 20; }
        last_match = Some(found);
        cursor = found + 1;
    }
    Some(score - (hay.len() as i32 / 2))
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn fuzzy_scores_filename_matches_higher_than_scattered_paths() {
        let exact = fuzzy_score("src/gpui_app/shell.rs", "shell").unwrap();
        let fuzzy = fuzzy_score("src/gpui_app/screens/header_list_local.rs", "shell").unwrap();
        assert!(exact > fuzzy);
        assert!(fuzzy_score("README.md", "xyz").is_none());
    }
}
