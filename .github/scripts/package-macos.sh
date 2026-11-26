#!/usr/bin/env bash
set -euo pipefail

# Packages a compiled macOS release binary into:
#   1. GitLab-for-Mac-<tag>-<arch>.app.zip  (signed .app bundle)
#   2. GitLab-for-Mac-<tag>-<arch>.dmg      (drag-to-Applications disk image)
#   3. gitlab-for-mac-<tag>-<target>.tar.gz (standalone CLI archive)
#   4. SHA256SUMS-<arch>.txt                (verified SHA-256 manifest)

usage() {
  cat <<'EOF' >&2
Usage: package-macos.sh --binary <path> --version <semver> --tag <label> --arch <arm64|x86_64> --target <triple> --out-dir <dir>
EOF
  exit 2
}

BINARY=""
VERSION=""
TAG=""
ARCH=""
TARGET=""
OUT_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary) BINARY="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    --tag) TAG="$2"; shift 2 ;;
    --arch) ARCH="$2"; shift 2 ;;
    --target) TARGET="$2"; shift 2 ;;
    --out-dir) OUT_DIR="$2"; shift 2 ;;
    *) usage ;;
  esac
done

[[ -n "$BINARY" && -n "$VERSION" && -n "$TAG" && -n "$ARCH" && -n "$TARGET" && -n "$OUT_DIR" ]] || usage
[[ -f "$BINARY" ]] || { echo "Binary not found: $BINARY" >&2; exit 1; }

APP_NAME="GitLab for Mac"
BUNDLE_ID="dev.gitlabformac.app"
BIN_NAME="gitlab-for-mac"
SHORT_VERSION="${VERSION%%-*}"

echo "==> Verifying binary architecture (${ARCH}) and dynamic links..."
file "$BINARY"
actual_arch=$(lipo -archs "$BINARY")
if [[ "$actual_arch" != *"$ARCH"* ]]; then
  echo "ERROR: Expected architecture '${ARCH}', got '${actual_arch}'" >&2
  exit 1
fi

dylib_report=$(otool -L "$BINARY")
echo "$dylib_report"
if echo "$dylib_report" | grep -E '(/opt/homebrew/|/usr/local/|/opt/local/)' >/dev/null 2>&1; then
  echo "ERROR: Release binary links non-system local dynamic libraries!" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

APP_DIR="${WORK_DIR}/${APP_NAME}.app"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "$BINARY" "${APP_DIR}/Contents/MacOS/${BIN_NAME}"
chmod 755 "${APP_DIR}/Contents/MacOS/${BIN_NAME}"

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
  <string>${VERSION}</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
</dict>
</plist>
EOF

plutil -lint "${APP_DIR}/Contents/Info.plist"

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "==> Code signing app bundle with Developer ID identity..."
  codesign --force --deep --options runtime --timestamp \
    --sign "$APPLE_SIGNING_IDENTITY" "$APP_DIR"
else
  echo "==> Code signing app bundle with ad-hoc signature..."
  codesign --force --deep --sign - "$APP_DIR"
fi
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

ZIP_NAME="GitLab-for-Mac-${TAG}-${ARCH}.app.zip"
DMG_NAME="GitLab-for-Mac-${TAG}-${ARCH}.dmg"
TAR_NAME="${BIN_NAME}-${TAG}-${TARGET}.tar.gz"

echo "==> Creating ${ZIP_NAME}..."
ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "${OUT_DIR}/${ZIP_NAME}"

echo "==> Creating ${DMG_NAME}..."
DMG_STAGE="${WORK_DIR}/dmg-stage"
mkdir -p "$DMG_STAGE"
cp -R "$APP_DIR" "${DMG_STAGE}/"
ln -s /Applications "${DMG_STAGE}/Applications"
# Retry hdiutil to mitigate transient Spotlight "Resource busy" locks on macOS CI runners.
dmg_created=0
for attempt in 1 2 3; do
  if hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$DMG_STAGE" \
    -ov \
    -format UDZO \
    "${OUT_DIR}/${DMG_NAME}"; then
    dmg_created=1
    break
  fi
  echo "hdiutil create attempt ${attempt} failed; retrying in 5s..." >&2
  sleep 5
done
if (( dmg_created == 0 )); then
  echo "ERROR: Failed to create ${DMG_NAME} after 3 attempts." >&2
  exit 1
fi

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" && -n "${APPLE_ID:-}" && -n "${APPLE_TEAM_ID:-}" && -n "${APPLE_APP_PASSWORD:-}" ]]; then
  echo "==> Signing and notarizing ${DMG_NAME}..."
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "${OUT_DIR}/${DMG_NAME}"
  xcrun notarytool submit "${OUT_DIR}/${DMG_NAME}" \
    --apple-id "$APPLE_ID" \
    --team-id "$APPLE_TEAM_ID" \
    --password "$APPLE_APP_PASSWORD" \
    --wait
  xcrun stapler staple "${OUT_DIR}/${DMG_NAME}"
  xcrun stapler validate "${OUT_DIR}/${DMG_NAME}"
fi

echo "==> Creating ${TAR_NAME}..."
TAR_STAGE="${WORK_DIR}/${BIN_NAME}-${TAG}-${TARGET}"
mkdir -p "$TAR_STAGE"
cp "$BINARY" "${TAR_STAGE}/${BIN_NAME}"
chmod 755 "${TAR_STAGE}/${BIN_NAME}"
cp LICENSE README.md "$TAR_STAGE/"
tar -C "$WORK_DIR" -czf "${OUT_DIR}/${TAR_NAME}" "${BIN_NAME}-${TAG}-${TARGET}"

echo "==> Generating and verifying SHA-256 checksums..."
(
  cd "$OUT_DIR"
  shasum -a 256 "$ZIP_NAME" "$DMG_NAME" "$TAR_NAME" > "SHA256SUMS-${ARCH}.txt"
  shasum -a 256 -c "SHA256SUMS-${ARCH}.txt"
)

echo "==> Packaged artifacts in ${OUT_DIR}:"
ls -lh "$OUT_DIR"
