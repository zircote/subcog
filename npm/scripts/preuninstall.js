#!/usr/bin/env node
/**
 * @fileoverview Preuninstall script for @zircote/subcog
 *
 * Cleans up installed binary on uninstall.
 */

'use strict';

const fs = require('fs');
const path = require('path');

const BINARY_NAME = 'subcog';

function cleanup() {
  const binDir = path.join(__dirname, '..', 'bin');

  if (!fs.existsSync(binDir)) {
    return;
  }

  // Remove binary
  const binaryPath = path.join(binDir, BINARY_NAME);
  const binaryExePath = path.join(binDir, `${BINARY_NAME}.exe`);

  [binaryPath, binaryExePath].forEach((filePath) => {
    if (fs.existsSync(filePath)) {
      try {
        fs.unlinkSync(filePath);
        console.log(`Removed: ${filePath}`);
      } catch (error) {
        console.warn(`Warning: Could not remove ${filePath}: ${error.message}`);
      }
    }
  });

  // Try to remove empty bin directory
  try {
    const files = fs.readdirSync(binDir);
    if (files.length === 0) {
      fs.rmdirSync(binDir);
      console.log(`Removed directory: ${binDir}`);
    }
  } catch {
    // Ignore errors when removing directory
  }
}

cleanup();
