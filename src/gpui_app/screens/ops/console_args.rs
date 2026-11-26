const SENSITIVE_SUBCOMMANDS: &[&str] = &["auth", "config", "alias"];
const TOKEN_PREFIXES: &[&str] = &["glpat-", "gldt-", "glrt-", "glcbt-", "glsoat-"];

pub fn parse_console_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;
    let mut has_token = false;

    for ch in input.chars() {
        if escape {
            cur.push(ch);
            escape = false;
            has_token = true;
            continue;
        }
        match quote {
            Some(q) if ch == q => quote = None,
            Some('"') if ch == '\\' => escape = true,
            Some(_) => {
                cur.push(ch);
                has_token = true;
            }
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    has_token = true;
                }
                '\\' => escape = true,
                c if c.is_whitespace() => {
                    if has_token {
                        args.push(std::mem::take(&mut cur));
                        has_token = false;
                    }
                }
                c => {
                    cur.push(c);
                    has_token = true;
                }
            },
        }
    }
    if quote.is_some() {
        return Err("Unclosed quote in command".into());
    }
    if has_token {
        args.push(cur);
    }
    if args.first().map(String::as_str) == Some("glab") {
        args.remove(0);
    }
    Ok(args)
}

pub fn is_valid_repo_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return false;
    }
    parts.into_iter().all(|seg| {
        !seg.is_empty()
            && seg != "."
            && seg != ".."
            && seg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    })
}

pub fn inject_repo_flag(args: &mut Vec<String>, repo: Option<&str>) {
    let Some(repo) = repo.filter(|r| is_valid_repo_path(r)) else {
        return;
    };
    let end_flags = args.iter().position(|a| a == "--").unwrap_or(args.len());
    let has_repo = args[..end_flags]
        .iter()
        .any(|a| a == "-R" || a == "--repo" || a.starts_with("--repo=") || a.starts_with("-R="));
    if !has_repo {
        args.insert(end_flags, repo.to_string());
        args.insert(end_flags, "-R".into());
    }
}

pub fn is_sensitive_subcommand(args: &[String]) -> Option<&'static str> {
    let end_flags = args.iter().position(|a| a == "--").unwrap_or(args.len());
    for token in &args[..end_flags] {
        if token.starts_with('-') {
            continue;
        }
        if let Some(&cmd) = SENSITIVE_SUBCOMMANDS
            .iter()
            .find(|&&cmd| cmd.eq_ignore_ascii_case(token))
        {
            return Some(cmd);
        }
    }
    None
}

pub fn redact_tokens(input: &str) -> String {
    let mut out = input.to_string();
    for prefix in TOKEN_PREFIXES {
        let mut cursor = 0;
        while let Some(rel) = out[cursor..].find(prefix) {
            let start = cursor + rel;
            let mut end = start + prefix.len();
            while end < out.len() {
                let b = out.as_bytes()[end];
                if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.') {
                    end += 1;
                } else {
                    break;
                }
            }
            out.replace_range(start..end, "[REDACTED-TOKEN]");
            cursor = start + "[REDACTED-TOKEN]".len();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_args_and_strips_leading_glab() {
        let args = parse_console_args(r#"glab mr list --label "needs review" -- -x"#).unwrap();
        assert_eq!(args, vec!["mr", "list", "--label", "needs review", "--", "-x"]);
        assert!(parse_console_args("mr list \"unclosed").is_err());
    }

    #[test]
    fn injects_validated_repo_before_double_dash() {
        let mut args = vec!["ci".into(), "list".into(), "--".into(), "extra".into()];
        inject_repo_flag(&mut args, Some("group/sub/proj-1"));
        assert_eq!(args, vec!["ci", "list", "-R", "group/sub/proj-1", "--", "extra"]);

        let mut bad = vec!["ci".into(), "list".into()];
        inject_repo_flag(&mut bad, Some("../evil;rm"));
        assert_eq!(bad, vec!["ci", "list"]);
    }

    #[test]
    fn flags_sensitive_subcommands_and_redacts_tokens() {
        assert_eq!(is_sensitive_subcommand(&["auth".into(), "status".into()]), Some("auth"));
        assert_eq!(
            is_sensitive_subcommand(&["-R".into(), "group/proj".into(), "auth".into(), "status".into()]),
            Some("auth")
        );
        assert_eq!(
            is_sensitive_subcommand(&["--hostname".into(), "gl.example.com".into(), "config".into(), "get".into()]),
            Some("config")
        );
        assert_eq!(is_sensitive_subcommand(&["alias".into(), "set".into()]), Some("alias"));
        assert_eq!(is_sensitive_subcommand(&["ci".into(), "status".into()]), None);

        let cleaned = redact_tokens("token=glpat-abc123_XYZ and glrt-999!");
        assert_eq!(cleaned, "token=[REDACTED-TOKEN] and [REDACTED-TOKEN]!");
        assert!(!cleaned.contains("glpat-"));
    }
}
