#!/usr/bin/env bash
set -euo pipefail

# Fetch the Chromium the app ships: Chrome for Testing's chrome-headless-shell
# (macOS arm64), the headless browser behind each tab's browser panel
# (desktop/docs/architecture/browser.md). The app never uses an installed
# browser, so a build needs this folder: desktop/src-tauri/resources/chromium/
# (generated and gitignored, bundled by tauri.conf.json).
#
# The URL and the SHA-256 are pinned; the cached zip is verified before every
# unpack and a mismatch stops the script. Idempotent: it does nothing when
# the folder already holds this exact version. The binaries are already
# (ad-hoc, linker-)signed by Google's build, and the app bundle seals them as
# resources, so nothing is re-signed here.
#
# Usage: scripts/fetch-chromium.sh
# Requires: curl, unzip, shasum.

VERSION="156.0.8078.80"
PLATFORM="mac-arm64"
NAME="chrome-headless-shell-${PLATFORM}"
URL="https://storage.googleapis.com/chrome-for-testing-public/${VERSION}/${PLATFORM}/${NAME}.zip"
SHA256="ea615baa2e9caf0d02d1898a6562b5f8fa9c35239707dae0f592178df5b986ac"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/desktop/src-tauri/resources/chromium"
CACHE="${ROOT}/.cache/chromium-downloads"
STAMP="${DEST}/.version"
WANT="${VERSION} ${SHA256}"

if [[ -x "${DEST}/${NAME}/chrome-headless-shell" && -f "${STAMP}" && "$(cat "${STAMP}")" == "${WANT}" ]]; then
  echo "chromium ${VERSION}: already in place"
  exit 0
fi

sha() { shasum -a 256 "$1" | awk '{print $1}'; }

mkdir -p "${CACHE}"
ZIP="${CACHE}/${NAME}-${VERSION}.zip"
if [[ ! -f "${ZIP}" || "$(sha "${ZIP}")" != "${SHA256}" ]]; then
  rm -f "${ZIP}"
  echo "downloading ${URL}"
  curl --fail --silent --show-error --location -o "${ZIP}.part" "${URL}"
  mv "${ZIP}.part" "${ZIP}"
fi
if [[ "$(sha "${ZIP}")" != "${SHA256}" ]]; then
  echo "SHA-256 mismatch for ${ZIP}: refusing to use it" >&2
  rm -f "${ZIP}"
  exit 1
fi

WORK="$(mktemp -d /tmp/npi-chromium.XXXXXX)"
trap 'rm -rf "${WORK}"' EXIT
unzip -q "${ZIP}" -d "${WORK}"
[[ -x "${WORK}/${NAME}/chrome-headless-shell" ]] || { echo "unexpected archive layout" >&2; exit 1; }
rm -rf "${DEST}"
mkdir -p "${DEST}"
mv "${WORK}/${NAME}" "${DEST}/${NAME}"
printf '%s' "${WANT}" > "${STAMP}"
"${DEST}/${NAME}/chrome-headless-shell" --version
