#!/usr/bin/env node
import fs from 'fs';
import path from 'path';

// Ensure target directory exists and list its contents
const mode = process.argv[2] || 'release';
const dirPath = path.resolve(process.cwd(), 'target', mode);
fs.mkdirSync(dirPath, { recursive: true });
console.log(`Contents of ${dirPath}:`);
try {
  for (const entry of fs.readdirSync(dirPath)) {
    console.log(entry);
  }
} catch (err) {
  console.error(`Error reading directory ${dirPath}:`, err);
  process.exit(1);
}