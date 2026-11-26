#!/usr/bin/env bash
set -euo pipefail

# Packages a compiled macOS release binary into:
#   1. GitLab-for-Mac-<tag>-<arch>.app.zip  (signed & stapled .app bundle)
#   2. GitLab-for-Mac-<tag>-<arch>.dmg      (drag-to-Applications disk image)
#   3. gitlab-for-mac-<tag>-<target>.tar.gz (signed standalone CLI archive)
#   4. SHA256SUMS-<arch>.txt                (verified SHA-256 manifest)

usage() {
  cat <<'EOF' >&2
Usage: package-macos.sh --binary <path> --version <semver> --tag <label> --arch <arm64|x86_64> --target <triple> --out-dir <dir> [--build-number <int>] [--require-signing]
EOF
  exit 2
}

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/../.." && pwd)
# shellcheck source=.github/scripts/notarize-macos.sh
source "${SCRIPT_DIR}/notarize-macos.sh"

BINARY="" VERSION="" TAG="" ARCH="" TARGET="" OUT_DIR=""
BUILD_NUMBER="${GITHUB_RUN_NUMBER:-0}"
REQUIRE_SIGNING=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)   [[ $# -ge 2 ]] || usage; BINARY="$2"; shift 2 ;;
    --version)  [[ $# -ge 2 ]] || usage; VERSION="$2"; shift 2 ;;
    --tag)      [[ $# -ge 2 ]] || usage; TAG="$2"; shift 2 ;;
    --arch)     [[ $# -ge 2 ]] || usage; ARCH="$2"; shift 2 ;;
    --target)   [[ $# -ge 2 ]] || usage; TARGET="$2"; shift 2 ;;
    --out-dir)  [[ $# -ge 2 ]] || usage; OUT_DIR="$2"; shift 2 ;;
    --build-number) [[ $# -ge 2 ]] || usage; BUILD_NUMBER="$2"; shift 2 ;;
    --require-signing) REQUIRE_SIGNING=1; shift ;;
    *) usage ;;
  esac
done

[[ -n "$BINARY" && -n "$VERSION" && -n "$TAG" && -n "$ARCH" && -n "$TARGET" && -n "$OUT_DIR" ]] || usage
[[ -f "$BINARY" ]] || { echo "Binary not found: $BINARY" >&2; exit 1; }
[[ "$BUILD_NUMBER" =~ ^[0-9]+$ ]] || { echo "Invalid numeric --build-number: $BUILD_NUMBER" >&2; exit 2; }

APP_NAME="GitLab for Mac"
BUNDLE_ID="dev.gitlabformac.app"
BIN_NAME="gitlab-for-mac"
SHORT_VERSION="${VERSION%%-*}"
[[ "$SHORT_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Invalid SemVer version: $VERSION" >&2; exit 2; }
BUNDLE_VERSION="${SHORT_VERSION}.${BUILD_NUMBER}"
MIN_MACOS="${MACOSX_DEPLOYMENT_TARGET:-13.0}"

echo "==> Verifying binary architecture (${ARCH}), dynamic links, and minos..."
file "$BINARY"
actual_arch=$(lipo -archs "$BINARY")
[[ "$actual_arch" == "$ARCH" ]] || { echo "ERROR: Expected arch '${ARCH}', got '${actual_arch}'" >&2; exit 1; }

non_system_dylibs=$(otool -L "$BINARY" | tail -n +2 | awk '{print $1}' | grep -vE '^(/System/Library/|/usr/lib/)' || true)
if [[ -n "$non_system_dylibs" ]]; then
  echo "ERROR: Binary links disallowed non-system dynamic libraries:" >&2
  echo "$non_system_dylibs" >&2
  exit 1
fi

minos=$(otool -l "$BINARY" | awk '/minos[[:space:]]+/ { print $2; exit }')
if [[ -n "$minos" ]]; then
  echo "Detected Mach-O minos: ${minos} (target <= ${MIN_MACOS})"
  awk -v actual="$minos" -v max="$MIN_MACOS" 'BEGIN { exit !(actual + 0 <= max + 0) }' || {
    echo "ERROR: Binary minos (${minos}) exceeds LSMinimumSystemVersion (${MIN_MACOS})." >&2
    exit 1
  }
fi

if [[ "$(uname -m)" == "$ARCH" ]]; then
  echo "==> Smoke-testing binary execution (--version)..."
  ver_out=$("$BINARY" --version)
  echo "Binary reported: ${ver_out}"
  [[ "$ver_out" == "${BIN_NAME} "* ]] || { echo "ERROR: Unexpected --version output: ${ver_out}" >&2; exit 1; }
fi

mkdir -p "$OUT_DIR"
WORK_DIR=$(mktemp -d)
cleanup() { notary_cleanup; rm -rf "$WORK_DIR"; }
trap cleanup EXIT
notary_init

APP_DIR="${WORK_DIR}/${APP_NAME}.app"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "$BINARY" "${APP_DIR}/Contents/MacOS/${BIN_NAME}"
chmod 755 "${APP_DIR}/Contents/MacOS/${BIN_NAME}"

ICON_PLIST=""
if [[ -f "${REPO_ROOT}/assets/AppIcon.icns" ]]; then
  cp "${REPO_ROOT}/assets/AppIcon.icns" "${APP_DIR}/Contents/Resources/AppIcon.icns"
  ICON_PLIST=$'  <key>CFBundleIconFile</key>\n  <string>AppIcon</string>'
fi

cat > "${APP_DIR}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleExecutable</key>
  <string>${BIN_NAME}</string>
${ICON_PLIST}
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${SHORT_VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${BUNDLE_VERSION}</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key>
  <string>${MIN_MACOS}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright (c) GitLab for Mac Contributors (MIT License)</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
</dict>
</plist>
EOF
plutil -lint "${APP_DIR}/Contents/Info.plist"

# Bottom-up signing (never pass deprecated --deep when signing).
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "==> Code signing Mach-O and .app with Developer ID identity..."
  codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "${APP_DIR}/Contents/MacOS/${BIN_NAME}"
  codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$APP_DIR"
else
  echo "==> Code signing Mach-O and .app with ad-hoc signature..."
  codesign --force --sign - "${APP_DIR}/Contents/MacOS/${BIN_NAME}"
  codesign --force --sign - "$APP_DIR"
fi
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

if [[ "$NOTARY_MODE" != "none" ]]; then
  echo "==> Notarizing and stapling ${APP_NAME}.app..."
  ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "${WORK_DIR}/notary-app.zip"
  notary_submit "${WORK_DIR}/notary-app.zip"
  xcrun stapler staple "$APP_DIR"
  xcrun stapler validate "$APP_DIR"
  spctl --assess --type exec --verbose=4 "$APP_DIR"
fi

ZIP_NAME="GitLab-for-Mac-${TAG}-${ARCH}.app.zip"
DMG_NAME="GitLab-for-Mac-${TAG}-${ARCH}.dmg"
TAR_NAME="${BIN_NAME}-${TAG}-${TARGET}.tar.gz"

ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "${OUT_DIR}/${ZIP_NAME}"

DMG_STAGE="${WORK_DIR}/dmg-stage"
mkdir -p "$DMG_STAGE"
ditto "$APP_DIR" "${DMG_STAGE}/${APP_NAME}.app"
ln -s /Applications "${DMG_STAGE}/Applications"
dmg_created=0
for attempt in 1 2 3; do
  if hdiutil create -volname "$APP_NAME" -srcfolder "$DMG_STAGE" -ov -format UDZO "${OUT_DIR}/${DMG_NAME}"; then
    dmg_created=1; break
  fi
  echo "hdiutil create attempt ${attempt} failed; retrying in 5s..." >&2
  sleep 5
done
(( dmg_created == 1 )) || { echo "ERROR: Failed to create ${DMG_NAME} after 3 attempts." >&2; exit 1; }

if [[ "$NOTARY_MODE" != "none" ]]; then
  echo "==> Signing, notarizing, and stapling ${DMG_NAME}..."
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "${OUT_DIR}/${DMG_NAME}"
  notary_submit "${OUT_DIR}/${DMG_NAME}"
  xcrun stapler staple "${OUT_DIR}/${DMG_NAME}"
  xcrun stapler validate "${OUT_DIR}/${DMG_NAME}"
fi

TAR_STAGE="${WORK_DIR}/${BIN_NAME}-${TAG}-${TARGET}"
mkdir -p "$TAR_STAGE"
cp "${APP_DIR}/Contents/MacOS/${BIN_NAME}" "${TAR_STAGE}/${BIN_NAME}"
chmod 755 "${TAR_STAGE}/${BIN_NAME}"
cp "${REPO_ROOT}/LICENSE" "${REPO_ROOT}/README.md" "$TAR_STAGE/"
tar -C "$WORK_DIR" -czf "${OUT_DIR}/${TAR_NAME}" "${BIN_NAME}-${TAG}-${TARGET}"

(
  cd "$OUT_DIR"
  shasum -a 256 "$ZIP_NAME" "$DMG_NAME" "$TAR_NAME" > "SHA256SUMS-${ARCH}.txt"
  shasum -a 256 -c "SHA256SUMS-${ARCH}.txt"
)
ls -lh "$OUT_DIR"
