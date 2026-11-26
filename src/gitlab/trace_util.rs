use reqwest::StatusCode;

pub(super) fn fresh_trace<'a>(
    offset: u64,
    status: StatusCode,
    range: Option<&str>,
    body: &'a [u8],
) -> (u64, &'a [u8]) {
    if let Some(start) = range.and_then(range_start) {
        if start >= offset {
            return (start + body.len() as u64, body);
        }
        let skip = (offset - start) as usize;
        if skip >= body.len() {
            return (offset, &[]);
        }
        let fresh = &body[skip..];
        return (offset + fresh.len() as u64, fresh);
    }
    if status == StatusCode::PARTIAL_CONTENT {
        return (offset + body.len() as u64, body);
    }
    let Some(start) = usize::try_from(offset).ok() else {
        return (offset, &[]);
    };
    if start >= body.len() {
        return (offset, &[]);
    }
    (body.len() as u64, &body[start..])
}

fn range_start(header: &str) -> Option<u64> {
    let start = header.trim().strip_prefix("bytes ")?.split('-').next()?.trim();
    if start == "*" {
        return None;
    }
    start.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_body_response_appends_only_new_bytes() {
        let body = b"hello world";
        let (next, fresh) = fresh_trace(6, StatusCode::OK, None, body);
        assert_eq!(fresh, b"world");
        assert_eq!(next, 11);
    }

    #[test]
    fn content_range_skips_bytes_already_shown() {
        let body = b"hello world";
        let (next, fresh) = fresh_trace(6, StatusCode::OK, Some("bytes 0-10/11"), body);
        assert_eq!(fresh, b"world");
        assert_eq!(next, 11);
    }

    #[test]
    fn partial_range_is_the_new_chunk() {
        let body = b"world";
        let (next, fresh) = fresh_trace(6, StatusCode::PARTIAL_CONTENT, Some("bytes 6-10/11"), body);
        assert_eq!(fresh, b"world");
        assert_eq!(next, 11);
    }
}
