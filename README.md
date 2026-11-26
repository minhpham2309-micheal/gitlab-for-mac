# GitLab for Mac

A fast, native, GPU-accelerated macOS desktop client for GitLab built in Rust with **GPUI** (`gpui-kit`), powered by your existing [`glab`](https://gitlab.com/gitlab-org/cli) CLI authentication.

> **Status:** Active early development — contributions, bug reports, and architecture discussions are warmly welcome.

---

## Why GitLab for Mac?

Working across multiple GitLab projects and self-hosted instances in a browser quickly turns into dozens of heavy tabs—especially when reviewing large merge request diffs, tailing long CI/CD logs, or jumping between repositories.

**GitLab for Mac** (`glab-mac`) provides a keyboard-first native workspace with zero Electron overhead:

- **Instant GPU rendering** via GPUI with native macOS feel and Pajamas-inspired Dark & Light themes.
- **Zero extra login flow** — reads your existing `glab` CLI hosts and tokens directly.
- **Built for daily engineering ops** — MRs, streaming CI logs, repository browsing, issues, to-dos, and an embedded `glab` console in one window.

---

## Features

- **Multi-Host Support:** Switch seamlessly between `gitlab.com` and self-hosted GitLab instances configured in `glab`, with full cache and request isolation on host switch.
- **Project Overview & Repository Explorer:**
  - Directory tree with last-commit metadata (with automatic fallback for older self-hosted GitLab instances).
  - Syntax-highlighted source viewer (`syntect`), Markdown/README rendering, binary and oversize file guards.
  - Fuzzy file finder (`⌘T`), branches, tags, and commit diff views.
- **Merge Requests & Code Review:**
  - Filter by Open, Merged, Closed, or All.
  - Unified and Split diff views with inline discussions, reply/resolve actions, and Approve / Merge / Close workflows guarded by confirmation dialogs.
- **CI/CD Pipelines & High-Throughput Job Logs:**
  - Stage and downstream bridge pipeline graph with run, retry, cancel, manual job play, and artifact downloads.
  - Streaming ANSI log model (`src/gitlab/log_model.rs`) with collapsible GitLab log sections, live follow (`⌘L`), in-log search (`⌘F` / `⌘G`), and support for 20k+ lines without artificial truncation.
- **Issues, To-Do List & Operations:**
  - Native views for Issues, To-Do List, Releases, Pipeline Schedules, Runners, Code Search, and an integrated `glab` CLI console.
- **Keyboard-First Navigation:**
  - Command palette (`⌘K`), back/forward navigation history (`⌘[` / `⌘]`), collapsible super-sidebar (`⌘\`), live UI zoom (`⌘=` / `⌘-` / `⌘0`), and customizable typography/layout preferences (`⌘,`).

---

## Security & Isolation Model

Because a GitLab client handles personal access tokens and untrusted repository/issue content, security boundaries are enforced in core modules:

- **Origin-bound credentials** (`src/gitlab/http_util.rs`, `src/config.rs`): Access tokens are only attached to the configured GitLab origin over HTTPS (or loopback HTTP for local testing), never re-attached across cross-origin redirects, and redacted in `Debug` output.
- **SSRF & media guards** (`src/gitlab/media.rs`, `src/gpui_app/markdown.rs`, `src/gpui_app/safe_url.rs`): Blocks loopback, private-network, IPv4-mapped, and local domains for remote images; validates URL schemes (`http`/`https` only) before opening links in the browser.
- **Console guardrails** (`src/gpui_app/screens/ops/console_args.rs`, `src/glab.rs`): Flags sensitive `glab` subcommands (`auth`, `config`, `alias`), redacts GitLab token prefixes, and bounds command execution with a 30-second timeout and 4 MB output cap.
- **Host-switch isolation** (`src/gpui_app/state.rs`, `src/runtime.rs`): Switching hosts increments generation epochs, cancels in-flight Tokio tasks via `AbortOnDrop`, and clears `ImageCache` and `RepoCache`.

---

## Quick Start

### Prerequisites

- **macOS** (Apple Silicon or Intel)
- **Rust toolchain** (Edition 2021 — install via [rustup](https://rustup.rs))
- **[`glab` CLI](https://gitlab.com/gitlab-org/cli)** authenticated with at least one GitLab host:

```bash
glab auth login
```

GitLab for Mac automatically reads hosts and tokens from `glab`'s config file (`~/Library/Application Support/glab-cli/config.yml` or `~/.config/glab-cli/config.yml`). Local UI preferences are stored separately at `~/Library/Application Support/glab-mac/prefs.json`.

### Build & Run

```bash
git clone https://github.com/minhpham2309-micheal/gitlab-for-mac.git
cd gitlab-for-mac

# Development run
cargo run

# Optimized release build (LTO + stripped binary)
cargo build --release
./target/release/glab-mac
```

---

## Keyboard Shortcuts

Keybindings are registered in [`src/gpui_app/actions.rs`](src/gpui_app/actions.rs):

| Shortcut | Action |
|---|---|
| `⌘K` | Open / toggle Command Palette |
| `⌘T` | Fuzzy Find File in repository |
| `⌘P` | Jump to Projects switcher |
| `⌘[` / `⌘]` | Navigate Back / Forward |
| `⌘\` | Collapse / Expand sidebar |
| `⌘⇧T` | Toggle Dark / Light Pajamas theme |
| `⌘R` | Refresh active screen |
| `⌘,` | Open Preferences |
| `⌘=` / `⌘-` / `⌘0` | Zoom In / Zoom Out / Reset UI scale |
| `⌘L` | Toggle live-follow in Job Log |
| `⌘F` / `⌘G` / `⌘⇧G` | Search in Job Log / Next / Previous match |
| `⌘⇧C` | Copy full Job Log |
| `Esc` | Dismiss active overlay / modal |

---

## Architecture & Project Structure

The codebase enforces a clean separation between **pure GitLab/domain logic** (zero UI dependency) and the **GPUI desktop layer**:

```text
┌──────────────────────────────────────────────────────────────────────┐
│                     GPUI Desktop Shell (src/gpui_app/)               │
│   AppShell · TopBar · Super Sidebar · Command Palette · Screens      │
│   AppState (epochs, nav history, prefs) · ImageCache · RepoCache     │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ TokioBridge (AbortOnDrop)
                                ▼
┌───────────────────────────────┴──────────────────────────────────────┐
│                       Core & Data Layer (UI-free)                    │
│  src/gitlab/  REST v4 client · redirect/SSRF guards · diff & log     │
│  src/config*  glab YAML loader · origin & scheme validation          │
│  src/glab.rs  sandboxed glab CLI subprocess runner                   │
│  src/nav.rs   screen/section model  ·  src/prefs.rs  persistence     │
└──────────────────────────────────────────────────────────────────────┘
```

### Directory Overview

| Path | Responsibility |
|---|---|
| `src/main.rs`, `src/lib.rs` | Binary entry point (`glab_mac::gpui_app::run`) and library exports |
| `src/config.rs`, `src/config_parse.rs` | `glab-cli/config.yml` discovery, parsing, and host/origin validation |
| `src/gitlab/` | Async GitLab REST v4 client (`reqwest` + `rustls`), redirect & media security guards, unified diff parser, ANSI/collapsible job log engine, and API data types |
| `src/glab.rs` | Bounded async `glab` subprocess execution (30s timeout, 4 MB cap) |
| `src/runtime.rs` | Multi-threaded Tokio runtime bridge with automatic task cancellation on drop |
| `src/nav.rs`, `src/prefs.rs` | Navigation taxonomy (16 screens, 9 sidebar sections) and clamped UI preferences |
| `src/gpui_app/` | GPUI shell, Pajamas theme, sidebar, topbar, statusbar, command palette, avatar/image/repo caches, safe markdown/URL helpers |
| `src/gpui_app/screens/` | Feature screens: `overview/`, `repository/`, `code_refs/`, `ops/`, merge requests, pipelines & job logs, issues, to-dos, projects, and preferences |
| `assets/`, `fonts/` | Bundled SVG icons and JetBrains Mono font files |
| `tests/` | Headless GPUI UI integration tests (`gpui_shell_tests.rs`, `gpui_repo_tests.rs`) |

### Engineering Conventions

- **Keep `src/gitlab/` UI-agnostic:** Data fetching, diff parsing, ANSI log parsing, and security validation must stay independent of `gpui-kit` so they remain fast to test and easy to reuse.
- **Small, focused modules:** Prefer splitting large screens into cohesive modules (roughly ~200 lines per file) over monolithic views.
- **Stale-response protection:** Async UI loads check `host_epoch` / `project_epoch` before committing results to state.
- **Headless UI testing:** UI regressions are verified automatically with `#[gpui_kit::test]` without launching a visible desktop window.

---

## Development & Testing

```bash
# Run all unit tests and headless GPUI integration tests
cargo test

# Run linter across library, binary, and integration tests
cargo clippy --all-targets

# Build optimized release binary
cargo build --release
```

---

## Contributing

Contributions of all sizes are welcome—whether it's a bug fix, UI refinement, performance improvement, packaging help, or a brand-new GitLab feature.

### Great Areas to Contribute

- **macOS Distribution & Packaging:** `.app` bundle generation, application icon, code-signing/notarization workflow, DMG releases, and Homebrew Cask formula.
- **CI/CD Automation:** GitHub Actions workflows for `cargo test`, `cargo clippy`, `cargo fmt`, and tagged macOS release artifacts.
- **Expanding Native Sidebar Sections:** Sections currently opening in GitLab Web (`Manage`, `Secure`, `Operate`, `Monitor`, `Analyze`) are natural candidates for native views.
- **Code Review Enhancements:** Inline code suggestions, draft review comments, pipeline trigger variable editor, and MR commit-by-commit diffs.
- **UI Polish & Accessibility:** Keyboard navigation across lists/diffs, theme refinements, and screen-reader labels.
- **Cross-Platform Exploration:** Evaluating GPUI support on Linux and Windows.

### Pull Request Workflow

1. **Discuss larger changes first:** Open an Issue before starting major features or architectural shifts so we can align on scope.
2. **Fork & branch:** Create a focused branch (`feat/<slug>`, `fix/<slug>`, `docs/<slug>`).
3. **Keep tests & lint green:** Add unit tests (for `src/gitlab/` or core logic) or headless `#[gpui_kit::test]` coverage (for UI/layout changes) and verify:
   ```bash
   cargo test && cargo clippy --all-targets
   ```
4. **Keep fixtures clean:** Never commit real tokens, `.env` files, or internal hostnames—use synthetic `*.example.com` / `*.test` values in tests.
5. **Use Conventional Commits:** `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`.

---

## License

Licensed under the [MIT License](LICENSE).
