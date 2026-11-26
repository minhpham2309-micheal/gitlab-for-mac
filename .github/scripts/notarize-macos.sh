#!/usr/bin/env bash
# Shared Apple notarization helper sourced by package-macos.sh.
# Expects: WORK_DIR, REQUIRE_SIGNING, APPLE_SIGNING_IDENTITY, and optional API key / Apple ID vars.

NOTARY_MODE="none"
NOTARY_PROFILE=""

notary_init() {
  local has_api_key=0
  if [[ -n "${APPLE_API_KEY_P8_BASE64:-}" || -n "${APPLE_API_KEY_ID:-}" || -n "${APPLE_API_ISSUER_ID:-}" ]]; then
    if [[ -z "${APPLE_API_KEY_P8_BASE64:-}" || -z "${APPLE_API_KEY_ID:-}" || -z "${APPLE_API_ISSUER_ID:-}" ]]; then
      echo "ERROR: Partial Apple API key notarization secrets set (require P8, KEY_ID, ISSUER_ID)." >&2
      exit 1
    fi
    has_api_key=1
    NOTARY_MODE="api-key"
  fi
  if (( has_api_key == 0 )) && [[ -n "${APPLE_ID:-}" || -n "${APPLE_TEAM_ID:-}" || -n "${APPLE_APP_PASSWORD:-}" ]]; then
    if [[ -z "${APPLE_ID:-}" || -z "${APPLE_TEAM_ID:-}" || -z "${APPLE_APP_PASSWORD:-}" ]]; then
      echo "ERROR: Partial Apple ID notarization secrets set (require APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD)." >&2
      exit 1
    fi
    NOTARY_MODE="apple-id"
  fi
  if [[ "$NOTARY_MODE" != "none" && -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    echo "ERROR: Notarization secrets set without APPLE_SIGNING_IDENTITY." >&2
    exit 1
  fi
  if (( REQUIRE_SIGNING == 1 )) && [[ -z "${APPLE_SIGNING_IDENTITY:-}" || "$NOTARY_MODE" == "none" ]]; then
    echo "ERROR: --require-signing set, but Developer ID signing and notarization secrets are incomplete." >&2
    exit 1
  fi

  if [[ "$NOTARY_MODE" == "api-key" ]]; then
    # Isolate umask 077 in a subshell so packaged .app/archive permissions stay world-readable.
    (
      umask 077
      printf '%s' "$APPLE_API_KEY_P8_BASE64" | tr -d '[:space:]' | base64 --decode > "${WORK_DIR}/AuthKey.p8"
    )
  elif [[ "$NOTARY_MODE" == "apple-id" ]]; then
    NOTARY_PROFILE="gitlab-for-mac-notary-$$"
    local -a store_args=(--apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --no-validate)
    if [[ -f "${RUNNER_TEMP:-}/app-signing.keychain-db" ]]; then
      store_args+=(--keychain "${RUNNER_TEMP}/app-signing.keychain-db")
    fi
    xcrun notarytool store-credentials "$NOTARY_PROFILE" "${store_args[@]}" >/dev/null
  fi
}

notary_cleanup() {
  if [[ -n "$NOTARY_PROFILE" ]]; then
    local -a kc_args=()
    if [[ -f "${RUNNER_TEMP:-}/app-signing.keychain-db" ]]; then
      kc_args=("${RUNNER_TEMP}/app-signing.keychain-db")
    fi
    security delete-generic-password -s "com.apple.gke.notary.tool.com.apple.security.keychain-profile.${NOTARY_PROFILE}" ${kc_args[@]+"${kc_args[@]}"} 2>/dev/null || true
    security delete-generic-password -a "com.apple.gke.notary.tool.saved-creds.${NOTARY_PROFILE}" ${kc_args[@]+"${kc_args[@]}"} 2>/dev/null || true
  fi
}

notary_submit() {
  local artifact="$1"
  local submit_json="${WORK_DIR}/notary-result.json"
  local -a auth_args=()
  if [[ "$NOTARY_MODE" == "api-key" ]]; then
    auth_args=(--key "${WORK_DIR}/AuthKey.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER_ID")
  else
    auth_args=(--keychain-profile "$NOTARY_PROFILE")
    if [[ -f "${RUNNER_TEMP:-}/app-signing.keychain-db" ]]; then
      auth_args+=(--keychain "${RUNNER_TEMP}/app-signing.keychain-db")
    fi
  fi
  set +e
  xcrun notarytool submit "$artifact" "${auth_args[@]}" \
    --wait --timeout 15m --output-format json > "$submit_json"
  local rc=$?
  set -e
  local sub_id status
  sub_id=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d.get("id",""))' "$submit_json" 2>/dev/null || true)
  status=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d.get("status",""))' "$submit_json" 2>/dev/null || true)
  if (( rc != 0 )) || [[ "$status" != "Accepted" ]]; then
    echo "ERROR: Notarization failed for ${artifact} (rc=${rc}, status=${status}, id=${sub_id})" >&2
    if [[ -n "$sub_id" ]]; then
      xcrun notarytool log "$sub_id" "${auth_args[@]}" >&2 || true
    fi
    exit 1
  fi
}
