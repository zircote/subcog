//! Encryption at rest for filesystem storage (CRIT-005).
//!
//! Provides AES-256-GCM authenticated encryption for memory files.
//! Encryption is opt-in via the `encryption` feature flag and requires
//! setting the `SUBCOG_ENCRYPTION_KEY` environment variable.
//!
//! # Security Properties
//!
//! - **Algorithm**: AES-256-GCM (authenticated encryption)
//! - **Key**: 32 bytes (256 bits) from base64-encoded env var
//! - **Nonce**: 12 bytes, randomly generated per encryption
//! - **Format**: `SUBCOG_ENC_V1` magic + nonce + ciphertext + auth tag
//!
//! # Usage
//!
//! ```bash
//! # Generate a key (32 random bytes, base64 encoded)
//! openssl rand -base64 32
//!
//! # Set the environment variable
//! export SUBCOG_ENCRYPTION_KEY="your-base64-encoded-key"
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::security::encryption::{Encryptor, EncryptionConfig};
//!
//! let config = EncryptionConfig::from_env()?;
//! let encryptor = Encryptor::new(config)?;
//!
//! let plaintext = b"sensitive data";
//! let encrypted = encryptor.encrypt(plaintext)?;
//! let decrypted = encryptor.decrypt(&encrypted)?;
//! assert_eq!(plaintext, &decrypted[..]);
//! ```

#[cfg(feature = "encryption")]
mod implementation {
    use crate::{Error, Result};

    use aes_gcm::{
        Aes256Gcm, Key, Nonce,
        aead::{Aead, KeyInit},
    };
    use base64::Engine;
    use rand::RngCore;

    /// Magic bytes to identify encrypted files.
    /// Format: `SUBCOG_ENC_V1\0` (14 bytes)
    pub const MAGIC_HEADER: &[u8] = b"SUBCOG_ENC_V1\0";

    /// Nonce size for AES-256-GCM (12 bytes / 96 bits).
    const NONCE_SIZE: usize = 12;

    /// Key size for AES-256 (32 bytes / 256 bits).
    const KEY_SIZE: usize = 32;

    /// Environment variable for encryption key.
    const ENV_ENCRYPTION_KEY: &str = "SUBCOG_ENCRYPTION_KEY";

    /// Encryption configuration.
    #[derive(Debug, Clone)]
    pub struct EncryptionConfig {
        /// Raw 32-byte encryption key.
        key: [u8; KEY_SIZE],
    }

    impl EncryptionConfig {
        /// Creates configuration from a base64-encoded key.
        ///
        /// # Errors
        ///
        /// Returns an error if the key is invalid or wrong size.
        pub fn from_base64(key_b64: &str) -> Result<Self> {
            let key_bytes = base64::engine::general_purpose::STANDARD
                .decode(key_b64.trim())
                .map_err(|e| Error::InvalidInput(format!("Invalid base64 encryption key: {e}")))?;

            if key_bytes.len() != KEY_SIZE {
                return Err(Error::InvalidInput(format!(
                    "Encryption key must be {} bytes, got {}",
                    KEY_SIZE,
                    key_bytes.len()
                )));
            }

            let mut key = [0u8; KEY_SIZE];
            key.copy_from_slice(&key_bytes);

            Ok(Self { key })
        }

        /// Loads configuration from environment variable.
        ///
        /// # Errors
        ///
        /// Returns an error if `SUBCOG_ENCRYPTION_KEY` is not set or invalid.
        pub fn from_env() -> Result<Self> {
            let key_b64 = std::env::var(ENV_ENCRYPTION_KEY).map_err(|_| {
                Error::InvalidInput(format!(
                    "Encryption enabled but {ENV_ENCRYPTION_KEY} not set. \
                     Generate a key with: openssl rand -base64 32"
                ))
            })?;

            Self::from_base64(&key_b64)
        }

        /// Tries to load configuration from environment, returns None if not configured.
        #[must_use]
        pub fn try_from_env() -> Option<Self> {
            Self::from_env().ok()
        }
    }

    /// AES-256-GCM encryptor.
    pub struct Encryptor {
        cipher: Aes256Gcm,
    }

    impl Encryptor {
        /// Creates a new encryptor from configuration.
        ///
        /// # Errors
        ///
        /// Returns an error if the key is invalid.
        pub fn new(config: EncryptionConfig) -> Result<Self> {
            let key = Key::<Aes256Gcm>::from(config.key);
            let cipher = Aes256Gcm::new(&key);
            Ok(Self { cipher })
        }

        /// Creates an encryptor from environment configuration.
        ///
        /// # Errors
        ///
        /// Returns an error if configuration is missing or invalid.
        pub fn from_env() -> Result<Self> {
            let config = EncryptionConfig::from_env()?;
            Self::new(config)
        }

        /// Encrypts plaintext data.
        ///
        /// Returns: magic header + nonce + ciphertext (includes auth tag)
        ///
        /// # Errors
        ///
        /// Returns an error if encryption fails.
        pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
            // Generate random nonce
            let mut nonce_bytes = [0u8; NONCE_SIZE];
            rand::rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from(nonce_bytes);

            // Encrypt
            let ciphertext =
                self.cipher
                    .encrypt(&nonce, plaintext)
                    .map_err(|e| Error::OperationFailed {
                        operation: "encrypt".to_string(),
                        cause: format!("AES-256-GCM encryption failed: {e}"),
                    })?;

            // Build output: magic + nonce + ciphertext
            let mut output = Vec::with_capacity(MAGIC_HEADER.len() + NONCE_SIZE + ciphertext.len());
            output.extend_from_slice(MAGIC_HEADER);
            output.extend_from_slice(&nonce_bytes);
            output.extend_from_slice(&ciphertext);

            tracing::debug!(
                plaintext_len = plaintext.len(),
                encrypted_len = output.len(),
                "Encrypted data"
            );

            Ok(output)
        }

        /// Decrypts encrypted data.
        ///
        /// # Errors
        ///
        /// Returns an error if decryption fails or data is invalid.
        pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
            let min_size = MAGIC_HEADER.len() + NONCE_SIZE + 16; // 16 = auth tag
            if encrypted.len() < min_size {
                return Err(Error::InvalidInput(format!(
                    "Encrypted data too short: {} bytes, minimum {}",
                    encrypted.len(),
                    min_size
                )));
            }

            // Verify magic header
            if !encrypted.starts_with(MAGIC_HEADER) {
                return Err(Error::InvalidInput(
                    "Invalid encrypted file: missing magic header".to_string(),
                ));
            }

            // Extract nonce and ciphertext
            let nonce_start = MAGIC_HEADER.len();
            let nonce_end = nonce_start + NONCE_SIZE;
            let nonce_array: [u8; NONCE_SIZE] = encrypted[nonce_start..nonce_end]
                .try_into()
                .map_err(|_| Error::InvalidInput("Invalid nonce length".to_string()))?;
            let nonce = Nonce::from(nonce_array);
            let ciphertext = &encrypted[nonce_end..];

            // Decrypt
            let plaintext =
                self.cipher
                    .decrypt(&nonce, ciphertext)
                    .map_err(|e| Error::OperationFailed {
                        operation: "decrypt".to_string(),
                        cause: format!(
                            "AES-256-GCM decryption failed (wrong key or corrupted data): {e}"
                        ),
                    })?;

            tracing::debug!(
                encrypted_len = encrypted.len(),
                plaintext_len = plaintext.len(),
                "Decrypted data"
            );

            Ok(plaintext)
        }
    }

    /// Checks if data appears to be encrypted (has magic header).
    #[must_use]
    pub fn is_encrypted(data: &[u8]) -> bool {
        data.starts_with(MAGIC_HEADER)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn test_config() -> EncryptionConfig {
            // 32 bytes of test key
            EncryptionConfig {
                key: [
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
                    0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
                    0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
                ],
            }
        }

        #[test]
        fn test_encrypt_decrypt_roundtrip() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            let plaintext = b"Hello, World! This is a test of AES-256-GCM encryption.";

            let encrypted = encryptor.encrypt(plaintext).unwrap();
            assert!(is_encrypted(&encrypted));
            assert_ne!(encrypted, plaintext);

            let decrypted = encryptor.decrypt(&encrypted).unwrap();
            assert_eq!(decrypted, plaintext);
        }

        #[test]
        fn test_encrypt_decrypt_empty() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            let plaintext = b"";

            let encrypted = encryptor.encrypt(plaintext).unwrap();
            let decrypted = encryptor.decrypt(&encrypted).unwrap();
            assert_eq!(decrypted, plaintext);
        }

        #[test]
        fn test_encrypt_decrypt_large() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            let plaintext: Vec<u8> = (0u32..10000).map(|i| (i % 256) as u8).collect();

            let encrypted = encryptor.encrypt(&plaintext).unwrap();
            let decrypted = encryptor.decrypt(&encrypted).unwrap();
            assert_eq!(decrypted, plaintext);
        }

        #[test]
        fn test_different_nonces_produce_different_ciphertext() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            let plaintext = b"Same plaintext";

            let encrypted1 = encryptor.encrypt(plaintext).unwrap();
            let encrypted2 = encryptor.encrypt(plaintext).unwrap();

            // Same plaintext should produce different ciphertext due to random nonce
            assert_ne!(encrypted1, encrypted2);

            // Both should decrypt to same plaintext
            let decrypted1 = encryptor.decrypt(&encrypted1).unwrap();
            let decrypted2 = encryptor.decrypt(&encrypted2).unwrap();
            assert_eq!(decrypted1, decrypted2);
        }

        #[test]
        fn test_wrong_key_fails() {
            let encryptor1 = Encryptor::new(test_config()).unwrap();
            let mut wrong_config = test_config();
            wrong_config.key[0] ^= 0xff; // Flip a bit
            let encryptor2 = Encryptor::new(wrong_config).unwrap();

            let plaintext = b"Secret data";
            let encrypted = encryptor1.encrypt(plaintext).unwrap();

            // Decryption with wrong key should fail
            let result = encryptor2.decrypt(&encrypted);
            assert!(result.is_err());
        }

        #[test]
        fn test_tampered_ciphertext_fails() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            let plaintext = b"Secret data";

            let mut encrypted = encryptor.encrypt(plaintext).unwrap();
            // Tamper with the ciphertext
            let last = encrypted.len() - 1;
            encrypted[last] ^= 0xff;

            let result = encryptor.decrypt(&encrypted);
            assert!(result.is_err());
        }

        #[test]
        fn test_is_encrypted() {
            assert!(is_encrypted(MAGIC_HEADER));
            assert!(is_encrypted(b"SUBCOG_ENC_V1\0some_data"));
            assert!(!is_encrypted(b"plain text"));
            assert!(!is_encrypted(b"{}"));
            assert!(!is_encrypted(b""));
        }

        #[test]
        fn test_config_from_base64() {
            // Valid 32-byte key in base64
            let key_b64 = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
            let config = EncryptionConfig::from_base64(key_b64).unwrap();
            assert_eq!(config.key.len(), 32);
        }

        #[test]
        fn test_config_from_base64_invalid() {
            // Too short
            let result = EncryptionConfig::from_base64("AAEC");
            assert!(result.is_err());

            // Invalid base64
            let result = EncryptionConfig::from_base64("not-valid-base64!!!");
            assert!(result.is_err());
        }

        #[test]
        fn test_too_short_encrypted_data() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            let result = encryptor.decrypt(b"too short");
            assert!(result.is_err());
        }

        #[test]
        fn test_missing_magic_header() {
            let encryptor = Encryptor::new(test_config()).unwrap();
            // Long enough but wrong header
            let fake_data = vec![0u8; 100];
            let result = encryptor.decrypt(&fake_data);
            assert!(result.is_err());
        }
    }
}

#[cfg(feature = "encryption")]
pub use implementation::{EncryptionConfig, Encryptor, MAGIC_HEADER, is_encrypted};

// Stub implementations when encryption feature is disabled
#[cfg(not(feature = "encryption"))]
mod stub {
    use crate::{Error, Result};

    /// Encryption configuration (stub).
    #[derive(Debug, Clone)]
    pub struct EncryptionConfig;

    impl EncryptionConfig {
        /// Returns an error indicating encryption is not available.
        ///
        /// # Errors
        ///
        /// Always returns an error.
        pub fn from_env() -> Result<Self> {
            Err(Error::FeatureNotEnabled(
                "encryption feature not compiled".to_string(),
            ))
        }

        /// Always returns None.
        #[must_use]
        pub fn try_from_env() -> Option<Self> {
            None
        }
    }

    /// Encryptor (stub).
    pub struct Encryptor;

    impl Encryptor {
        /// Returns an error indicating encryption is not available.
        ///
        /// # Errors
        ///
        /// Always returns an error.
        pub fn from_env() -> Result<Self> {
            Err(Error::FeatureNotEnabled(
                "encryption feature not compiled".to_string(),
            ))
        }
    }

    /// Checks if data appears to be encrypted.
    #[must_use]
    pub fn is_encrypted(data: &[u8]) -> bool {
        data.starts_with(b"SUBCOG_ENC_V1\0")
    }
}

#[cfg(not(feature = "encryption"))]
pub use stub::{EncryptionConfig, Encryptor, is_encrypted};
