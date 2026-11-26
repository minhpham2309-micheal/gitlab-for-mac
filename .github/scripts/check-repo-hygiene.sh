#!/usr/bin/env bash
set -euo pipefail

# Enforces repository architecture and hygiene rules from AGENTS.md.
# Runnable both locally and in GitHub Actions.

errors=0
warnings=0

emit_error() {
  local msg="$1"
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "::error::${msg}"
  else
    echo "ERROR: ${msg}" >&2
  fi
  errors=$((errors + 1))
}

emit_warning() {
  local msg="$1"
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "::warning::${msg}"
  else
    echo "WARN: ${msg}" >&2
  fi
  warnings=$((warnings + 1))
}

echo "==> Checking forbidden tracked files..."
while IFS= read -r file; do
  case "$file" in
    .env|.env.*|*/.env|*/.env.*|*.pem|*.key|*.p12|credentials.json|*/credentials.json|.DS_Store|*/.DS_Store)
      emit_error "Forbidden sensitive or OS metadata file is tracked in git: ${file}"
      ;;
  esac
done < <(git ls-files)

echo "==> Checking src/gitlab/ UI isolation (no gpui_kit in non-test code)..."
while IFS= read -r file; do
  case "$file" in
    *_tests.rs) continue ;;
  esac
  if grep -nE '\bgpui_kit\b|\bgpui::' "$file" >/dev/null 2>&1; then
    matches=$(grep -nE '\bgpui_kit\b|\bgpui::' "$file")
    emit_error "UI dependency found in UI-agnostic module ${file}: ${matches}"
  fi
done < <(git ls-files 'src/gitlab/*.rs' 'src/gitlab/**/*.rs')

echo "==> Checking for leaked local workspace paths..."
while IFS= read -r file; do
  case "$file" in
    Cargo.lock|.github/scripts/check-repo-hygiene.sh) continue ;;
    *.png|*.ttf|*.otf|*.icns|*.ico|*.jpg|*.jpeg|*.webp|*.pdf) continue ;;
  esac
  # Flag local user paths such as /Users/<name>, /home/<name>, or C:\Users\<name>,
  # while permitting the synthetic /Users/example placeholder used in unit tests.
  if matches=$(grep -nE '(/Users/[^/[:space:]"'\'']+|/home/[^/[:space:]"'\'']+|[A-Za-z]:\\Users\\[^\\[:space:]"'\'']+)' "$file" \
    | grep -vE '/Users/example\b' || true); then
    if [[ -n "$matches" ]]; then
      emit_error "Possible local filesystem path leaked in ${file}: ${matches}"
    fi
  fi
done < <(git ls-files)

echo "==> Checking source file size budget (target <=200 lines, hard cap 250 lines)..."
oversized_report=""
while IFS= read -r file; do
  lines=$(wc -l < "$file" | tr -d ' ')
  if (( lines > 250 )); then
    emit_error "Source file ${file} has ${lines} lines (exceeds hard cap of 250 lines; split into focused modules)."
    oversized_report+="- \`${file}\`: ${lines} lines (exceeds 250 hard cap)"$'\n'
  elif (( lines > 200 )); then
    emit_warning "Source file ${file} has ${lines} lines (soft target is <=200 lines per AGENTS.md)."
    oversized_report+="- \`${file}\`: ${lines} lines (above 200 soft target)"$'\n'
  fi
done < <(git ls-files 'src/*.rs' 'src/**/*.rs')

if [[ -n "${BASE_SHA:-}" && -n "${HEAD_SHA:-}" ]]; then
  echo "==> Checking PR commits for disallowed AI attribution trailers..."
  while IFS= read -r sha; do
    msg=$(git show -s --format=%B "$sha")
    if echo "$msg" | grep -iE '^Co-Authored-By:.*(claude|anthropic|copilot|openai|chatgpt|gemini|antigravity|cursor|noreply@anthropic\.com)' >/dev/null 2>&1; then
      emit_error "Commit ${sha:0:8} contains a disallowed AI Co-Authored-By attribution line."
    fi
  done < <(git rev-list "${BASE_SHA}..${HEAD_SHA}")
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "### Repository Hygiene & Architecture Audit"
    echo ""
    echo "- **Errors:** ${errors}"
    echo "- **Warnings:** ${warnings}"
    if [[ -n "$oversized_report" ]]; then
      echo ""
      echo "#### Files above 200-line soft target"
      printf '%s' "$oversized_report"
    fi
  } >> "$GITHUB_STEP_SUMMARY"
fi

if (( errors > 0 )); then
  echo "Hygiene audit failed with ${errors} error(s)." >&2
  exit 1
fi

echo "Hygiene audit passed (${warnings} warning(s))."
