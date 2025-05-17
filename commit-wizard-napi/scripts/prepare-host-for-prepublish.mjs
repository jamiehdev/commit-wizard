import fs from 'fs/promises';
import path from 'path';

// Read main package.json
const mainPackageJsonPath = path.resolve('package.json');
const mainPackageJsonContent = await fs.readFile(mainPackageJsonPath, 'utf8');
const mainPackage = JSON.parse(mainPackageJsonContent);

const version = mainPackage.version;
const mainName = mainPackage.name; // e.g., @jamiehdev/commit-wizard
const napiBinaryName = mainPackage.napi.name; // e.g., commit-wizard-napi

// Get the platform from command line args or default to the current platform
const platformArg = process.argv[2];

// Define all supported platforms with their configurations
const platforms = {
  'linux-x64-gnu': { os: 'linux', cpu: 'x64' },
  'darwin-x64': { os: 'darwin', cpu: 'x64' },
  'darwin-arm64': { os: 'darwin', cpu: 'arm64' },
  'win32-x64-msvc': { os: 'win32', cpu: 'x64' }
};

// If a specific platform is provided, only process that one
const platformsToProcess = platformArg ? 
  (platforms[platformArg] ? { [platformArg]: platforms[platformArg] } : {}) : 
  platforms;

if (platformArg && !platforms[platformArg]) {
  console.error(`Error: Unknown platform "${platformArg}". Supported platforms are: ${Object.keys(platforms).join(', ')}`);
  process.exit(1);
}

// Process each platform
for (const [platformTriple, { os, cpu }] of Object.entries(platformsToProcess)) {
  try {
    const artifactName = `${napiBinaryName}.${platformTriple}.node`; // e.g., commit-wizard-napi.linux-x64-gnu.node
    
    // Construct the final package name
    const finalSubPackageName = `@jamiehdev/commit-wizard-${platformTriple}`;
    
    const npmDir = path.resolve('npm', platformTriple);
    await fs.mkdir(npmDir, { recursive: true });
    
    // Copy the .node file if it exists
    const sourceNodeFile = path.resolve(artifactName);
    const destNodeFile = path.join(npmDir, artifactName);
    
    // Check if source .node file exists (it should have been built by `npm run build`)
    try {
      await fs.access(sourceNodeFile);
      await fs.copyFile(sourceNodeFile, destNodeFile);
      console.log(`Copied ${sourceNodeFile} to ${destNodeFile}`);
    } catch (e) {
      console.warn(`Warning: Source .node file ${sourceNodeFile} not found. This platform may not be built yet.`);
      // Continue with other platforms instead of exiting
      continue;
    }
    
    // Create the sub-package.json
    const subPackageJson = {
      name: finalSubPackageName,
      version: version,
      os: [os],
      cpu: [cpu],
      main: artifactName,
      files: [artifactName],
    };
    
    const subPackageJsonPath = path.join(npmDir, 'package.json');
    await fs.writeFile(subPackageJsonPath, JSON.stringify(subPackageJson, null, 2));
    
    console.log(`Prepared ${finalSubPackageName} in ${npmDir}`);
  } catch (error) {
    console.error(`Error processing platform ${platformTriple}:`, error);
  }
}

console.log('Preparation complete for all platforms.');