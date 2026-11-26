pub fn ellipsis(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let tail: String = text.chars().rev().take(max.saturating_sub(1)).collect();
    format!("…{}", tail.chars().rev().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::ellipsis;

    #[test]
    fn ellipsis_keeps_short_text() {
        assert_eq!(ellipsis("abc", 8), "abc");
    }

    #[test]
    fn ellipsis_keeps_path_tail() {
        assert_eq!(ellipsis("group/very/long/project", 10), "…g/project");
    }
}
