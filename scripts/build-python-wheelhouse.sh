#!/usr/bin/env bash
set -euo pipefail

# Build the offline wheelhouse of the scientific suite shipped next to the
# embedded Python runtime (numpy, scipy, matplotlib, pandas, sympy and their
# dependencies).
#
# The runtime host installs it into the managed environment with
# `uv pip install --offline --no-index --require-hashes`, so the app never
# reaches an index for these packages. Every wheel is pinned by version and
# SHA-256 in scripts/python-scientific.lock; the archive is reproducible
# (sorted members, normalized owner and mtimes, gzip with no name or time).
#
# Usage:
#   scripts/build-python-wheelhouse.sh            build from the committed lock
#   scripts/build-python-wheelhouse.sh --relock   re-resolve python-scientific.in
#                                                 into the lock first
# Requires: the runtime archive from scripts/build-python-runtime.sh, curl
# access to PyPI for wheels missing from .cache/runtime-downloads/wheels/,
# tar, gzip, shasum/sha256sum.

PLATFORM="aarch64-apple-darwin"
PYTHON_MINOR="3.12"
# scipy publishes no Apple Silicon wheel older than macOS 12.
MACOS_TARGET="12.0"
PIP_PLATFORM="macosx_12_0_arm64"
SUITE="scientific"

# 2026-10-26T00:00:00Z, the same stamp as the runtime archive.
TOUCH_STAMP="202610260000.00"

ARCHIVE_NAME="python-runtime-wheels-${SUITE}-${PLATFORM}.tar.gz"
MANIFEST_NAME="python-runtime-wheels-manifest.json"
RUNTIME_MANIFEST_NAME="python-runtime-manifest.json"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RESOURCES_DIR="${REPO_ROOT}/desktop/src-tauri/resources"
REQUIREMENTS_IN="${SCRIPT_DIR}/python-${SUITE}.in"
LOCK_FILE="${SCRIPT_DIR}/python-${SUITE}.lock"
WHEEL_CACHE="${REPO_ROOT}/.cache/runtime-downloads/wheels"
WORK_DIR="$(mktemp -d /tmp/npi-python-wheels.XXXXXX)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

RELOCK=0
for arg in "$@"; do
  case "${arg}" in
    --relock) RELOCK=1 ;;
    *)
      echo "unknown argument: ${arg}" >&2
      exit 2
      ;;
  esac
done

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

size_of() {
  if [[ "$(uname -s)" == "Darwin" ]]; then
    stat -f%z "$1"
  else
    stat -c%s "$1"
  fi
}

# A string field of the runtime manifest, read without a JSON tool: the
# manifest is written by build-python-runtime.sh with one key per line.
runtime_field() {
  sed -n "s/^  \"$1\": \"\\(.*\\)\",\$/\\1/p" "${RESOURCES_DIR}/${RUNTIME_MANIFEST_NAME}" | head -n 1
}

echo "=== 1/6 Unpack the bundled runtime as the build tool ==="
RUNTIME_ARCHIVE="${RESOURCES_DIR}/$(runtime_field archiveName)"
RUNTIME_SHA256="$(runtime_field archiveSha256)"
if [[ ! -f "${RUNTIME_ARCHIVE}" ]]; then
  echo "missing ${RUNTIME_ARCHIVE}; run scripts/build-python-runtime.sh first" >&2
  exit 1
fi
if [[ "$(sha256_of "${RUNTIME_ARCHIVE}")" != "${RUNTIME_SHA256}" ]]; then
  echo "${RUNTIME_ARCHIVE} does not match ${RUNTIME_MANIFEST_NAME}; rebuild the runtime" >&2
  exit 1
fi
tar -xzf "${RUNTIME_ARCHIVE}" -C "${WORK_DIR}"
TOOL_PYTHON="${WORK_DIR}/python/bin/python3"
TOOL_UV="${WORK_DIR}/python/bin/uv"
export UV_CACHE_DIR="${WORK_DIR}/uv-cache"
export PIP_CACHE_DIR="${WORK_DIR}/pip-cache"
export PYTHONDONTWRITEBYTECODE=1

echo "=== 2/6 Resolve the lock ==="
if [[ "${RELOCK}" == 1 || ! -f "${LOCK_FILE}" ]]; then
  MACOSX_DEPLOYMENT_TARGET="${MACOS_TARGET}" "${TOOL_UV}" pip compile "${REQUIREMENTS_IN}" \
    --python-version "${PYTHON_MINOR}" \
    --python-platform "${PLATFORM}" \
    --only-binary ':all:' \
    --generate-hashes \
    --custom-compile-command "scripts/build-python-wheelhouse.sh --relock" \
    --output-file "${LOCK_FILE}"
  echo "wrote ${LOCK_FILE}"
else
  echo "using ${LOCK_FILE}"
fi

echo "=== 3/6 Fetch the pinned wheels ==="
mkdir -p "${WHEEL_CACHE}"
# --require-hashes rejects any file whose digest is not in the lock, so a
# cached or downloaded wheel is verified before it is used.
"${TOOL_PYTHON}" -I -m pip download \
  --require-hashes \
  --only-binary=:all: \
  --no-deps \
  --platform "${PIP_PLATFORM}" \
  --implementation cp \
  --python-version "${PYTHON_MINOR}" \
  --abi "cp${PYTHON_MINOR/./}" \
  --dest "${WHEEL_CACHE}" \
  --requirement "${LOCK_FILE}" \
  --disable-pip-version-check \
  --quiet

echo "=== 4/6 Stage and package the wheelhouse ==="
STAGING_DIR="${WORK_DIR}/staging"
WHEELHOUSE="${STAGING_DIR}/wheelhouse"
mkdir -p "${WHEELHOUSE}/wheels"
cp "${LOCK_FILE}" "${WHEELHOUSE}/requirements.lock"

# Exactly the wheels the lock names: `name==version` lines, matched against
# wheel file names by their normalized distribution name and version.
PINS="${WORK_DIR}/pins.txt"
sed -n 's/^\([A-Za-z0-9._-]*\)==\([^ ;\\]*\).*/\1 \2/p' "${LOCK_FILE}" > "${PINS}"
while read -r name version; do
  normalized="$(printf '%s' "${name}" | tr '[:upper:]' '[:lower:]' | sed 's/[-_.]\{1,\}/_/g')"
  match=""
  for wheel in "${WHEEL_CACHE}"/*.whl; do
    base="$(basename "${wheel}")"
    wheel_name="$(printf '%s' "${base%%-*}" | tr '[:upper:]' '[:lower:]' | sed 's/[-_.]\{1,\}/_/g')"
    rest="${base#*-}"
    wheel_version="${rest%%-*}"
    if [[ "${wheel_name}" == "${normalized}" && "${wheel_version}" == "${version}" ]]; then
      match="${wheel}"
      cp "${wheel}" "${WHEELHOUSE}/wheels/"
    fi
  done
  if [[ -z "${match}" ]]; then
    echo "no downloaded wheel for ${name}==${version}" >&2
    exit 1
  fi
done < "${PINS}"

cat > "${WHEELHOUSE}/THIRD-PARTY-NOTICES.txt" <<EOF
NPi embedded Python runtime - ${SUITE} suite
=============================================

Wheels installed offline into the managed environment, pinned by version and
SHA-256 in requirements.lock. Each distribution carries its own license in its
installed *.dist-info directory (numpy, scipy, pandas, sympy, matplotlib and
their dependencies use BSD, MIT, PSF-style, Apache-2.0 or HPND licenses).

Pinned distributions:
$(sed 's/^/  /' "${PINS}")
EOF

find "${STAGING_DIR}" -name '.DS_Store' -delete
find "${STAGING_DIR}" -depth -exec env TZ=UTC touch -h -t "${TOUCH_STAMP}" {} +
FILE_LIST="${WORK_DIR}/filelist.txt"
(
  cd "${STAGING_DIR}"
  find wheelhouse -print | LC_ALL=C sort > "${FILE_LIST}"
)
while IFS= read -r member; do
  if (( ${#member} > 255 )); then
    echo "path exceeds ustar's 255-byte limit: ${member}" >&2
    exit 1
  fi
done < "${FILE_LIST}"

package_archive() {
  (
    cd "${STAGING_DIR}"
    COPYFILE_DISABLE=1 LC_ALL=C tar \
      --format ustar \
      --uid 0 --gid 0 --uname root --gname wheel \
      --no-recursion \
      -cf - -T "${FILE_LIST}" | gzip -9 -n > "$1"
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

echo "=== 5/6 Verify an offline install from the packaged archive ==="
VERIFY_DIR="${WORK_DIR}/verify"
mkdir -p "${VERIFY_DIR}"
tar -xzf "${TARGET_ARCHIVE}" -C "${VERIFY_DIR}"
"${TOOL_PYTHON}" -I -B -m venv --without-pip "${VERIFY_DIR}/venv"
"${TOOL_UV}" pip install \
  --python "${VERIFY_DIR}/venv/bin/python" \
  --offline --no-index \
  --find-links "${VERIFY_DIR}/wheelhouse/wheels" \
  --require-hashes \
  --requirement "${VERIFY_DIR}/wheelhouse/requirements.lock" \
  --quiet
MPLBACKEND=Agg MPLCONFIGDIR="${WORK_DIR}/mpl" "${VERIFY_DIR}/venv/bin/python" -I -B -c '
import numpy, scipy, scipy.linalg, matplotlib, matplotlib.pyplot as plt, pandas, sympy
plt.plot(numpy.arange(3)); plt.savefig("/dev/null", format="png")
assert abs(scipy.linalg.det(numpy.eye(3)) - 1.0) < 1e-12
print("numpy", numpy.__version__, "| scipy", scipy.__version__, "| matplotlib", matplotlib.__version__,
      "| pandas", pandas.__version__, "| sympy", sympy.__version__)
'

echo "=== 6/6 Write the wheelhouse manifest ==="
ARCHIVE_SHA256="$(sha256_of "${TARGET_ARCHIVE}")"
ARCHIVE_SIZE="$(size_of "${TARGET_ARCHIVE}")"
PACKAGES_JSON="$(awk 'BEGIN{sep=""} {printf "%s\n    {\"name\": \"%s\", \"version\": \"%s\"}", sep, $1, $2; sep=","}' "${PINS}")"
TARGET_MANIFEST="${RESOURCES_DIR}/${MANIFEST_NAME}"
cat > "${TARGET_MANIFEST}.tmp" <<EOF
{
  "suite": "${SUITE}",
  "platform": "${PLATFORM}",
  "pythonVersion": "${PYTHON_MINOR}",
  "minimumMacos": "${MACOS_TARGET}",
  "archiveName": "${ARCHIVE_NAME}",
  "archiveSha256": "${ARCHIVE_SHA256}",
  "archiveSize": ${ARCHIVE_SIZE},
  "lockFile": "wheelhouse/requirements.lock",
  "wheelsDir": "wheelhouse/wheels",
  "packages": [${PACKAGES_JSON}
  ],
  "build": {
    "script": "scripts/build-python-wheelhouse.sh",
    "lock": "scripts/python-${SUITE}.lock"
  }
}
EOF
mv "${TARGET_MANIFEST}.tmp" "${TARGET_MANIFEST}"

echo ""
echo "archive:  ${TARGET_ARCHIVE}"
echo "size:     ${ARCHIVE_SIZE} bytes"
echo "sha256:   ${ARCHIVE_SHA256}"
echo "manifest: ${TARGET_MANIFEST}"
