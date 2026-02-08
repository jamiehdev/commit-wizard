#!/usr/bin/env node

const { runCommitWizardCli } = require("./index");

runCommitWizardCli(process.argv.slice(2))
  .then((exitCode) => {
    process.exit(exitCode);
  })
  .catch((error) => {
    const message = error && error.message ? error.message : String(error);
    console.error(message);
    process.exit(1);
  });
