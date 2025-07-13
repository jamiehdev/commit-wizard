import fs from 'fs/promises';
import path from 'path';

async function syncVersions() {
  try {
    // get the workspace root directory - handle being called from different locations
    const currentDir = process.cwd();
    let workspaceRoot, napiDir;
    
    // check if we're already in the napi directory
    if (currentDir.endsWith('commit-wizard-napi')) {
      napiDir = currentDir;
      workspaceRoot = path.resolve(napiDir, '..');
    } 
    // check if we're in the workspace root
    else if (await fs.access(path.join(currentDir, 'commit-wizard-napi', 'package.json')).then(() => true).catch(() => false)) {
      workspaceRoot = currentDir;
      napiDir = path.join(workspaceRoot, 'commit-wizard-napi');
    }
    // try to find workspace root by looking for Cargo.toml
    else {
      let searchDir = currentDir;
      while (searchDir !== path.dirname(searchDir)) {
        if (await fs.access(path.join(searchDir, 'Cargo.toml')).then(() => true).catch(() => false)) {
          workspaceRoot = searchDir;
          napiDir = path.join(workspaceRoot, 'commit-wizard-napi');
          break;
        }
        searchDir = path.dirname(searchDir);
      }
      
      if (!workspaceRoot) {
        throw new Error('could not find workspace root (no Cargo.toml found)');
      }
    }
    
    console.log(`workspace root: ${workspaceRoot}`);
    console.log(`napi directory: ${napiDir}`);
    
    // read the workspace Cargo.toml to get the version
    const cargoPath = path.join(workspaceRoot, 'Cargo.toml');
    console.log(`reading version from: ${cargoPath}`);
    
    const cargoContent = await fs.readFile(cargoPath, 'utf8');
    
    // find version in [workspace.package] section
    const versionMatch = cargoContent.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
    if (!versionMatch) {
      throw new Error('workspace version not found in Cargo.toml');
    }
    
    const version = versionMatch[1];
    console.log(`found workspace version: ${version}`);
    
    // update package.json in the NAPI crate
    const packagePath = path.join(napiDir, 'package.json');
    console.log(`updating package.json at: ${packagePath}`);
    
    const packageContent = await fs.readFile(packagePath, 'utf8');
    const packageJson = JSON.parse(packageContent);
    
    const oldVersion = packageJson.version;
    packageJson.version = version;
    
    // write back with proper formatting
    const updatedContent = JSON.stringify(packageJson, null, 2) + '\n';
    await fs.writeFile(packagePath, updatedContent);
    
    console.log(`✅ synced package.json version: ${oldVersion} → ${version}`);
    
    // also check for any nested package.json files that need syncing
    const nestedPackagePatterns = [
      path.join(napiDir, 'npm', '*', 'package.json'),
      path.join(napiDir, '*', 'npm', '*', 'package.json')
    ];
    
    for (const pattern of nestedPackagePatterns) {
      try {
        // simple glob-like matching for npm subdirectories
        const baseDir = path.dirname(pattern);
        const entries = await fs.readdir(baseDir, { withFileTypes: true }).catch(() => []);
        
        for (const entry of entries) {
          if (entry.isDirectory()) {
            const nestedPackagePath = path.join(baseDir, entry.name, 'package.json');
            try {
              const nestedContent = await fs.readFile(nestedPackagePath, 'utf8');
              const nestedJson = JSON.parse(nestedContent);
              
              if (nestedJson.version !== version) {
                const oldNestedVersion = nestedJson.version;
                nestedJson.version = version;
                
                const updatedNestedContent = JSON.stringify(nestedJson, null, 2) + '\n';
                await fs.writeFile(nestedPackagePath, updatedNestedContent);
                
                console.log(`✅ synced nested package.json: ${nestedPackagePath} (${oldNestedVersion} → ${version})`);
              }
            } catch (e) {
              // ignore missing or invalid nested package.json files
            }
          }
        }
      } catch (e) {
        // ignore missing directories
      }
    }
    
  } catch (error) {
    console.error(`❌ version sync failed: ${error.message}`);
    process.exit(1);
  }
}

// run the sync if this script is executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  syncVersions().catch(err => {
    console.error(`❌ sync failed: ${err.message}`);
    process.exit(1);
  });
}

export default syncVersions;