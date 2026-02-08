#!/usr/bin/env node
import fs from 'fs';
import path from 'path';

// Remove the staged target directory
const dirPath = path.resolve(process.cwd(), 'target');
try {
  fs.rmSync(dirPath, { recursive: true, force: true });
} catch (err) {
  console.error(`Error removing directory ${dirPath}:`, err);
  process.exit(1);
}