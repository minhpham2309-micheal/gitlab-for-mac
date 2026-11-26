pub const MAX_ESCAPE_SCAN: usize = 256;

fn snap_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

pub fn split_utf8_complete(buf: &mut Vec<u8>) -> String {
    let mut out = String::new();
    let mut consumed = 0;
    while consumed < buf.len() {
        match std::str::from_utf8(&buf[consumed..]) {
            Ok(valid) => {
                out.push_str(valid);
                consumed = buf.len();
                break;
            }
            Err(err) => {
                let valid_len = err.valid_up_to();
                if valid_len > 0 {
                    if let Ok(s) = std::str::from_utf8(&buf[consumed..consumed + valid_len]) {
                        out.push_str(s);
                    }
                    consumed += valid_len;
                }
                match err.error_len() {
                    Some(bad_len) => {
                        out.push('\u{FFFD}');
                        consumed += bad_len;
                    }
                    None => break,
                }
            }
        }
    }
    if consumed > 0 {
        buf.drain(..consumed);
    }
    out
}

pub fn is_disallowed_char(ch: char) -> bool {
    let code = ch as u32;
    (code < 0x20 && !matches!(ch, '\n' | '\r' | '\t'))
        || code == 0x7F
        || (0x80..=0x9F).contains(&code)
        || (0x202A..=0x202E).contains(&code)
        || (0x2066..=0x2069).contains(&code)
}

pub fn sanitize_streaming_escapes(input: &str, pending_esc: &mut String) -> String {
    let combined = if pending_esc.is_empty() {
        input.to_string()
    } else {
        let mut s = std::mem::take(pending_esc);
        s.push_str(input);
        s
    };
    let bytes = combined.as_bytes();
    let mut out = String::with_capacity(combined.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if i + 1 >= bytes.len() {
                pending_esc.push_str(&combined[i..]);
                break;
            }
            match bytes[i + 1] {
                b'[' => {
                    let mut j = i + 2;
                    let limit = (i + MAX_ESCAPE_SCAN).min(bytes.len());
                    while j < limit && !bytes[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    if j == bytes.len() && j - i < MAX_ESCAPE_SCAN {
                        pending_esc.push_str(&combined[i..]);
                        break;
                    }
                    if j < limit && bytes[j] == b'm' {
                        let params = &bytes[i + 2..j];
                        if params.iter().all(|b| b.is_ascii_digit() || *b == b';') {
                            out.push_str(&combined[i..=j]);
                        }
                        i = snap_char_boundary(&combined, j + 1);
                    } else {
                        let raw_next = if j < limit { j + 1 } else { limit };
                        i = snap_char_boundary(&combined, raw_next);
                    }
                    continue;
                }
                b']' => {
                    let mut j = i + 2;
                    let limit = (i + MAX_ESCAPE_SCAN).min(bytes.len());
                    let mut closed = false;
                    while j < limit {
                        if bytes[j] == 0x07 {
                            j += 1;
                            closed = true;
                            break;
                        }
                        if bytes[j] == 0x1b && j + 1 < limit && bytes[j + 1] == b'\\' {
                            j += 2;
                            closed = true;
                            break;
                        }
                        j += 1;
                    }
                    if !closed && j == bytes.len() && j - i < MAX_ESCAPE_SCAN {
                        pending_esc.push_str(&combined[i..]);
                        break;
                    }
                    i = snap_char_boundary(&combined, j);
                    continue;
                }
                _ => {
                    let next_len = combined[i + 1..].chars().next().map_or(0, char::len_utf8);
                    i = snap_char_boundary(&combined, i + 1 + next_len);
                    continue;
                }
            }
        }
        let Some(ch) = combined[i..].chars().next() else {
            break;
        };
        if !is_disallowed_char(ch) {
            out.push(ch);
        }
        i += ch.len_utf8();
    }
    out
}

pub fn strip_to_plain(input: &str) -> String {
    let mut pending = String::new();
    let kept = sanitize_streaming_escapes(input, &mut pending);
    let bytes = kept.as_bytes();
    let mut out = String::with_capacity(kept.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'm' {
                j += 1;
            }
            i = snap_char_boundary(&kept, (j + 1).min(bytes.len()));
            continue;
        }
        let Some(ch) = kept[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    out.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_esc_followed_by_multibyte_utf8_and_mid_char_scan_cap() {
        let mut p = String::new();
        let out = sanitize_streaming_escapes("\x1b✓ok", &mut p);
        assert_eq!(out, "ok");

        // Unterminated OSC whose 256-byte cap lands inside a 3-byte UTF-8 checkmark.
        let mut osc = String::from("\x1b]0;");
        osc.push_str(&"a".repeat(MAX_ESCAPE_SCAN - 5));
        osc.push_str("✓tail");
        let mut p2 = String::new();
        let out2 = sanitize_streaming_escapes(&osc, &mut p2);
        assert!(out2.ends_with("tail"), "got: {out2:?}");

        let plain = strip_to_plain("\x1b[32m✓\u{202E}ok\x1b[0m\r\nnext");
        assert_eq!(plain, "✓ok\nnext");
    }
}
