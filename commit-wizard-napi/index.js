const { join } = require('path');
const os = process.platform;
const arch = process.arch;
let binding = null;
let file = null;

switch (os) {
  case 'win32':
    if (arch === 'x64') {
      file = join(__dirname, 'npm', 'win32-x64-msvc', 'commit-wizard-napi.win32-x64-msvc.node');
    }
    break;
  case 'darwin':
    if (arch === 'x64') {
      file = join(__dirname, 'npm', 'darwin-x64', 'commit-wizard-napi.darwin-x64.node');
    } else if (arch === 'arm64') {
      file = join(__dirname, 'npm', 'darwin-arm64', 'commit-wizard-napi.darwin-arm64.node');
    }
    break;
  case 'linux':
    if (arch === 'x64') {
      file = join(__dirname, 'npm', 'linux-x64-gnu', 'commit-wizard-napi.linux-x64-gnu.node');
    }
    break;
}

if (!file) {
  throw new Error(`Unsupported platform: ${os} ${arch}`);
}

try {
  binding = require(file);
} catch (err) {
  throw new Error(`Failed to load native binding\n${err}`);
}

module.exports = binding;
