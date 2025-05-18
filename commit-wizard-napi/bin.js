#!/usr/bin/env node

// first try to use our npm module
try {
  const { runCommitWizardCli } = require("./index");
  const arguments = process.argv.slice(2);
  
  runCommitWizardCli(arguments)
    .then(commitMessage => {
      process.exit(0);
    })
    .catch(error => {
      console.error("Could not run Node.js version, error:", error.message);
      
      // if napi version fails, fall back to CLI binary
      try {
        const path = require('path');
        const { execFileSync } = require('child_process');
        
        // find the native CLI binary relative to this script
        const binaryPath = path.resolve(__dirname, '..', 'target', 'release', 'commit-wizard');
        console.log(`Using native binary: ${binaryPath}`);
        
        // pass through any arguments
        console.log(`Trying native binary at: ${binaryPath}`);
        execFileSync(binaryPath, process.argv.slice(2), { 
          stdio: 'inherit',
          env: process.env
        });
        process.exit(0);
      } catch (fallbackError) {
        console.error("Native binary fallback also failed:", fallbackError.message);
        process.exit(1);
      }
    });
} catch (initialError) {
  console.error("Could not load Node.js module:", initialError.message);
  // if loading the Node.js binding fails, fall back to the native CLI binary
  try {
    const path = require('path');
    const { execFileSync } = require('child_process');
    const binaryPath = path.resolve(__dirname, '..', 'target', 'release', 'commit-wizard');
    console.log(`Using native binary: ${binaryPath}`);
    execFileSync(binaryPath, process.argv.slice(2), {
      stdio: 'inherit',
      env: process.env
    });
    process.exit(0);
  } catch (fallbackError) {
    console.error("Native binary fallback also failed:", fallbackError.message);
    process.exit(1);
  }
}