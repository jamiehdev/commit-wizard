import fs from "fs/promises";
import path from "path";

const rootDir = process.cwd();
const mainPackageJsonPath = path.join(rootDir, "package.json");
const mainPackageJsonContent = await fs.readFile(mainPackageJsonPath, "utf8");
const mainPackage = JSON.parse(mainPackageJsonContent);

const version = mainPackage.version;
const artifactsDir =
  process.env.CW_ARTIFACTS_DIR || path.resolve(rootDir, "artifacts");
const platformArg = process.argv[2];

const platforms = {
  "linux-x64-gnu": {
    os: "linux",
    cpu: "x64",
    target: "x86_64-unknown-linux-gnu",
    binaryName: "commit-wizard",
  },
  "darwin-x64": {
    os: "darwin",
    cpu: "x64",
    target: "x86_64-apple-darwin",
    binaryName: "commit-wizard",
  },
  "darwin-arm64": {
    os: "darwin",
    cpu: "arm64",
    target: "aarch64-apple-darwin",
    binaryName: "commit-wizard",
  },
  "win32-x64-msvc": {
    os: "win32",
    cpu: "x64",
    target: "x86_64-pc-windows-msvc",
    binaryName: "commit-wizard.exe",
  },
};

const platformsToProcess = platformArg
  ? platforms[platformArg]
    ? { [platformArg]: platforms[platformArg] }
    : {}
  : platforms;

if (platformArg && !platforms[platformArg]) {
  console.error(
    `Error: Unknown platform "${platformArg}". Supported platforms are: ${Object.keys(
      platforms
    ).join(", ")}`
  );
  process.exit(1);
}

for (const [platformTriple, platformInfo] of Object.entries(
  platformsToProcess
)) {
  const { os, cpu, target, binaryName } = platformInfo;
  const artifactDir = path.join(artifactsDir, `rust-binary-${target}`);
  const sourceBinary = path.join(artifactDir, `commit-wizard-${target}`);
  const sourcePath =
    binaryName.endsWith(".exe") ? `${sourceBinary}.exe` : sourceBinary;

  try {
    await fs.access(sourcePath);
  } catch (error) {
    console.error(
      `Missing binary for ${platformTriple} at ${sourcePath}. Set CW_ARTIFACTS_DIR if artifacts are elsewhere.`
    );
    process.exit(1);
  }

  const npmDir = path.join(rootDir, "npm", platformTriple);
  await fs.mkdir(npmDir, { recursive: true });
  const destBinary = path.join(npmDir, binaryName);
  await fs.copyFile(sourcePath, destBinary);

  if (os !== "win32") {
    await fs.chmod(destBinary, 0o755);
  }

  const subPackageJson = {
    name: `@jamiehdev/commit-wizard-${platformTriple}`,
    version,
    os: [os],
    cpu: [cpu],
    bin: {
      "commit-wizard": binaryName,
    },
    files: [binaryName],
  };

  const subPackageJsonPath = path.join(npmDir, "package.json");
  await fs.writeFile(
    subPackageJsonPath,
    JSON.stringify(subPackageJson, null, 2)
  );

  console.log(`Prepared ${platformTriple} package in ${npmDir}`);
}

console.log("✅ platform package preparation complete");
