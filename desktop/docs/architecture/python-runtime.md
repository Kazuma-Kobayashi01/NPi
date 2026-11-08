# Embedded Python runtime

The desktop app ships one relocatable CPython distribution and owns every
mutation of it. The `omp` side never unpacks the archive, never creates a
virtual environment and never falls back to a system or project interpreter
when the desktop host is available. This document covers the bundle, the
on-disk layout, the install lifecycle, the `--npi-python-host` protocol and the
packaging contract.

Implementation files:

- `scripts/build-python-runtime.sh` - reproducible archive and manifest builder
- `scripts/check-python-runtime.mjs` - fail-closed pre-build archive check
- `scripts/smoke-python-runtime.mjs` - built-app host smoke test
- `src-tauri/resources/python-runtime-manifest.json` - the bundled descriptor
- `src-tauri/resources/.gitignore` - keeps the generated archive out of git
- `src-tauri/resources/python-runtime-3.12.16-aarch64-apple-darwin.tar.gz`
- `src-tauri/src/main.rs` - `--npi-python-host` reentry before Tauri starts
- `src-tauri/src/python_runtime/mod.rs` - `run_host`, resource resolution and
  the Tauri commands used by the settings pane
- `src-tauri/src/python_runtime/manifest.rs` - bundled and installed descriptors
- `src-tauri/src/python_runtime/installer.rs` - verification, extraction,
  content-addressed install and locking
- `src-tauri/src/python_runtime/environment.rs` - venv, packages and health
- `src-tauri/src/python_runtime/wheels.rs` - the bundled scientific suite
  (offline wheelhouse) installed into each environment version
- `scripts/build-python-wheelhouse.sh` - reproducible wheelhouse builder;
  `scripts/python-scientific.in` (top-level requirements) and
  `scripts/python-scientific.lock` (versions and SHA-256 of every wheel)
- `src-tauri/resources/python-runtime-wheels-manifest.json` - the bundled
  wheelhouse descriptor
- `src-tauri/src/python_runtime/paths.rs` - one resolver for every directory
- `src-tauri/src/agent/spawn.rs` - exports `NPI_PYTHON_HOST` to every `omp`
  child as the absolute desktop executable
- `tauri.conf.json` - `bundle.resources: ["resources/python-runtime-*"]` ships the manifest and
  archive under the app's `Contents/Resources/resources/` folder

## Component set

| Component | Version | Source | License |
|---|---|---|---|
| CPython + stdlib | 3.12.16 | `python-build-standalone` `cpython-3.12.16+20261026-aarch64-apple-darwin-install_only` | PSF-2.0 |
| pip | 26.2.2 (bundled by CPython, offline ensurepip wheel) | same archive | MIT + vendored licenses |
| uv | 0.13.6 | `astral-sh/uv` `uv-aarch64-apple-darwin.tar.gz` | Apache-2.0 OR MIT |
| Tcl/Tk | 9.0.4 (bundled by python-build-standalone) | same CPython archive | BSD-style |

The archive is macOS Apple Silicon only (`aarch64-apple-darwin`). The bundle
manifest identifies the platform; an incompatible host must fail rather than
run the wrong binaries.

## Current artifact

The values below are the artifact produced by the script. The archive itself
is gitignored; the manifest is committed and authoritative for the digest. Read
`python-runtime-manifest.json` for the current value at any time.

| Field | Value |
|---|---|
| Archive | `python-runtime-3.12.16-aarch64-apple-darwin.tar.gz` |
| Compressed size | 38,607,157 bytes |
| Uncompressed size | 95,436,800 bytes |
| SHA-256 | `3a713636157b8e54e978415f407d9e1fe8222e7259bcdd7e14e06f084232c532` |

Upstream digests pinned by the script:

| Artifact | SHA-256 |
|---|---|
| `cpython-3.12.16+20261026-aarch64-apple-darwin-install_only.tar.gz` | `fbf4afef53c2a6b72bb741bc06d5671a9dfaf87bd84e5aa73ac9b3d0d589f14e` |
| `uv-aarch64-apple-darwin.tar.gz` | `60fbd5c8a1ed681845eea28d9d622fcde03c51e91a43465b64a1e17db1ed3472` |

## Bundle layout

The archive has one top-level directory, `python/`, which becomes the runtime
directory itself, so an installed runtime always looks like:

```text
<runtime>/bin/python3          # symlink to python3.12
<runtime>/bin/python3.12
<runtime>/bin/pip              # ensurepip console script
<runtime>/bin/uv
<runtime>/lib/libpython3.12.dylib
<runtime>/lib/python3.12/      # stdlib, site-packages/pip, ensurepip wheel
<runtime>/licenses/            # notices added by the packaging script
```

Licenses shipped inside the archive:

- `licenses/CPYTHON-LICENSE.txt`
- `licenses/CPYTHON-THIRD-PARTY-NOTICES.rst` - CPython's incorporated-software
  notices, which the upstream `install_only` archive does not carry
- `licenses/uv/LICENSE-APACHE`, `licenses/uv/LICENSE-MIT`
- `licenses/tcl/license.terms`, `licenses/tk/license.terms`
- `licenses/THIRD-PARTY-NOTICES.txt` - component list, provenance and locations
- `lib/python3.12/site-packages/pip-26.2.2.dist-info/licenses/` - pip and its
  vendored dependencies, as published in the official CPython archive

## On-disk layout

The base directory is, in priority order, `NPI_ENV_DIR` when non-empty, then
`~/.local/share/npi/env`:

```text
<base>/
  runtimes/python/<runtimeVersion>-<platform>-<archiveSha12>/
  environments/default/
    current                      # pointer file: active environment version id
    versions/py<runtimeVersion>-<sha12>/bin/python
  environments/academic/         # reserved for an optional expansion bundle
  archives/                      # host-managed archives (reserved cache)
  cache/uv/
  workspaces/<session-id>/       # omp execution workspace
  state/install.lock
```

- Runtime directories are content-addressed by the archive digest prefix, so a
  new bundle never overwrites a runtime that a live session is using. Old
  runtime directories are not deleted during an upgrade.
- The default environment is versioned. `current` is an atomically rewritten
  pointer; an environment build that fails leaves the previous pointer intact.
- Both runtime installation and environment creation serialize on
  `<base>/state/install.lock`, so two windows or two processes converge on one
  install.
- The runtime is immutable after installation. Packages go into the default
  environment; `uv` writes its downloads and build cache under
  `<base>/cache/uv`.

## Install lifecycle

1. Read `python-runtime-manifest.json` from the resource directory (or its
   `resources/` subdirectory, where Tauri may place bundle resources).
2. If the content-addressed runtime and its installed-state record already
   match the manifest, reuse it; no archive read and no network.
3. Otherwise verify the archive's size and SHA-256 against the manifest.
4. Extract into a hidden staging directory next to the target, then run the
   packaged `python3` and `uv` once. Only a successful probe is renamed into
   the content-addressed runtime path.
5. Create the default environment offline with the runtime's own
   `python3 -I -B -m venv --without-pip` and seed pip with
   `-I -B -m ensurepip --upgrade --default-pip` (the wheel is bundled).
6. When the app ships `python-runtime-wheels-manifest.json`, install the
   scientific suite into the new version offline (see below), then verify the
   version and atomically switch `current`.
7. Report health by reading the interpreter and `uv` versions. A timeout,
   crash, digest mismatch, or missing binary is an error; nothing falls back to
   another interpreter.

An interrupted install can leave a `.staging-*` directory, but never a runtime
that later runs look healthy: the runtime is only visible after the atomic
rename, and the installed-state record names the exact digest.

## Scientific suite (offline wheelhouse)

numpy, scipy, matplotlib, pandas and sympy, with their dependencies (15
wheels), ship as a second archive next to the runtime:
`python-runtime-wheels-scientific-aarch64-apple-darwin.tar.gz`, about 76 MB,
described by `python-runtime-wheels-manifest.json` (suite, platform, archive
name/size/SHA-256, the lock and wheels folder inside it, the pinned package
list). Installed, the suite takes about 265 MB in the environment.

- The suite is part of the environment's identity: the version id becomes
  `py<runtimeVersion>-<runtimeSha12>-scientific-<wheelhouseSha12>`, so a new pin
  set builds a new version beside the old one and flips `current` only after it
  is complete. Packages a user installed into an older version stay there.
- `wheels.rs` verifies the archive's size and SHA-256, unpacks it into
  `<base>/archives/.wheels-*`, and runs the runtime's
  `uv pip install --offline --no-index --require-hashes --find-links <wheels>
  --requirement <lock>`. No index is contacted, and a wheel whose digest is not
  in the lock is refused. The staging directory is removed afterwards.
- A `.npi-wheelhouse` marker in the version directory records the archive
  digest; a version without it (a build that died mid-install) is rebuilt, not
  published.
- An app without the wheelhouse manifest builds the environment with pip only.
  A manifest whose archive is missing or altered, or that names another
  platform than the runtime, fails `ensure`, like the runtime archive.
- The wheels target macOS 12+ (scipy publishes no older Apple Silicon wheel).
- Each distribution keeps its license in its installed `*.dist-info`; the
  archive adds `wheelhouse/THIRD-PARTY-NOTICES.txt` listing the pins.

## Host protocol

The desktop executable is also a one-shot JSON host. `main.rs` intercepts
`--npi-python-host` before Tauri starts, reads exactly one JSON request from
stdin, writes exactly one JSON value to stdout and exits 0.

```bash
/Applications/NPi\ Desktop.app/Contents/MacOS/NPi\ Desktop --npi-python-host
```

Requests (camelCase; unknown fields are rejected):

```json
{"action": "ensure", "resourceDir": "/abs/path/to/Resources"}
{"action": "environment"}
{"action": "packages", "operation": "list"}
{"action": "packages", "operation": "install", "packages": ["numpy>=2"]}
```

- `ensure` installs the bundled runtime if needed, creates the default
  environment if needed, and returns its health. `resourceDir` is optional.
- `environment` is read-only: it returns the same health object and exits 0
  even when `ok` is false.
- `packages` with `operation: "list"` returns the installed distributions;
  `operation: "install"` installs the requested requirement strings with `uv`
  and returns the resulting list. `install` requires a non-empty `packages`
  array. `operation` defaults to `list`.

Health response:

```json
{
  "ok": true,
  "pythonVersion": "3.12.16",
  "uvVersion": "0.13.6",
  "pythonPath": "<base>/environments/default/versions/py.../bin/python",
  "envPath": "<base>/environments/default",
  "runtimePath": "<base>/runtimes/python/3.12.16-aarch64-apple-darwin-<sha12>",
  "message": null
}
```

Packages response: `[{"name": "numpy", "version": "2.1.0"}]`, sorted by name.

Errors exit 1 with empty stdout and one `npi-python-error: <message>` line on
stderr. Long installs may emit `npi-python-progress {"stage": "...",
"message": "..."}` lines on stderr; they are informational and must not be
parsed as the result.

Resource resolution for `ensure` and `packages`: an explicit `resourceDir`
(must be an absolute existing directory), then a non-empty
`NPI_PYTHON_RESOURCE_DIR`, then the running executable's `../Resources`
(macOS `.app`), then the crate's own `resources/` in debug builds only.
`environment` is read-only and ignores `resourceDir`.

## Relationship to `omp`

`agent/spawn.rs` sets `NPI_PYTHON_HOST` to `current_exe()` for every spawned
`omp`, so an `omp` running inside the app can only reach the desktop-managed
runtime, including for gateway/worker processes. `omp` runs
`$NPI_PYTHON_HOST --npi-python-host` with an `ensure` request and uses the
absolute `pythonPath` it returns. It does not read the archive or build a venv.

`omp` runs Python code through `eval`, whose Python backend uses this
interpreter, so the scientific suite is importable there. `omp`'s `python`
tool (mounted under `xd://python`, and registered only when `NPI_PYTHON_HOST`
is set) reports the environment and lists or installs packages through this
host; it no longer executes code. See `omp/docs/python-repl.md`.

## Packaging contract

`scripts/build-python-runtime.sh` is the only supported way to produce the
archive. It is executable and must stay that way.

- Upstream URLs and SHA-256 values are pinned in the script. Downloads are
  cached in `.cache/runtime-downloads/` and re-verified on every run; there is
  no unverified download path.
- The license texts the upstream archives omit (CPython incorporated-software
  notices, uv dual license, Tcl/Tk terms) are fetched from pinned tags and
  verified, never from a moving branch.
- The staged tree removes `.DS_Store`, `__pycache__` and `*.pyc`, normalizes
  every mtime to the pinned `SOURCE_DATE_EPOCH` (2026-10-26T00:00:00Z), and
  writes a sorted member list.
- The tar uses the ustar format (paths verified inside its limits) with
  owner/group normalized to `0:0` (`root:wheel`), `--no-recursion`,
  `COPYFILE_DISABLE=1`, and gzip `-9 -n`
  (zero header timestamp). The script builds the archive twice and fails unless
  the two files are byte-identical.
- On macOS every Mach-O file is checked with `codesign --verify --strict`; an
  unsigned or broken file is ad-hoc signed with `codesign --force --sign -`.
  The upstream CPython binaries are already linker-signed ad-hoc and uv is
  Developer ID signed, so in the current bundle no re-signing is needed.
  Timestamps are normalized again after signing.
- The script validates the packaged archive by extracting it to a fresh
  directory, checking required paths, running `python3 --version`,
  `python3 -m pip --version`, `uv --version`, and verifying the signatures of
  the extracted interpreter and uv.

To regenerate after changing a pin:

```bash
./scripts/build-python-runtime.sh
```

The wheelhouse is built from the runtime archive (its `python3`, pip and `uv`
are the build tools) and the committed lock:

```bash
./scripts/build-python-wheelhouse.sh            # from python-scientific.lock
./scripts/build-python-wheelhouse.sh --relock   # re-resolve python-scientific.in first
```

It downloads the pinned wheels into `.cache/runtime-downloads/wheels/` with
`pip download --require-hashes`, packages them with the same ustar/gzip
normalization (built twice, byte-identical), installs the packaged archive
offline into a scratch venv and imports every package, then writes
`python-runtime-wheels-manifest.json`.

The script writes the archive and `python-runtime-manifest.json` into
`src-tauri/resources/`. The manifest keeps the fields the Rust readers require
(`runtimeVersion`, `platform`, `archiveName`, `archiveSha256`, `archiveSize`,
`uvVersion`, `cpythonBuild`, `components`) and adds informational
`archiveUncompressedSize`, `upstream`, `licenses`, `licenseSources` and `build`
objects. Unknown keys are ignored by the Rust manifest struct, so old readers
still work.

## Source checkout builds

The archives are generated binaries (38 MB runtime, 76 MB wheelhouse) and are
not committed. `src-tauri/resources/.gitignore` ignores
`python-runtime-*.tar.gz`; both manifests and the lock stay tracked so the
expected digests and package pins are reviewable.

Before `tauri build` (and before `npm run build`, which invokes it), prepare
and verify the runtime:

```bash
./scripts/build-python-runtime.sh
./scripts/build-python-wheelhouse.sh
node scripts/check-python-runtime.mjs
```

`check-python-runtime.mjs` is the fail-closed pre-build gate. It reads the
committed manifests, requires both archives, and compares their size and
SHA-256. It
never downloads, extracts or builds anything: a missing or stale archive stops
the build with the exact `./scripts/build-python-runtime.sh` command to run.
`tauri build` alone does not run a JavaScript pre-build hook, so the npm build
wrapper must call this check (or the release script must run it explicitly);
there is no silent build without the runtime.

Smoke-test a finished app against the real host protocol, using a throwaway
`NPI_ENV_DIR`:

```bash
node scripts/smoke-python-runtime.mjs \
  "dist/.../NPi Desktop.app/Contents/MacOS/NPi Desktop" \
  "dist/.../NPi Desktop.app" \
  "dist/.../NPi Desktop.app/Contents/Resources/resources"
```

Tauri 2.12 copies the `resources/python-runtime-*` slice under `Contents/Resources/resources/`
(resource paths preserve the `resources/` prefix), so the third argument is the
nested directory. The smoke command verifies the packaged archive digest,
calls `environment` (must be healthy=false before provisioning), `ensure`,
`environment` and `packages list`, runs stdlib imports with the reported
interpreter, checks the version and archive-addressed runtime directory, and
verifies the nested Mach-O signatures. It removes its temporary environment
afterwards and installs no packages.

## Limits and risks

- Only macOS Apple Silicon is bundled. Other targets need their own pinned
  archive and manifest; do not run this one under Rosetta or on another OS.
- The runtime's Mach-O files are ad-hoc signed (the CPython binaries by their
  upstream linker, uv by Developer ID). Release packaging must preserve valid
  nested signatures through the app's own signing and notarization flow.
- The runtime is 38.6 MB compressed (95.4 MB on disk), so the first `ensure`
  is a visible one-time cost. Later calls read state and probe two versions.
- `uv` package installs need network access to the configured index. Virtual
  environment creation is offline because ensurepip is bundled.
- No system, Homebrew, pyenv or project `.venv` interpreter is used when the
  desktop host is configured; an unavailable host is an error, not a fallback.
