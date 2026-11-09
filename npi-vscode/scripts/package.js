'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { version } = require('../package.json');

async function main() {
  const root = path.resolve(__dirname, '..');
  const target = process.platform + '-' + process.arch;
  if (target !== 'darwin-arm64') throw new Error('This release bundles the macOS Apple Silicon backend only.');
  const source = path.resolve(process.argv[2] || path.join(root, '../desktop/src-tauri/binaries/npi-aarch64-apple-darwin'));
  execFileSync('/usr/bin/codesign', ['--verify', '--strict', source], { stdio: 'inherit' });
  const runtimeDir = path.join(root, 'runtime', target);
  await fs.mkdir(runtimeDir, { recursive: true });
  await fs.copyFile(source, path.join(runtimeDir, 'npi'));
  await fs.chmod(path.join(runtimeDir, 'npi'), 0o755);
  execFileSync(process.execPath, [path.join(root, 'node_modules/@vscode/vsce/vsce'), 'package',
    '--allow-missing-repository', '--target', target, '--out', 'npi-vscode-' + version + '-' + target + '.vsix'],
  { cwd: root, stdio: 'inherit' });
}

main().catch(error => { console.error(error); process.exitCode = 1; });
