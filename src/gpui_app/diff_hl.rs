use gpui_kit::{
    component::diff::{DiffFile, DiffMode, DiffParseOptions, DiffState},
    Context,
};

use crate::gitlab::RawDiff;

#[derive(Clone)]
pub struct ParsedDiffBundle {
    pub files: Vec<DiffFile>,
    pub fallback_notices: Vec<(String, String)>,
    pub has_truncated: bool,
}

pub fn parse_raw_diffs_individually(raw_diffs: &[RawDiff]) -> ParsedDiffBundle {
    let opts = DiffParseOptions::new().truncated_hunks(true);
    let mut files = Vec::with_capacity(raw_diffs.len());
    let mut fallback_notices = Vec::new();
    let mut has_truncated = false;

    for raw in raw_diffs {
        let label = raw.display_path().to_string();
        if raw.too_large {
            fallback_notices.push((label, "Diff omitted by GitLab (file too large)".into()));
            continue;
        }
        if raw.collapsed {
            fallback_notices.push((label, "Diff collapsed by GitLab".into()));
            continue;
        }
        match raw.to_single_file_patch() {
            Ok(patch) => match DiffFile::parse_with(&patch, opts) {
                Ok(parsed) if parsed.len() == 1 => {
                    let file = parsed.into_iter().next().unwrap();
                    if file.is_truncated() {
                        has_truncated = true;
                    }
                    files.push(file);
                }
                Ok(parsed) if parsed.len() > 1 => {
                    fallback_notices.push((label, "Rejected multi-file payload in single diff".into()));
                }
                Ok(_) => fallback_notices.push((label, "No textual diff lines".into())),
                Err(err) => fallback_notices.push((label, format!("Unparseable diff: {err}"))),
            },
            Err(err) => fallback_notices.push((label, err)),
        }
    }

    ParsedDiffBundle {
        files,
        fallback_notices,
        has_truncated,
    }
}

pub fn new_diff_state(files: Vec<DiffFile>, mode: DiffMode, cx: &mut Context<DiffState>) -> DiffState {
    DiffState::new(files, cx).with_mode(mode)
}

pub fn language_from_path(path: &str) -> &'static str {
    let filename = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    if filename == "dockerfile" || filename.starts_with("dockerfile.") { return "dockerfile"; }
    if filename == "makefile" || filename == "gnumakefile" { return "make"; }
    if filename == "jenkinsfile" || filename.ends_with(".gradle") { return "groovy"; }
    if filename == ".env" || filename.starts_with(".env.") || filename == ".gitignore" || filename == ".dockerignore" {
        return "bash";
    }
    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" => "ts",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "js",
        "jsx" => "jsx",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "rb" => "ruby",
        "yml" | "yaml" => "yaml",
        "json" => "json",
        "toml" => "toml",
        "xml" | "svg" | "plist" => "xml",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "proto" => "protobuf",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" | "scss" => "css",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "swift" => "swift",
        "cs" => "csharp",
        "php" => "php",
        "lua" => "lua",
        "properties" | "ini" | "cfg" => "ini",
        "graphql" | "gql" => "graphql",
        "dockerfile" => "dockerfile",
        _ => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fifty_plus_file_mr_diffs_without_artificial_caps() {
        let raw_diffs: Vec<RawDiff> = (0..60)
            .map(|i| RawDiff {
                old_path: format!("src/module_{i:02}.rs"),
                new_path: format!("src/module_{i:02}.rs"),
                diff: format!(
                    "@@ -1,3 +1,4 @@\n fn f_{i}() {{\n-    let x = {i};\n+    let x = {};\n+    println!(\"{{x}}\");\n }}\n",
                    i + 1
                ),
                new_file: false,
                deleted_file: false,
                renamed_file: false,
                too_large: false,
                collapsed: false,
                generated_file: false,
            })
            .collect();

        let bundle = parse_raw_diffs_individually(&raw_diffs);
        assert_eq!(bundle.files.len(), 60);
        assert!(bundle.fallback_notices.is_empty());
        assert!(!bundle.has_truncated);
    }
}
