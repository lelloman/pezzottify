//! Authentication and authorization
#![allow(dead_code)] // Challenge system for future use

use anyhow::{bail, Result};

use rand::Rng;
use rand_distr::Alphanumeric;
use serde::{Deserialize, Serialize};

use std::str::FromStr;
use std::time::SystemTime;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct AuthTokenValue(pub String);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthToken {
    pub user_id: usize,
    pub device_id: Option<usize>,
    pub created: SystemTime,
    pub last_used: Option<SystemTime>,
    pub value: AuthTokenValue,
}

impl AuthTokenValue {
    pub fn generate() -> AuthTokenValue {
        let rng = rand::rng();
        let random_string: String = rng
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        AuthTokenValue(random_string)
    }
}

mod pezzottify_argon2 {
    use anyhow::{anyhow, Result};
    use argon2::{
        password_hash::{
            rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        },
        Argon2,
    };

    pub fn generate_b64_salt() -> String {
        SaltString::generate(&mut OsRng).to_string()
    }

    pub fn hash<T: AsRef<str>>(plain: &[u8], b64_salt: T) -> Result<String> {
        let argon2 = Argon2::default();
        let salt = SaltString::from_b64(b64_salt.as_ref()).map_err(|err| anyhow!("{}", err))?;
        let hash_string = argon2
            .hash_password(plain, &salt)
            .map_err(|err| anyhow!("{}", err))?
            /*.hash
            .with_context(|| "asd")?*/
            .to_string();
        Ok(hash_string)
    }

    pub fn verify<T: AsRef<str>>(plain_pw: &[u8], target_hash: T) -> Result<bool> {
        let argon2 = Argon2::default();
        let password_hash =
            PasswordHash::new(target_hash.as_ref()).map_err(|err| anyhow!("{}", err))?;
        Ok(argon2.verify_password(plain_pw, &password_hash).is_ok())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum PezzottifyHasher {
    Argon2,
}

impl FromStr for PezzottifyHasher {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "argon2" => Ok(PezzottifyHasher::Argon2),
            _ => bail!("Unknown hasher {}", s),
        }
    }
}

impl ToString for PezzottifyHasher {
    fn to_string(&self) -> String {
        match self {
            PezzottifyHasher::Argon2 => "argon2".to_string(),
        }
    }
}

impl PezzottifyHasher {
    pub fn generate_b64_salt(&self) -> String {
        match self {
            PezzottifyHasher::Argon2 => pezzottify_argon2::generate_b64_salt(),
        }
    }
    pub fn hash<T: AsRef<str>>(&self, plain: &[u8], b64_salt: T) -> Result<String> {
        match self {
            PezzottifyHasher::Argon2 => pezzottify_argon2::hash(plain, b64_salt),
        }
    }

    pub fn verify<T: AsRef<str>>(&self, plain_pw: T, target_hash: T, _salt: T) -> Result<bool> {
        match self {
            PezzottifyHasher::Argon2 => {
                pezzottify_argon2::verify(plain_pw.as_ref().as_bytes(), target_hash)
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CryptoKeyKind {
    Rsa,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActiveChallenge {
    pub nonce: String,
    pub sent_at: Option<SystemTime>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UsernamePasswordCredentials {
    pub user_id: usize,
    pub salt: String,
    pub hash: String,
    pub hasher: PezzottifyHasher,

    pub created: SystemTime,
    pub last_tried: Option<SystemTime>,
    pub last_used: Option<SystemTime>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CryptoKeyCredentials {
    name: String,
    kind: CryptoKeyKind,
    pub_key: String,

    created: SystemTime,
    last_tried: Option<SystemTime>,
    last_used: Option<SystemTime>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserAuthCredentials {
    pub user_id: usize,
    pub username_password: Option<UsernamePasswordCredentials>,
    pub keys: Vec<CryptoKeyCredentials>,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn argon2_hash() {
        let pw = "123mypw";
        let b64_salt = PezzottifyHasher::Argon2.generate_b64_salt();

        println!("salt: \"{}\"", &b64_salt);

        let hash1 = PezzottifyHasher::Argon2
            .hash(pw.as_bytes(), &b64_salt)
            .unwrap();
        println!("hash1: \"{}\"", &hash1);

        let hash2 = PezzottifyHasher::Argon2
            .hash(b"123mypw", &b64_salt)
            .unwrap();
        println!("hash2: \"{}\"", hash2);
        assert_eq!(hash1, hash2);

        assert!(PezzottifyHasher::Argon2
            .verify("123mypw", &hash1, "unusued")
            .unwrap());
        assert!(!PezzottifyHasher::Argon2
            .verify("not the pw", &hash1, "unusued")
            .unwrap());
    }
}
