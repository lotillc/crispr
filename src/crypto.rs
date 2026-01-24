//! Cryptographic utilities for CRISPR
//!
//! Provides AES-256-GCM encryption for sensitive data like API keys and tokens.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;

/// Encryption key for AES-256-GCM
#[derive(Clone)]
pub struct EncryptionKey {
    key: Key<Aes256Gcm>,
}

impl EncryptionKey {
    /// Create a new encryption key from a base64-encoded string
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let decoded = BASE64.decode(encoded)?;
        if decoded.len() != KEY_SIZE {
            return Err(anyhow!(
                "Invalid key size: expected {}, got {}",
                KEY_SIZE,
                decoded.len()
            ));
        }

        let key = *Key::<Aes256Gcm>::from_slice(&decoded);
        Ok(Self { key })
    }

    /// Generate a new random encryption key
    pub fn generate() -> Self {
        let key = Aes256Gcm::generate_key(OsRng);
        Self { key }
    }

    /// Export the key as a base64-encoded string
    pub fn to_base64(&self) -> String {
        BASE64.encode(self.key.as_slice())
    }

    /// Encrypt plaintext and return base64-encoded ciphertext
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(&self.key);

        // Generate random nonce
        let nonce_bytes: [u8; NONCE_SIZE] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Prepend nonce to ciphertext and encode
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);

        Ok(BASE64.encode(&result))
    }

    /// Decrypt base64-encoded ciphertext and return plaintext
    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(&self.key);

        // Decode from base64
        let data = BASE64.decode(encrypted)?;

        if data.len() < NONCE_SIZE {
            return Err(anyhow!("Ciphertext too short"));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
    }
}

/// Encrypted value wrapper for storing sensitive data
#[derive(Debug, Clone)]
pub struct EncryptedValue {
    pub ciphertext: String,
}

impl EncryptedValue {
    /// Create a new encrypted value
    pub fn new(key: &EncryptionKey, plaintext: &str) -> Result<Self> {
        let ciphertext = key.encrypt(plaintext)?;
        Ok(Self { ciphertext })
    }

    /// Decrypt the value
    pub fn decrypt(&self, key: &EncryptionKey) -> Result<String> {
        key.decrypt(&self.ciphertext)
    }
}

/// Hash a password using Argon2id
pub fn hash_password(password: &str) -> Result<String> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2,
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("Password hashing failed: {}", e))?;

    Ok(hash.to_string())
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };

    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| anyhow!("Invalid password hash: {}", e))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = EncryptionKey::generate();
        let plaintext = "my-secret-api-key-12345";

        let encrypted = key.encrypt(plaintext).unwrap();
        let decrypted = key.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
        assert_ne!(plaintext, encrypted); // Should be different
    }

    #[test]
    fn test_key_serialization() {
        let key = EncryptionKey::generate();
        let encoded = key.to_base64();

        let restored = EncryptionKey::from_base64(&encoded).unwrap();
        let plaintext = "test-value";

        let encrypted = key.encrypt(plaintext).unwrap();
        let decrypted = restored.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypted_value() {
        let key = EncryptionKey::generate();
        let secret = "super-secret-token";

        let encrypted = EncryptedValue::new(&key, secret).unwrap();
        let decrypted = encrypted.decrypt(&key).unwrap();

        assert_eq!(secret, decrypted);
    }

    #[test]
    fn test_password_hashing() {
        let password = "my-secure-password";

        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong-password", &hash).unwrap());
    }
}
