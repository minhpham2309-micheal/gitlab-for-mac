use gpui_kit::{
    component::{ActiveTheme as _, Icon, IconName},
    div, rgb, App, Hsla, IntoElement, ParentElement as _, Styled as _,
};

pub fn file_accent_color(name: &str, is_dir: bool, dark: bool) -> Hsla {
    if is_dir {
        return rgb(if dark { 0x9E9CA5 } else { 0x626168 }).into();
    }
    let lower = name.to_ascii_lowercase();
    if lower == ".gitlab-ci.yml" { return rgb(0xFC6D26).into(); }
    if lower == "dockerfile" || lower == ".dockerignore" || lower.starts_with("docker-compose") {
        return rgb(0x2496ED).into();
    }
    if lower == ".gitignore" || lower == ".gitattributes" { return rgb(0xF05032).into(); }
    if lower.starts_with(".env") { return rgb(0x3B82F6).into(); }
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => rgb(0xDEA584).into(),
        "java" | "gradle" | "kts" => rgb(0xE07A38).into(),
        "ts" | "tsx" => rgb(0x3178C6).into(),
        "js" | "jsx" | "mjs" => rgb(0xF1E05A).into(),
        "py" => rgb(0x3572A5).into(),
        "go" => rgb(0x00ADD8).into(),
        "md" | "markdown" => rgb(0x519ABA).into(),
        "yml" | "yaml" | "toml" | "json" => rgb(0xCB6CE6).into(),
        "sh" | "bash" | "zsh" => rgb(0x89E051).into(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => rgb(0xA074C4).into(),
        _ => rgb(if dark { 0x89888D } else { 0x626168 }).into(),
    }
}

pub fn render_entry_icon(name: &str, is_dir: bool, cx: &App) -> impl IntoElement {
    let dark = cx.theme().is_dark();
    let color = file_accent_color(name, is_dir, dark);
    if is_dir {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .size_4()
            .flex_shrink_0()
            .child(Icon::new(IconName::FolderClosed).size_4().text_color(color));
    }
    let lower = name.to_ascii_lowercase();
    let icon = if lower == ".gitlab-ci.yml" {
        Icon::empty().path("icons/glab-tanuki.svg").size_3p5().text_color(color)
    } else if lower.ends_with(".md") || lower.ends_with(".txt") || lower == "license" {
        Icon::new(IconName::FileText).size_4().text_color(color)
    } else {
        Icon::new(IconName::File).size_4().text_color(color)
    };
    div().flex().items_center().justify_center().size_4().flex_shrink_0().child(icon)
}

pub fn language_color(lang: &str) -> Hsla {
    match lang.to_ascii_lowercase().as_str() {
        "java" => rgb(0xB07219).into(),
        "rust" => rgb(0xDEA584).into(),
        "typescript" => rgb(0x3178C6).into(),
        "javascript" => rgb(0xF1E05A).into(),
        "python" => rgb(0x3572A5).into(),
        "go" => rgb(0x00ADD8).into(),
        "kotlin" => rgb(0xA97BFF).into(),
        "shell" | "bash" => rgb(0x89E051).into(),
        "dockerfile" => rgb(0x384D54).into(),
        "html" => rgb(0xE34C26).into(),
        "css" | "scss" => rgb(0x563D7C).into(),
        "ruby" => rgb(0x701516).into(),
        "c" => rgb(0x555555).into(),
        "c++" => rgb(0xF34B7D).into(),
        "swift" => rgb(0xF05138).into(),
        "sql" | "plpgsql" => rgb(0xE38C00).into(),
        _ => rgb(0x63A6E9).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_gitlab_ci_and_language_colors() {
        let ci = file_accent_color(".gitlab-ci.yml", false, true);
        let plain = file_accent_color("notes.unknown", false, true);
        assert_ne!(ci, plain);
        assert_ne!(language_color("Java"), language_color("Shell"));
    }
}
