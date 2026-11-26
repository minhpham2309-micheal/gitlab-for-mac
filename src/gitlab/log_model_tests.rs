#[cfg(test)]
mod tests {
    use crate::gitlab::ansi_palette::GREEN;
    use crate::gitlab::log_model::{LogModel, VisibleLogRow};

    #[test]
    fn handles_split_utf8_and_split_sgr_across_chunks() {
        let mut m = LogModel::new();
        let full = "\x1b[32m✓ passed\x1b[0m\n".as_bytes();
        // Split right inside ESC sequence and inside 3-byte UTF-8 checkmark
        m.push_bytes(&full[..3]);
        m.push_bytes(&full[3..7]);
        m.push_bytes(&full[7..]);
        assert_eq!(m.line_count(), 1);
        assert_eq!(m.lines()[0].text, "✓ passed");
        assert_eq!(m.lines()[0].spans[0].color, GREEN);
    }

    #[test]
    fn bounds_unterminated_osc_and_strips_c1_and_bidi() {
        let mut m = LogModel::new();
        let mut payload = b"\x1b]0;".to_vec();
        payload.extend(std::iter::repeat_n(b'A', 300));
        payload.extend_from_slice("\nhello \u{202E}bidi \u{009B}31mworld\n".as_bytes());
        m.push_bytes(&payload);
        let plain = m.plain_all();
        assert!(plain.contains("hello bidi 31mworld"), "got: {plain:?}");
        assert!(!plain.contains('\u{202E}'));
        assert!(!plain.contains('\u{009B}'));
    }

    #[test]
    fn parses_collapsible_sections_duration_and_search_auto_expand() {
        let mut m = LogModel::new();
        m.push_bytes(
            b"section_start:100:step_script[collapsed=true]\r\x1b[0K$ cargo test\nERROR: boom\nsection_end:112:step_script\r\x1b[0KJob succeeded\r\n",
        );
        assert_eq!(m.sections().len(), 1);
        assert!(m.sections()[0].collapsed);
        assert_eq!(m.sections()[0].duration_secs, Some(12));

        // While collapsed, body lines inside section are hidden
        let collapsed_rows = m.visible_rows();
        assert!(collapsed_rows.contains(&VisibleLogRow::SectionHeader(0)));
        assert_eq!(collapsed_rows.len(), 2); // header + "Job succeeded"
        assert_eq!(m.lines().last().unwrap().text, "Job succeeded");

        // Search finds ERROR inside collapsed section and expand_for_line reveals it
        let hits = m.search("error");
        assert_eq!(hits.len(), 1);
        m.expand_for_line(hits[0].line_ix);
        assert!(!m.sections()[0].collapsed);
        assert!(m.visible_rows().len() > collapsed_rows.len());
    }

    #[test]
    fn handles_cr_progress_overwrite_without_line_explosion() {
        let mut m = LogModel::new();
        m.push_bytes(b"downloading 10%\rdownloading 50%\rdownloading 100%\n");
        assert_eq!(m.line_count(), 1);
        assert_eq!(m.lines()[0].text, "downloading 100%");
    }

    #[test]
    fn preserves_crlf_line_split_across_chunks() {
        let mut m = LogModel::new();
        m.push_bytes(b"important output\r");
        m.push_bytes(b"\nsecond line\r\n");
        assert_eq!(m.line_count(), 2);
        assert_eq!(m.lines()[0].text, "important output");
        assert_eq!(m.lines()[1].text, "second line");
    }

    #[test]
    fn scales_to_20k_lines_without_400_line_cap() {
        let mut m = LogModel::new();
        let mut buf = String::with_capacity(600_000);
        for i in 0..20_000 {
            buf.push_str(&format!("line {i}: ok\n"));
        }
        buf.push_str("line 20000: FATAL needle\n");
        m.push_bytes(buf.as_bytes());
        assert_eq!(m.line_count(), 20_001);
        let hits = m.search("needle");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_ix, 20_000);
    }
}
