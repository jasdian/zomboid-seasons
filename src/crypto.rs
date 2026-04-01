use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::error::{AppError, AppResult};

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

/// AES-256-GCM cipher for encrypting passwords at rest.
///
/// Encrypted format: base64(nonce || ciphertext || tag)
/// Generate a key with: `openssl rand -base64 32`
#[derive(Clone)]
pub struct Cipher {
    inner: Aes256Gcm,
}

impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cipher(***)")
    }
}

impl Cipher {
    /// Create from a base64-encoded 32-byte key.
    pub fn from_base64_key(key_b64: &str) -> AppResult<Self> {
        let key_bytes = STANDARD
            .decode(key_b64.trim())
            .map_err(|e| AppError::Config(format!("invalid encryption_key (bad base64): {e}")))?;
        if key_bytes.len() != 32 {
            return Err(AppError::Config(format!(
                "encryption_key must be 32 bytes (base64 of 32 bytes), got {} bytes. \
                 Generate with: openssl rand -base64 32",
                key_bytes.len()
            )));
        }
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Self {
            inner: Aes256Gcm::new(key),
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .inner
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Config(format!("password encryption failed: {e}")))?;

        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(STANDARD.encode(&combined))
    }

    pub fn decrypt(&self, encrypted_b64: &str) -> AppResult<String> {
        let combined = STANDARD.decode(encrypted_b64).map_err(|e| {
            AppError::Config(format!("invalid encrypted password (bad base64): {e}"))
        })?;

        if combined.len() < NONCE_LEN + TAG_LEN {
            return Err(AppError::Config("encrypted password data too short".into()));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .inner
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Config(format!("password decryption failed: {e}")))?;

        String::from_utf8(plaintext)
            .map_err(|e| AppError::Config(format!("decrypted password is not valid UTF-8: {e}")))
    }

    /// Try to decrypt; if it fails, assume it's still plaintext (pre-migration).
    pub fn decrypt_or_plaintext(&self, value: &str) -> String {
        self.decrypt(value).unwrap_or_else(|_| value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cipher() -> Cipher {
        // 32 bytes of zeros in base64
        Cipher::from_base64_key("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let cipher = test_cipher();
        let plaintext = "my_secret_password";
        let encrypted = cipher.encrypt(plaintext).unwrap();
        assert_ne!(encrypted, plaintext);
        let decrypted = cipher.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_or_plaintext_returns_plaintext_for_non_encrypted() {
        let cipher = test_cipher();
        let plain = "just_a_password";
        assert_eq!(cipher.decrypt_or_plaintext(plain), plain);
    }

    #[test]
    fn different_encryptions_produce_different_ciphertext() {
        let cipher = test_cipher();
        let plaintext = "password123";
        let a = cipher.encrypt(plaintext).unwrap();
        let b = cipher.encrypt(plaintext).unwrap();
        assert_ne!(a, b); // different nonces
        assert_eq!(cipher.decrypt(&a).unwrap(), plaintext);
        assert_eq!(cipher.decrypt(&b).unwrap(), plaintext);
    }

    #[test]
    fn invalid_key_length_rejected() {
        let short_key = STANDARD.encode([0u8; 16]); // 16 bytes, not 32
        assert!(Cipher::from_base64_key(&short_key).is_err());
    }
}
