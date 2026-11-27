use super::diff::RawDiff;

fn raw(old: &str, new: &str, diff: &str) -> RawDiff {
    RawDiff {
        old_path: old.into(),
        new_path: new.into(),
        diff: diff.into(),
        new_file: false,
        renamed_file: false,
        deleted_file: false,
        too_large: false,
        collapsed: false,
        generated_file: false,
    }
}

#[test]
fn rejects_newline_and_null_in_paths() {
    let bad = raw("a.rs\ndiff --git a/x b/x", "b.rs", "@@ -1,1 +1,1 @@\n-a\n+b\n");
    assert!(bad.to_single_file_patch().is_err());
    let null_bad = raw("a.rs", "b\0.rs", "@@ -1,1 +1,1 @@\n-a\n+b\n");
    assert!(null_bad.to_single_file_patch().is_err());
}

#[test]
fn formats_modified_added_deleted_renamed_and_spaced_paths() {
    let modified = raw("src/a b.rs", "src/a b.rs", "@@ -1,1 +1,1 @@\n-old\n+new\n");
    let p = modified.to_single_file_patch().unwrap();
    assert!(p.contains("\"a/src/a b.rs\" \"b/src/a b.rs\""));

    let mut added = raw("src/new.rs", "src/new.rs", "@@ -0,0 +1,1 @@\n+hello\n");
    added.new_file = true;
    assert!(added.to_single_file_patch().unwrap().contains("--- /dev/null"));

    let mut deleted = raw("src/old.rs", "src/old.rs", "@@ -1,1 +0,0 @@\n-bye\n");
    deleted.deleted_file = true;
    assert!(deleted.to_single_file_patch().unwrap().contains("+++ /dev/null"));

    let mut renamed = raw("src/a.rs", "src/b.rs", "@@ -1,1 +1,1 @@\n-a\n+b\n");
    renamed.renamed_file = true;
    assert!(renamed.to_single_file_patch().unwrap().contains("rename from src/a.rs"));
}

#[test]
fn strips_smuggled_git_and_non_git_file_headers_from_diff_body() {
    let smuggled = raw(
        "src/a.rs",
        "src/a.rs",
        "@@ -1,1 +1,1 @@\n-old\n+new\ndiff --git a/evil b/evil\n--- a/evil\n+++ b/evil\n",
    );
    let patch = smuggled.to_single_file_patch().unwrap();
    assert_eq!(patch.matches("diff --git ").count(), 1);
    assert!(!patch.contains("evil"));

    let unprefixed = raw(
        "src/a.rs",
        "src/a.rs",
        "@@ -1,1 +1,1 @@\n-old\n+new\n--- a/evil2\n+++ b/evil2\n",
    );
    let patch2 = unprefixed.to_single_file_patch().unwrap();
    assert!(!patch2.contains("evil2"));
}

#[test]
fn round_trips_through_gpui_diff_file_parser() {
    use gpui_kit::component::diff::{DiffFile, DiffParseOptions};

    let sample = raw(
        "src/lib.rs",
        "src/lib.rs",
        "@@ -1,3 +1,3 @@\n fn a() {}\n-let x = 1;\n+let x = 2;\n fn b() {}\n",
    );
    let patch = sample.to_single_file_patch().unwrap();
    let files =
        DiffFile::parse_with(&patch, DiffParseOptions::new().truncated_hunks(true)).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path().as_ref(), "src/lib.rs");
    assert_eq!(files[0].additions(), 1);
    assert_eq!(files[0].deletions(), 1);
}

#[test]
fn deserializes_null_diff_metadata_fields_from_gitlab_commit_diff_api() {
    let payload = r#"[
        {
            "old_path": "src/app.rs",
            "new_path": "src/app.rs",
            "a_mode": "100644",
            "b_mode": "100644",
            "diff": "--- a/src/app.rs\n+++ b/src/app.rs\n@@ -1,2 +1,3 @@\n fn main() {}\n+// ok\n // end\n",
            "new_file": false,
            "renamed_file": false,
            "deleted_file": false,
            "too_large": null,
            "collapsed": null,
            "generated_file": null
        },
        {
            "old_path": null,
            "new_path": "assets/new.txt",
            "diff": null,
            "new_file": true,
            "renamed_file": null,
            "deleted_file": null,
            "generated_file": null
        }
    ]"#;
    let diffs: Vec<RawDiff> = serde_json::from_str(payload).unwrap();
    assert_eq!(diffs.len(), 2);
    assert!(!diffs[0].generated_file);
    assert!(!diffs[0].too_large);
    assert!(!diffs[0].collapsed);
    assert!(diffs[0].to_single_file_patch().is_ok());

    assert_eq!(diffs[1].old_path, "");
    assert_eq!(diffs[1].new_path, "assets/new.txt");
    assert!(diffs[1].new_file);
    assert!(diffs[1].to_single_file_patch().unwrap().contains("new file mode 100644"));
}
