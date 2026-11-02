#!/usr/bin/env bash
set -euo pipefail

# Build the embedded Python runtime shipped in the desktop app's resources.
#
# Components: CPython + stdlib + pip (python-build-standalone) and uv. The
# archive is relocatable, contains its own licenses, and is reproducible:
# pinned upstream URLs and SHA-256 values, normalized ownership and mtimes,
# a sorted member list, and a gzip header with no name or timestamp.
#
# Usage: scripts/build-python-runtime.sh
# Requires: curl, tar (bsdtar or GNU tar), gzip, shasum/sha256sum, codesign on
# macOS. Downloads are cached under .cache/runtime-downloads/ and every cached
# file is verified against the pinned digest before use.

PYTHON_VERSION="3.12.16"
PYTHON_BUILD_RELEASE="20261026"
PLATFORM="aarch64-apple-darwin"
UV_VERSION="0.13.6"
TCLTK_VERSION="9.0.4"

CPYTHON_BUILD="cpython-${PYTHON_VERSION}+${PYTHON_BUILD_RELEASE}-${PLATFORM}-install_only"
PYTHON_TAR_NAME="${CPYTHON_BUILD}.tar.gz"
PYTHON_URL="https://github.com/astral-sh/python-build-standalone/releases/download/${PYTHON_BUILD_RELEASE}/${PYTHON_TAR_NAME}"
PYTHON_SHA256="fbf4afef53c2a6b72bb741bc06d5671a9dfaf87bd84e5aa73ac9b3d0d589f14e"

UV_TAR_NAME="uv-${PLATFORM}.tar.gz"
UV_URL="https://github.com/astral-sh/uv/releases/download/${UV_VERSION}/${UV_TAR_NAME}"
UV_SHA256="60fbd5c8a1ed681845eea28d9d622fcde03c51e91a43465b64a1e17db1ed3472"

# Licensing texts pinned to the exact upstream tags. The CPython license is
# copied from the runtime archive itself; these files add the notices that the
# install_only archive does not carry (CPython's incorporated-software notices,
# uv's dual license, and the Tcl/Tk terms for the bundled 9.0.4 runtime).
UV_LICENSE_APACHE_URL="https://raw.githubusercontent.com/astral-sh/uv/${UV_VERSION}/LICENSE-APACHE"
UV_LICENSE_APACHE_SHA256="c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4"
UV_LICENSE_MIT_URL="https://raw.githubusercontent.com/astral-sh/uv/${UV_VERSION}/LICENSE-MIT"
UV_LICENSE_MIT_SHA256="860e3d7a86b84e6a7012c7a635fc64df475cebc6cce34dfeb73a5982ec58176c"

CPYTHON_NOTICES_URL="https://raw.githubusercontent.com/python/cpython/v${PYTHON_VERSION}/Doc/license.rst"
CPYTHON_NOTICES_SHA256="341832873fd316a37927e79385093fbbfd40a467428480835fe435a80cadf4e5"

TCL_LICENSE_URL="https://raw.githubusercontent.com/tcltk/tcl/core-9-0-4/license.terms"
TCL_LICENSE_SHA256="c0a69a2bfd757361ec7e6143973b103c90409316b49e9c88db26ad6388e79f16"
TK_LICENSE_URL="https://raw.githubusercontent.com/tcltk/tk/core-9-0-4/license.terms"
TK_LICENSE_SHA256="2cde822b93ca16ae535c954b7dfe658b4ad10df2a193628d1b358f1765e8b198"

# 2026-10-26T00:00:00Z, the upstream CPython build release date.
SOURCE_DATE_EPOCH="1792972800"
TOUCH_STAMP="202610260000.00"

ARCHIVE_NAME="python-runtime-${PYTHON_VERSION}-${PLATFORM}.tar.gz"
MANIFEST_NAME="python-runtime-manifest.json"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RESOURCES_DIR="${REPO_ROOT}/desktop/src-tauri/resources"
CACHE_DIR="${REPO_ROOT}/.cache/runtime-downloads"
LICENSE_CACHE_DIR="${CACHE_DIR}/licenses"
WORK_DIR="$(mktemp -d /tmp/npi-python-build.XXXXXX)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

STAGING_DIR="${WORK_DIR}/staging"
STAGING_PYTHON="${STAGING_DIR}/python"
FILE_LIST="${WORK_DIR}/filelist.txt"

mkdir -p "${RESOURCES_DIR}" "${CACHE_DIR}" "${LICENSE_CACHE_DIR}"

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    echo "Error: neither shasum nor sha256sum is available" >&2
    exit 1
  fi
}

verify_sha256() {
  local file="$1"
  local expected="$2"
  local actual
  actual="$(sha256_of "${file}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "Checksum mismatch for ${file}!" >&2
    echo "  Expected: ${expected}" >&2
    echo "  Actual:   ${actual}" >&2
    return 1
  fi
}

fetch_pinned() {
  local url="$1"
  local dest="$2"
  local expected_sha="$3"

  if [[ -f "${dest}" ]]; then
    if verify_sha256 "${dest}" "${expected_sha}"; then
      echo "cache hit: $(basename "${dest}")"
      return 0
    fi
    echo "cached $(basename "${dest}") is corrupt; re-downloading" >&2
    rm -f "${dest}"
  fi

  echo "downloading ${url}"
  curl -fSL --proto '=https' --retry 3 --retry-delay 2 "${url}" -o "${dest}.tmp"
  verify_sha256 "${dest}.tmp" "${expected_sha}"
  mv "${dest}.tmp" "${dest}"
}

echo "=== 1/6 Fetch pinned upstream artifacts ==="
PYTHON_ARCHIVE="${CACHE_DIR}/${PYTHON_TAR_NAME}"
UV_ARCHIVE="${CACHE_DIR}/${UV_TAR_NAME}"
UV_LICENSE_APACHE="${LICENSE_CACHE_DIR}/uv-LICENSE-APACHE"
UV_LICENSE_MIT="${LICENSE_CACHE_DIR}/uv-LICENSE-MIT"
CPYTHON_NOTICES="${LICENSE_CACHE_DIR}/cpython-license.rst"
TCL_LICENSE="${LICENSE_CACHE_DIR}/tcl-license.terms"
TK_LICENSE="${LICENSE_CACHE_DIR}/tk-license.terms"

fetch_pinned "${PYTHON_URL}" "${PYTHON_ARCHIVE}" "${PYTHON_SHA256}"
fetch_pinned "${UV_URL}" "${UV_ARCHIVE}" "${UV_SHA256}"
fetch_pinned "${UV_LICENSE_APACHE_URL}" "${UV_LICENSE_APACHE}" "${UV_LICENSE_APACHE_SHA256}"
fetch_pinned "${UV_LICENSE_MIT_URL}" "${UV_LICENSE_MIT}" "${UV_LICENSE_MIT_SHA256}"
fetch_pinned "${CPYTHON_NOTICES_URL}" "${CPYTHON_NOTICES}" "${CPYTHON_NOTICES_SHA256}"
fetch_pinned "${TCL_LICENSE_URL}" "${TCL_LICENSE}" "${TCL_LICENSE_SHA256}"
fetch_pinned "${TK_LICENSE_URL}" "${TK_LICENSE}" "${TK_LICENSE_SHA256}"

echo "=== 2/6 Extract and stage the runtime ==="
mkdir -p "${STAGING_DIR}" "${WORK_DIR}/uv"
tar -xzf "${PYTHON_ARCHIVE}" -C "${STAGING_DIR}"
tar -xzf "${UV_ARCHIVE}" -C "${WORK_DIR}/uv"

for required in \
  "${STAGING_PYTHON}/bin/python3" \
  "${STAGING_PYTHON}/lib/python3.12/ensurepip" \
  "${STAGING_PYTHON}/lib/python3.12/site-packages/pip" \
  "${STAGING_PYTHON}/lib/python3.12/LICENSE.txt" \
  "${WORK_DIR}/uv/uv-${PLATFORM}/uv"; do
  if [[ ! -e "${required}" ]]; then
    echo "upstream archive is missing ${required}" >&2
    exit 1
  fi
done

install -m 0755 "${WORK_DIR}/uv/uv-${PLATFORM}/uv" "${STAGING_PYTHON}/bin/uv"

# The archive carries CPython's license and pip's vendored notices; add the
# notices it does not carry, under one discoverable directory.
LICENSE_DIR="${STAGING_PYTHON}/licenses"
mkdir -p "${LICENSE_DIR}/uv" "${LICENSE_DIR}/tcl" "${LICENSE_DIR}/tk"
cp "${STAGING_PYTHON}/lib/python3.12/LICENSE.txt" "${LICENSE_DIR}/CPYTHON-LICENSE.txt"
cp "${CPYTHON_NOTICES}" "${LICENSE_DIR}/CPYTHON-THIRD-PARTY-NOTICES.rst"
cp "${UV_LICENSE_APACHE}" "${LICENSE_DIR}/uv/LICENSE-APACHE"
cp "${UV_LICENSE_MIT}" "${LICENSE_DIR}/uv/LICENSE-MIT"
cp "${TCL_LICENSE}" "${LICENSE_DIR}/tcl/license.terms"
cp "${TK_LICENSE}" "${LICENSE_DIR}/tk/license.terms"

cat > "${LICENSE_DIR}/THIRD-PARTY-NOTICES.txt" <<EOF
NPi embedded Python runtime
===========================

Included components and license locations:

- CPython ${PYTHON_VERSION} (${CPYTHON_BUILD})
  License: PSF-2.0. See licenses/CPYTHON-LICENSE.txt and
  licenses/CPYTHON-THIRD-PARTY-NOTICES.rst (incorporated software).
  Upstream: ${PYTHON_URL}

- pip (bundled by CPython, with its vendored dependencies)
  License: MIT and the vendored dependency licenses. See
  lib/python3.12/site-packages/pip-26.2.2.dist-info/licenses/.

- uv ${UV_VERSION}
  License: Apache-2.0 OR MIT. See licenses/uv/LICENSE-APACHE and
  licenses/uv/LICENSE-MIT.
  Upstream: ${UV_URL}

- Tcl ${TCLTK_VERSION} and Tk ${TCLTK_VERSION} (bundled by
  python-build-standalone)
  License: BSD-style. See licenses/tcl/license.terms and
  licenses/tk/license.terms.
  Upstream: ${TCL_LICENSE_URL} and ${TK_LICENSE_URL}

Provenance (SHA-256):
  ${PYTHON_TAR_NAME}: ${PYTHON_SHA256}
  ${UV_TAR_NAME}: ${UV_SHA256}
EOF

echo "=== 3/6 Normalize and cod-sign Mach-O binaries ==="
# The runtime is read-only after installation; normalize build noise and any
# timestamps so the archive is byte-for-byte reproducible.
find "${STAGING_PYTHON}" -name '.DS_Store' -delete
find "${STAGING_PYTHON}" -name '__pycache__' -prune -exec rm -rf {} +
find "${STAGING_PYTHON}" -name '*.pyc' -delete
find "${STAGING_PYTHON}" -depth -exec env TZ=UTC touch -h -t "${TOUCH_STAMP}" {} +

if [[ "$(uname -s)" == "Darwin" ]]; then
  command -v codesign >/dev/null 2>&1 || {
    echo "codesign is required on macOS" >&2
    exit 1
  }
  while IFS= read -r -d '' candidate; do
    case "$(file -b "${candidate}")" in
      Mach-O*)
        if ! codesign --verify --strict "${candidate}" >/dev/null 2>&1; then
          echo "ad-hoc signing ${candidate#${STAGING_PYTHON}/}"
          codesign --force --sign - "${candidate}"
        fi
        ;;
    esac
  done < <(find "${STAGING_PYTHON}" -type f -print0)

  # Signing changes bytes, so normalize again before packaging.
  find "${STAGING_PYTHON}" -depth -exec env TZ=UTC touch -h -t "${TOUCH_STAMP}" {} +
fi

echo "=== 4/6 Package deterministic archive ==="
(
  cd "${STAGING_DIR}"
  find python -print | LC_ALL=C sort > "${FILE_LIST}"
)

# ustar stores paths in 255 bytes and link targets in 100. Fail loudly rather
# than let a future runtime silently fall back to an extended format.
while IFS= read -r member; do
  if (( ${#member} > 255 )); then
    echo "path exceeds ustar's 255-byte limit: ${member}" >&2
    exit 1
  fi
done < "${FILE_LIST}"
while IFS= read -r -d '' link; do
  target="$(readlink "${link}")"
  if (( ${#target} > 100 )); then
    echo "link target exceeds ustar's 100-byte limit: ${link} -> ${target}" >&2
    exit 1
  fi
done < <(find "${STAGING_PYTHON}" -type l -print0)

package_archive() {
  local output="$1"
  (
    cd "${STAGING_DIR}"
    COPYFILE_DISABLE=1 LC_ALL=C tar \
      --format ustar \
      --uid 0 --gid 0 --uname root --gname wheel \
      --no-recursion \
      -cf - -T "${FILE_LIST}" | gzip -9 -n > "${output}"
  )
}

TARGET_ARCHIVE="${RESOURCES_DIR}/${ARCHIVE_NAME}"
VERIFY_ARCHIVE="${WORK_DIR}/reproducible.tar.gz"
package_archive "${TARGET_ARCHIVE}.tmp"
package_archive "${VERIFY_ARCHIVE}"
if ! cmp -s "${TARGET_ARCHIVE}.tmp" "${VERIFY_ARCHIVE}"; then
  echo "archive is not reproducible: two builds differ" >&2
  exit 1
fi
mv "${TARGET_ARCHIVE}.tmp" "${TARGET_ARCHIVE}"
echo "reproducible: two builds are byte-identical"

echo "=== 5/6 Verify the packaged archive ==="
VERIFY_DIR="${WORK_DIR}/verify"
mkdir -p "${VERIFY_DIR}"
tar -xzf "${TARGET_ARCHIVE}" -C "${VERIFY_DIR}"

for required in \
  "${VERIFY_DIR}/python/bin/python3" \
  "${VERIFY_DIR}/python/bin/uv" \
  "${VERIFY_DIR}/python/bin/pip" \
  "${VERIFY_DIR}/python/licenses/uv/LICENSE-APACHE" \
  "${VERIFY_DIR}/python/licenses/uv/LICENSE-MIT" \
  "${VERIFY_DIR}/python/licenses/CPYTHON-LICENSE.txt"; do
  if [[ ! -e "${required}" ]]; then
    echo "packaged archive is missing ${required}" >&2
    exit 1
  fi
done

"${VERIFY_DIR}/python/bin/python3" --version
"${VERIFY_DIR}/python/bin/python3" -m pip --version
"${VERIFY_DIR}/python/bin/uv" --version

if [[ "$(uname -s)" == "Darwin" ]]; then
  for binary in "${VERIFY_DIR}/python/bin/python3.12" "${VERIFY_DIR}/python/bin/uv"; do
    codesign --verify --strict "${binary}"
  done
fi

ARCHIVE_SHA256="$(sha256_of "${TARGET_ARCHIVE}")"
ARCHIVE_UNCOMPRESSED_SIZE="$(gzip -dc "${TARGET_ARCHIVE}" | wc -c | tr -d ' ')"
if [[ "$(uname -s)" == "Darwin" ]]; then
  ARCHIVE_SIZE="$(stat -f%z "${TARGET_ARCHIVE}")"
else
  ARCHIVE_SIZE="$(stat -c%s "${TARGET_ARCHIVE}")"
fi

echo "=== 6/6 Write the bundle manifest ==="
TARGET_MANIFEST="${RESOURCES_DIR}/${MANIFEST_NAME}"
cat > "${TARGET_MANIFEST}.tmp" <<EOF
{
  "runtimeVersion": "${PYTHON_VERSION}",
  "platform": "${PLATFORM}",
  "archiveName": "${ARCHIVE_NAME}",
  "archiveSha256": "${ARCHIVE_SHA256}",
  "archiveSize": ${ARCHIVE_SIZE},
  "archiveUncompressedSize": ${ARCHIVE_UNCOMPRESSED_SIZE},
  "uvVersion": "${UV_VERSION}",
  "cpythonBuild": "${CPYTHON_BUILD}",
  "components": ["cpython", "stdlib", "pip", "uv"],
  "upstream": {
    "cpython": {
      "url": "${PYTHON_URL}",
      "sha256": "${PYTHON_SHA256}",
      "license": "PSF-2.0"
    },
    "uv": {
      "version": "${UV_VERSION}",
      "url": "${UV_URL}",
      "sha256": "${UV_SHA256}",
      "license": ["Apache-2.0", "MIT"]
    },
    "tclTk": {
      "version": "${TCLTK_VERSION}",
      "tclLicenseUrl": "${TCL_LICENSE_URL}",
      "tkLicenseUrl": "${TK_LICENSE_URL}"
    }
  },
  "licenses": {
    "cpython": "python/licenses/CPYTHON-LICENSE.txt",
    "cpythonThirdParty": "python/licenses/CPYTHON-THIRD-PARTY-NOTICES.rst",
    "pip": "python/lib/python3.12/site-packages/pip-26.2.2.dist-info/licenses/",
    "uvApache": "python/licenses/uv/LICENSE-APACHE",
    "uvMit": "python/licenses/uv/LICENSE-MIT",
    "tcl": "python/licenses/tcl/license.terms",
    "tk": "python/licenses/tk/license.terms",
    "notices": "python/licenses/THIRD-PARTY-NOTICES.txt"
  },
  "licenseSources": {
    "cpythonThirdParty": {
      "url": "${CPYTHON_NOTICES_URL}",
      "sha256": "${CPYTHON_NOTICES_SHA256}"
    },
    "uvApache": {
      "url": "${UV_LICENSE_APACHE_URL}",
      "sha256": "${UV_LICENSE_APACHE_SHA256}"
    },
    "uvMit": {
      "url": "${UV_LICENSE_MIT_URL}",
      "sha256": "${UV_LICENSE_MIT_SHA256}"
    },
    "tcl": {
      "url": "${TCL_LICENSE_URL}",
      "sha256": "${TCL_LICENSE_SHA256}"
    },
    "tk": {
      "url": "${TK_LICENSE_URL}",
      "sha256": "${TK_LICENSE_SHA256}"
    }
  },
  "build": {
    "sourceDateEpoch": ${SOURCE_DATE_EPOCH},
    "tarFormat": "ustar",
    "gzipFlags": "-9 -n",
    "owner": "0:0",
    "script": "scripts/build-python-runtime.sh"
  }
}
EOF
mv "${TARGET_MANIFEST}.tmp" "${TARGET_MANIFEST}"

echo ""
echo "archive:  ${TARGET_ARCHIVE}"
echo "size:     ${ARCHIVE_SIZE} bytes compressed, ${ARCHIVE_UNCOMPRESSED_SIZE} bytes uncompressed"
echo "sha256:   ${ARCHIVE_SHA256}"
echo "manifest: ${TARGET_MANIFEST}"
