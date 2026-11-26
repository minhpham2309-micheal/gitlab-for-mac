use url::Url;

pub fn validate_open_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    let url = Url::parse(trimmed).map_err(|err| format!("Invalid URL: {err}"))?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!("Blocked opening non-HTTP(S) URL scheme '{scheme}'"));
        }
    }
    if url.host_str().is_none() {
        return Err("Blocked URL without host".into());
    }
    Ok(url)
}

pub fn safe_open_url(raw: &str) -> Result<String, String> {
    let url = validate_open_url(raw)?;
    open::that(url.as_str()).map_err(|err| format!("Failed to open browser: {err}"))?;
    Ok(format!("Opened {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(validate_open_url("https://gitlab.com/group/project").is_ok());
        assert!(validate_open_url("http://localhost:8080/group/project").is_ok());
    }

    #[test]
    fn rejects_dangerous_schemes_and_relative_urls() {
        for bad in [
            "file:///etc/passwd",
            "smb://evil.local/share",
            "javascript:alert(1)",
            "vscode://file/Users/example",
            "data:text/html,<script>alert(1)</script>",
            "/relative/path",
            "mailto:user@example.com",
        ] {
            assert!(validate_open_url(bad).is_err(), "should reject {bad}");
        }
    }
}
