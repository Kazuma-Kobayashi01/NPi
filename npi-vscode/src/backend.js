'use strict';
const fs = require('node:fs/promises');
const { constants } = require('node:fs');
const path = require('node:path');

async function resolveBackend(extensionPath, executablePath) {
  if (executablePath) return executablePath;
  const target = process.platform + '-' + process.arch;
  const executable = path.join(extensionPath, 'runtime', target, process.platform === 'win32' ? 'npi.exe' : 'npi');
  try {
    await fs.access(executable, constants.X_OK);
  } catch (error) {
    throw new Error('The bundled NPi backend is unavailable for ' + target + '. Install the matching NPi VSIX or set npi.executablePath to your NPi executable.', { cause: error });
  }
  return executable;
}

module.exports = { resolveBackend };
