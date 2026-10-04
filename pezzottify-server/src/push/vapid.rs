//! VAPID (RFC 8292) key handling.

use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::pkcs8::DecodePrivateKey;
use p256::SecretKey;
use rand::RngCore;
use std::path::Path;

/// The server's VAPID key pair, ready for signing and for publishing the public key.
#[derive(Clone)]
pub struct VapidKeys {
    /// Raw 32-byte private scalar, base64url without padding (web-push's format).
    private_b64: String,
    /// Uncompressed 65-byte public point, base64url without padding (87 chars).
    public_b64: String,
}

impl std::fmt::Debug for VapidKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VapidKeys")
            .field("public_b64", &self.public_b64)
            .finish_non_exhaustive()
    }
}

impl VapidKeys {
    /// Load the PEM key at `path` (SEC1 or PKCS#8), generating and storing a new
    /// one (mode 0600) when the file does not exist.
    pub fn load_or_generate(path: &Path) -> Result<Self> {
        let secret = if path.exists() {
            let pem = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read VAPID key {}", path.display()))?;
            SecretKey::from_sec1_pem(&pem)
                .or_else(|_| SecretKey::from_pkcs8_pem(&pem))
                .map_err(|e| anyhow::anyhow!("Invalid VAPID key {}: {e}", path.display()))?
        } else {
            let secret = generate_secret();
            let pem = secret
                .to_sec1_pem(p256::pkcs8::LineEnding::LF)
                .map_err(|e| anyhow::anyhow!("Failed to encode VAPID key: {e}"))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            write_private(path, pem.as_bytes())
                .with_context(|| format!("Failed to write VAPID key {}", path.display()))?;
            tracing::info!("Generated VAPID key at {}", path.display());
            secret
        };
        Ok(Self::from_secret(&secret))
    }

    fn from_secret(secret: &SecretKey) -> Self {
        let public = secret.public_key().to_encoded_point(false);
        Self {
            private_b64: URL_SAFE_NO_PAD.encode(secret.to_bytes()),
            public_b64: URL_SAFE_NO_PAD.encode(public.as_bytes()),
        }
    }

    /// Public key for clients (`GET /v1/push/vapid`).
    pub fn public_key(&self) -> &str {
        &self.public_b64
    }

    pub(crate) fn private_key_b64(&self) -> &str {
        &self.private_b64
    }
}

fn generate_secret() -> SecretKey {
    let mut bytes = [0u8; 32];
    loop {
        rand::rng().fill_bytes(&mut bytes);
        // Rejects zero and values at or above the curve order; retry is ~2^-32 likely.
        if let Ok(secret) = SecretKey::from_slice(&bytes) {
            return secret;
        }
    }
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents)
}

#[cfg(not(unix))]
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_once_then_reloads_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys/vapid.pem");
        let first = VapidKeys::load_or_generate(&path).unwrap();
        let second = VapidKeys::load_or_generate(&path).unwrap();
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(first.private_key_b64(), second.private_key_b64());
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("-----BEGIN EC PRIVATE KEY-----"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn public_key_is_an_uncompressed_point_in_base64url() {
        let dir = tempfile::tempdir().unwrap();
        let keys = VapidKeys::load_or_generate(&dir.path().join("vapid.pem")).unwrap();
        assert_eq!(keys.public_key().len(), 87);
        let bytes = URL_SAFE_NO_PAD.decode(keys.public_key()).unwrap();
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[0], 0x04);
        assert_eq!(URL_SAFE_NO_PAD.decode(keys.private_key_b64()).unwrap().len(), 32);
    }

    #[test]
    fn accepts_pkcs8_keys_and_rejects_garbage() {
        use p256::pkcs8::EncodePrivateKey;
        let dir = tempfile::tempdir().unwrap();
        let secret = generate_secret();
        let pkcs8 = secret.to_pkcs8_pem(p256::pkcs8::LineEnding::LF).unwrap();
        let path = dir.path().join("pkcs8.pem");
        std::fs::write(&path, pkcs8.as_bytes()).unwrap();
        let keys = VapidKeys::load_or_generate(&path).unwrap();
        assert_eq!(keys.public_key(), VapidKeys::from_secret(&secret).public_key());

        let bad = dir.path().join("bad.pem");
        std::fs::write(&bad, "not a key").unwrap();
        assert!(VapidKeys::load_or_generate(&bad).is_err());
    }
}
