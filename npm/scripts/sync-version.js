#!/usr/bin/env node
/**
 * @fileoverview Sync npm package version with Cargo.toml
 *
 * This script reads the version from Cargo.toml and updates package.json.
 * Run this before publishing a new npm release.
 *
 * Usage: node scripts/sync-version.js
 */

'use strict';

const fs = require('fs');
const path = require('path');

const CARGO_TOML_PATH = path.join(__dirname, '..', '..', 'Cargo.toml');
const PACKAGE_JSON_PATH = path.join(__dirname, '..', 'package.json');

function extractCargoVersion() {
  const cargoToml = fs.readFileSync(CARGO_TOML_PATH, 'utf8');

  // Match version in [package] section
  const match = cargoToml.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);

  if (!match) {
    throw new Error('Could not find version in Cargo.toml');
  }

  return match[1];
}

function updatePackageJson(version) {
  const packageJson = JSON.parse(fs.readFileSync(PACKAGE_JSON_PATH, 'utf8'));
  const oldVersion = packageJson.version;

  if (oldVersion === version) {
    console.log(`Version already at ${version}`);
    return false;
  }

  packageJson.version = version;

  fs.writeFileSync(PACKAGE_JSON_PATH, JSON.stringify(packageJson, null, 2) + '\n');

  console.log(`Updated package.json: ${oldVersion} -> ${version}`);
  return true;
}

function main() {
  try {
    const version = extractCargoVersion();
    console.log(`Cargo.toml version: ${version}`);

    const updated = updatePackageJson(version);

    if (updated) {
      console.log('\nRemember to commit the updated package.json');
    }
  } catch (error) {
    console.error(`Error: ${error.message}`);
    process.exit(1);
  }
}

main();
