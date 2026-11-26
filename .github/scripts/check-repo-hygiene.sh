#!/usr/bin/env bash
set -euo pipefail

# Enforces repository architecture, security, and hygiene rules from AGENTS.md.
# Runnable both locally and in GitHub Actions.

errors=0
warnings=0

emit_error() {
  local msg="$1"
  local file="${2:-}"
  local line="${3:-}"
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    if [[ -n "$file" && -n "$line" ]]; then
      echo "::error file=${file},line=${line}::${msg}"
    elif [[ -n "$file" ]]; then
      echo "::error file=${file}::${msg}"
    else
      echo "::error::${msg}"
    fi
  else
    echo "ERROR: ${msg}" >&2
  fi
  errors=$((errors + 1))
}

emit_warning() {
  local msg="$1"
  local file="${2:-}"
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    if [[ -n "$file" ]]; then
      echo "::warning file=${file}::${msg}"
    else
      echo "::warning::${msg}"
    fi
  else
    echo "WARN: ${msg}" >&2
  fi
  warnings=$((warnings + 1))
}

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

echo "==> Checking tracked files, symlinks, submodules, and forbidden patterns..."
while IFS= read -r -d '' file; do
  if [[ -L "$file" ]]; then
    emit_error "Tracked symlinks are disallowed in this repository: ${file}" "$file"
    continue
  fi
  if [[ ! -f "$file" ]]; then
    emit_error "Unsupported non-regular tracked entry (e.g. git submodule): ${file}" "$file"
    continue
  fi

  lower=$(printf '%s' "$file" | tr '[:upper:]' '[:lower:]')
  base="${lower##*/}"
  case "$lower" in
    .env|.env.*|*/.env|*/.env.*|*.env|.envrc|*/.envrc|.netrc|*/.netrc|\
    .gitleaksignore|*/.gitleaksignore|\
    *.pem|*.key|*.p8|*.p12|*.pfx|*.jks|*.keystore|*.mobileprovision|\
    credentials.json|*/credentials.json|.ds_store|*/.ds_store|\
    glab-cli/config.yml|*/glab-cli/config.yml|.gitmodules|*/.gitmodules)
      emit_error "Forbidden sensitive, ignore, submodule, or OS metadata file tracked in git: ${file}" "$file"
      ;;
    .cargo/config|.cargo/config.toml|*/.cargo/config|*/.cargo/config.toml|vendor/*|*/vendor/*)
      emit_error "Unauthorized Cargo config or vendored source override tracked in git: ${file}" "$file"
      ;;
  esac
  case "$base" in
    id_rsa|id_rsa.*|id_ed25519|id_ed25519.*|id_ecdsa|id_ecdsa.*)
      emit_error "Forbidden SSH key file tracked in git: ${file}" "$file"
      ;;
  esac
done < <(git -c core.quotePath=false ls-files -z)

echo "==> Checking src/gitlab/ UI isolation (lexical Rust & Cargo.toml audit)..."
if ! python3 "${SCRIPT_DIR}/check-rust-boundaries.py"; then
  errors=$((errors + 1))
fi

echo "==> Checking for leaked local workspace paths..."
while IFS= read -r -d '' file; do
  [[ -f "$file" && ! -L "$file" ]] || continue
  case "$file" in
    Cargo.lock|.github/scripts/check-repo-hygiene.sh) continue ;;
  esac
  while IFS= read -r hit; do
    [[ -z "$hit" ]] && continue
    lineno="${hit%%:*}"
    emit_error "Possible local filesystem path leaked in ${file}:${lineno}" "$file" "$lineno"
  done < <(
    if grep -Iq . "$file" 2>/dev/null; then
      if [[ "$file" == "src/gpui_app/safe_url.rs" ]]; then
        sed 's|"vscode://file/Users/example"|""|g' "$file" \
          | grep -inE '(/Users/[^/[:space:]"'\'']+|/home/[^/[:space:]"'\'']+|[A-Za-z]:[/\\]Users[/\\][^/\\[:space:]"'\'']+)' || true
      else
        grep -inE '(/Users/[^/[:space:]"'\'']+|/home/[^/[:space:]"'\'']+|[A-Za-z]:[/\\]Users[/\\][^/\\[:space:]"'\'']+)' "$file" || true
      fi
    fi
  )
done < <(git -c core.quotePath=false ls-files -z)

echo "==> Checking source file size budget (target <=200 lines, hard cap 250 lines)..."
oversized_report=""
while IFS= read -r -d '' file; do
  [[ -f "$file" && ! -L "$file" ]] || continue
  lines=$(wc -l < "$file" | tr -d ' ')
  if (( lines > 250 )); then
    emit_error "Source file ${file} has ${lines} lines (exceeds hard cap of 250 lines; split into focused modules)." "$file"
    oversized_report+="- \`${file}\`: ${lines} lines (exceeds 250 hard cap)"$'\n'
  elif (( lines > 200 )); then
    emit_warning "Source file ${file} has ${lines} lines (soft target is <=200 lines per AGENTS.md)." "$file"
    oversized_report+="- \`${file}\`: ${lines} lines (above 200 soft target)"$'\n'
  fi
done < <(git -c core.quotePath=false ls-files -z 'src/*.rs')

if [[ -n "${COMMIT_RANGE:-}" ]]; then
  echo "==> Checking commits (${COMMIT_RANGE}) for disallowed AI attribution..."
  commits=$(git rev-list "$COMMIT_RANGE")
  ai_agents='(claude|anthropic|copilot|openai|chatgpt|gemini|antigravity|cursor|codex|aider|cline|windsurf|devin|codeium|opencode|deepseek|qwen|noreply@anthropic\.com)'
  while IFS= read -r sha; do
    [[ -z "$sha" ]] && continue
    meta=$(git show -s --format='%an <%ae>%n%cn <%ce>%n%B' "$sha")
    if printf '%s\n' "$meta" | grep -iE "(^[^[:alnum:]]*(Co-Authored-By|Signed-off-by|Assisted-by|Helped-by|Generated (with|by)):?.*${ai_agents}|^.*<[^>]*${ai_agents}[^>]*>$)" >/dev/null 2>&1; then
      emit_error "Commit ${sha:0:8} contains disallowed AI attribution or AI author metadata."
    fi
  done <<< "$commits"
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
