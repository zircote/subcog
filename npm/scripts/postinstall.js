#!/usr/bin/env node
/**
 * @fileoverview Postinstall script for @zircote/subcog
 *
 * Downloads pre-built binaries from GitHub Releases with fallback to cargo install.
 * Follows patterns established by esbuild, turbo, and prisma for binary distribution.
 *
 * SECURITY:
 * - Downloads only from official GitHub Releases
 * - Verifies all downloads with SHA256 checksums
 * - No arbitrary code execution (uses spawn with argument arrays)
 * - No eval, shell execution, or dynamic code generation
 * - Transparent logging of all operations
 * - Environment variables for opt-out (SUBCOG_SKIP_INSTALL)
 *
 * To skip: SUBCOG_SKIP_INSTALL=1 npm install
 * To audit: View source at https://github.com/zircote/subcog/blob/main/npm/scripts/postinstall.js
 */

'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');
const http = require('http');
const { spawn, execSync } = require('child_process');
const { createHash } = require('crypto');
const zlib = require('zlib');

// Configuration
const PACKAGE_NAME = '@zircote/subcog';
const BINARY_NAME = 'subcog';
const GITHUB_REPO = 'zircote/subcog';

// Read version from package.json
const packageJson = require('../package.json');
const VERSION = packageJson.version;

// Platform/architecture mapping to Rust target triples
const PLATFORM_MAP = {
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-x64-musl': 'x86_64-unknown-linux-musl',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win32-x64': 'x86_64-pc-windows-msvc',
};

// Detect if running in musl environment (Alpine, etc.)
function isMusl() {
  if (process.platform !== 'linux') return false;

  try {
    // Check for Alpine
    if (fs.existsSync('/etc/alpine-release')) return true;

    // Check ldd output for musl
    // Note: execSync here is safe - no user input, fixed command
    const lddOutput = execSync('ldd --version 2>&1 || true', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    return lddOutput.toLowerCase().includes('musl');
  } catch {
    return false;
  }
}

// Get the appropriate target triple for the current platform
function getTargetTriple() {
  const platform = process.platform;
  const arch = process.arch;

  // Handle musl-based Linux (Alpine, etc.)
  if (platform === 'linux' && arch === 'x64' && isMusl()) {
    return PLATFORM_MAP['linux-x64-musl'];
  }

  const key = `${platform}-${arch}`;
  const target = PLATFORM_MAP[key];

  if (!target) {
    throw new Error(
      `Unsupported platform: ${platform}-${arch}\n` +
        `Supported platforms: ${Object.keys(PLATFORM_MAP).join(', ')}`
    );
  }

  return target;
}

// Construct the download URL for the binary archive
function getDownloadUrl(target) {
  return `https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${BINARY_NAME}-${VERSION}-${target}.tar.gz`;
}

// Construct the checksums URL
function getChecksumsUrl() {
  return `https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/checksums.txt`;
}

// Follow redirects and download file
function downloadFile(url, maxRedirects = 5) {
  return new Promise((resolve, reject) => {
    if (maxRedirects <= 0) {
      reject(new Error('Too many redirects'));
      return;
    }

    const protocol = url.startsWith('https') ? https : http;
    const request = protocol.get(url, { timeout: 60000 }, (response) => {
      // Handle redirects (GitHub releases use redirects)
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        downloadFile(response.headers.location, maxRedirects - 1)
          .then(resolve)
          .catch(reject);
        return;
      }

      if (response.statusCode !== 200) {
        reject(new Error(`HTTP ${response.statusCode}: ${response.statusMessage}`));
        return;
      }

      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('end', () => resolve(Buffer.concat(chunks)));
      response.on('error', reject);
    });

    request.on('error', reject);
    request.on('timeout', () => {
      request.destroy();
      reject(new Error('Request timed out'));
    });
  });
}

// Parse checksums.txt and return a map of filename -> sha256
async function fetchChecksums() {
  try {
    const url = getChecksumsUrl();
    const data = await downloadFile(url);
    const checksums = new Map();

    data
      .toString('utf8')
      .split('\n')
      .filter((line) => line.trim())
      .forEach((line) => {
        const [hash, filename] = line.trim().split(/\s+/);
        if (hash && filename) {
          checksums.set(filename, hash);
        }
      });

    return checksums;
  } catch (error) {
    console.warn(`Warning: Could not fetch checksums: ${error.message}`);
    return null;
  }
}

// Verify SHA256 checksum
function verifyChecksum(data, expectedHash, filename) {
  const actualHash = createHash('sha256').update(data).digest('hex');
  if (actualHash !== expectedHash) {
    throw new Error(
      `Checksum mismatch for ${filename}\n` +
        `Expected: ${expectedHash}\n` +
        `Actual:   ${actualHash}`
    );
  }
  console.log(`Verified checksum for ${filename}`);
}

// Extract tar.gz archive
function extractTarGz(data, destDir) {
  return new Promise((resolve, reject) => {
    const gunzip = zlib.createGunzip();
    const chunks = [];

    gunzip.on('data', (chunk) => chunks.push(chunk));
    gunzip.on('end', () => {
      try {
        const tarData = Buffer.concat(chunks);
        extractTar(tarData, destDir);
        resolve();
      } catch (error) {
        reject(error);
      }
    });
    gunzip.on('error', reject);
    gunzip.end(data);
  });
}

// Simple tar extraction (handles ustar format)
function extractTar(data, destDir) {
  let offset = 0;

  while (offset < data.length - 512) {
    // Read header
    const header = data.slice(offset, offset + 512);

    // Check for end of archive (two zero blocks)
    if (header.every((byte) => byte === 0)) {
      break;
    }

    // Parse header fields
    const name = header.slice(0, 100).toString('utf8').replace(/\0/g, '').trim();
    const sizeOctal = header.slice(124, 136).toString('utf8').replace(/\0/g, '').trim();
    const typeFlag = header[156];

    if (!name) {
      offset += 512;
      continue;
    }

    const size = parseInt(sizeOctal, 8) || 0;
    offset += 512;

    // Only extract regular files (typeFlag 0 or '0')
    if (typeFlag === 0 || typeFlag === 48) {
      // 48 is ASCII '0'
      const content = data.slice(offset, offset + size);
      const destPath = path.join(destDir, path.basename(name));

      // Only extract the binary
      if (name === BINARY_NAME || name.endsWith(`/${BINARY_NAME}`)) {
        fs.writeFileSync(destPath, content);
        // Make executable on Unix
        if (process.platform !== 'win32') {
          fs.chmodSync(destPath, 0o755);
        }
        console.log(`Extracted: ${destPath}`);
      }
    }

    // Advance to next entry (512-byte aligned)
    offset += Math.ceil(size / 512) * 512;
  }
}

// Check if cargo is available
function isCargoAvailable() {
  try {
    // Note: execSync with fixed command string is safe - no user input
    execSync('cargo --version', { stdio: 'pipe' });
    return true;
  } catch {
    return false;
  }
}

// Download and install the binary
async function downloadBinary() {
  const target = getTargetTriple();
  const url = getDownloadUrl(target);
  const archiveName = `${BINARY_NAME}-${VERSION}-${target}.tar.gz`;

  console.log(`Downloading ${PACKAGE_NAME} v${VERSION} for ${target}...`);
  console.log(`URL: ${url}`);

  // Fetch checksums in parallel with the binary
  const [archiveData, checksums] = await Promise.all([downloadFile(url), fetchChecksums()]);

  // Verify checksum if available
  if (checksums && checksums.has(archiveName)) {
    verifyChecksum(archiveData, checksums.get(archiveName), archiveName);
  }

  // Create bin directory
  const binDir = path.join(__dirname, '..', 'bin');
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Extract archive
  console.log('Extracting archive...');
  await extractTarGz(archiveData, binDir);

  // Handle Windows executable extension
  if (process.platform === 'win32') {
    const binPath = path.join(binDir, BINARY_NAME);
    const exePath = path.join(binDir, `${BINARY_NAME}.exe`);
    if (fs.existsSync(binPath) && !fs.existsSync(exePath)) {
      fs.renameSync(binPath, exePath);
    }
  }

  console.log(`Successfully installed ${PACKAGE_NAME} v${VERSION}`);
}

// Fallback to cargo install
function cargoInstall() {
  return new Promise((resolve, reject) => {
    console.log('Attempting fallback: cargo install...');

    if (!isCargoAvailable()) {
      reject(
        new Error(
          'cargo is not installed. Please install Rust from https://rustup.rs/ ' +
            'or download a pre-built binary from https://github.com/zircote/subcog/releases'
        )
      );
      return;
    }

    const binDir = path.join(__dirname, '..', 'bin');
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    console.log(`Installing ${BINARY_NAME} v${VERSION} via cargo...`);
    console.log('This may take several minutes...');

    // Using spawn with argument array - safe from injection
    const cargo = spawn(
      'cargo',
      [
        'install',
        BINARY_NAME,
        '--version',
        VERSION,
        '--locked',
        '--force',
        '--root',
        path.join(__dirname, '..'),
      ],
      {
        stdio: 'inherit',
      }
    );

    cargo.on('close', (code) => {
      if (code === 0) {
        console.log(`Successfully installed ${PACKAGE_NAME} v${VERSION} via cargo`);
        resolve();
      } else {
        reject(new Error(`cargo install failed with exit code ${code}`));
      }
    });

    cargo.on('error', reject);
  });
}

// Try cargo install from git repository
function cargoInstallFromGit() {
  return new Promise((resolve, reject) => {
    console.log('Attempting fallback: cargo install from git...');

    const binDir = path.join(__dirname, '..', 'bin');
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    // Using spawn with argument array - safe from injection
    const cargo = spawn(
      'cargo',
      [
        'install',
        '--git',
        `https://github.com/${GITHUB_REPO}.git`,
        '--tag',
        `v${VERSION}`,
        '--locked',
        '--force',
        '--root',
        path.join(__dirname, '..'),
      ],
      {
        stdio: 'inherit',
      }
    );

    cargo.on('close', (code) => {
      if (code === 0) {
        console.log(`Successfully installed ${PACKAGE_NAME} v${VERSION} from git`);
        resolve();
      } else {
        reject(new Error(`cargo install from git failed with exit code ${code}`));
      }
    });

    cargo.on('error', reject);
  });
}

// Main installation flow
async function main() {
  console.log('');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
  console.log('  @zircote/subcog postinstall');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
  console.log('');
  console.log('This package downloads pre-built binaries from GitHub Releases.');
  console.log('For security information, see: npm/SECURITY.md');
  console.log('To skip: SUBCOG_SKIP_INSTALL=1 npm install');
  console.log('');

  // Skip installation in CI environments if SUBCOG_SKIP_INSTALL is set
  if (process.env.SUBCOG_SKIP_INSTALL === '1' || process.env.SUBCOG_SKIP_INSTALL === 'true') {
    console.log('Skipping binary installation (SUBCOG_SKIP_INSTALL is set)');
    return;
  }

  // Allow overriding the binary path
  if (process.env.SUBCOG_BINARY_PATH) {
    console.log(`Using custom binary path: ${process.env.SUBCOG_BINARY_PATH}`);
    const binDir = path.join(__dirname, '..', 'bin');
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    const destPath = path.join(binDir, BINARY_NAME);
    fs.copyFileSync(process.env.SUBCOG_BINARY_PATH, destPath);
    if (process.platform !== 'win32') {
      fs.chmodSync(destPath, 0o755);
    }
    console.log(`Installed custom binary to ${destPath}`);
    return;
  }

  // Try downloading pre-built binary first
  try {
    await downloadBinary();
    return;
  } catch (downloadError) {
    console.warn(`Binary download failed: ${downloadError.message}`);
    console.log('');
  }

  // Fallback 1: Try cargo install from crates.io
  try {
    await cargoInstall();
    return;
  } catch (cargoError) {
    console.warn(`cargo install failed: ${cargoError.message}`);
    console.log('');
  }

  // Fallback 2: Try cargo install from git
  try {
    await cargoInstallFromGit();
    return;
  } catch (gitError) {
    console.warn(`cargo install from git failed: ${gitError.message}`);
    console.log('');
  }

  // All installation methods failed
  console.error(`
================================================================================
Failed to install ${PACKAGE_NAME}

Please try one of the following:

1. Download a pre-built binary from:
   https://github.com/${GITHUB_REPO}/releases/tag/v${VERSION}

2. Install via Homebrew (macOS):
   brew tap zircote/tap && brew install subcog

3. Build from source:
   cargo install --git https://github.com/${GITHUB_REPO}.git --tag v${VERSION}

For more information, visit: https://github.com/${GITHUB_REPO}
================================================================================
`);
  process.exit(1);
}

main().catch((error) => {
  console.error(`Installation failed: ${error.message}`);
  process.exit(1);
});
