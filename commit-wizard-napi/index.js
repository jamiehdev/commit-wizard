const { existsSync } = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const PLATFORM_PACKAGES = {
  "darwin-x64": {
    packageName: "@jamiehdev/commit-wizard-darwin-x64",
    binaryName: "commit-wizard",
  },
  "darwin-arm64": {
    packageName: "@jamiehdev/commit-wizard-darwin-arm64",
    binaryName: "commit-wizard",
  },
  "linux-x64": {
    packageName: "@jamiehdev/commit-wizard-linux-x64-gnu",
    binaryName: "commit-wizard",
  },
  "win32-x64": {
    packageName: "@jamiehdev/commit-wizard-win32-x64-msvc",
    binaryName: "commit-wizard.exe",
  },
};

function isMusl() {
  if (!process.report || typeof process.report.getReport !== "function") {
    return false;
  }
  const { glibcVersionRuntime } = process.report.getReport().header;
  return !glibcVersionRuntime;
}

function resolveBinaryPath() {
  if (process.env.COMMIT_WIZARD_BINARY) {
    const overridePath = path.resolve(process.env.COMMIT_WIZARD_BINARY);
    if (!existsSync(overridePath)) {
      throw new Error(`commit-wizard binary not found at ${overridePath}`);
    }
    return overridePath;
  }

  if (process.platform === "linux" && isMusl()) {
    throw new Error(
      "commit-wizard does not ship musl binaries yet. use COMMIT_WIZARD_BINARY to point to a local build."
    );
  }

  const key = `${process.platform}-${process.arch}`;
  const platformInfo = PLATFORM_PACKAGES[key];
  if (!platformInfo) {
    throw new Error(
      `unsupported platform ${process.platform} ${process.arch}.`
    );
  }

  let packageRoot;
  try {
    packageRoot = path.dirname(
      require.resolve(`${platformInfo.packageName}/package.json`)
    );
  } catch (error) {
    throw new Error(
      `missing optional dependency ${platformInfo.packageName}. reinstall the package or set COMMIT_WIZARD_BINARY.`
    );
  }
  const binaryPath = path.join(packageRoot, platformInfo.binaryName);
  if (!existsSync(binaryPath)) {
    throw new Error(`commit-wizard binary not found at ${binaryPath}`);
  }

  return binaryPath;
}

function runCommitWizardCli(args = []) {
  const binaryPath = resolveBinaryPath();
  return new Promise((resolve, reject) => {
    const child = spawn(binaryPath, args, {
      stdio: "inherit",
      env: process.env,
    });

    child.on("error", reject);
    child.on("close", (code) => resolve(code ?? 1));
  });
}

module.exports = {
  getBinaryPath: resolveBinaryPath,
  runCommitWizardCli,
};
