use anyhow::Result;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use std::{collections::HashMap, time::SystemTime};

pub type UserId = String;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthTokenValue(String);

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub user_id: UserId,
    pub created: SystemTime,
    pub last_used: SystemTime,
    pub value: AuthTokenValue,
}

impl AuthTokenValue {
    fn generate() -> AuthTokenValue {
        let rng = thread_rng();
        let random_string: String = rng
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        AuthTokenValue(random_string)
    }
}

pub trait AuthStore {
    fn load_auth_credentials(&self) -> Result<HashMap<UserId, Vec<AuthCredentials>>>;
    fn update_auth_credentials(&self, credentials: AuthCredentials) -> Result<()>;

    fn load_challenges(&self) -> Result<Vec<ActiveChallenge>>;
    fn delete_challenge(&self, challenge: ActiveChallenge) -> Result<()>;
    fn flag_sent_challenge(&self, challenge: &ActiveChallenge) -> Result<()>;
    fn add_challenges(&self, challenges: Vec<ActiveChallenge>) -> Result<()>;

    fn load_auth_tokens(&self) -> Result<HashMap<AuthTokenValue, AuthToken>>;
    fn delete_auth_token(&self, value: AuthTokenValue) -> Result<()>;
    fn update_auth_token(&self, token: &AuthToken) -> Result<()>;
    fn add_auth_token(&self, token: &AuthToken) -> Result<()>;
}

pub struct AuthManager {
    store: Box<dyn AuthStore>,
    credentials: HashMap<UserId, Vec<AuthCredentials>>,
    active_challenges: Vec<ActiveChallenge>,
    auth_tokens: HashMap<AuthTokenValue, AuthToken>,
}

impl AuthManager {
    pub fn initialize(store: Box<dyn AuthStore>) -> Result<AuthManager> {
        let credentials = store.load_auth_credentials()?;
        let active_challenges = store.load_challenges()?;
        let auth_tokens = store.load_auth_tokens()?;
        Ok(AuthManager {
            store,
            credentials,
            active_challenges,
            auth_tokens,
        })
    }

    pub fn get_auth_token(&self, value: &AuthTokenValue) -> Option<AuthToken> {
        self.auth_tokens.get(value).cloned()
    }

    pub fn generate_auth_token(&mut self, credentials: AuthCredentials) -> Result<AuthTokenValue> {
        todo!()
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

#[derive(Clone, Serialize, Deserialize)]
pub enum PezzottifyHasher {
    Argon2,
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

    pub fn verify<T: AsRef<str>>(&self, plain_pw: T, target_hash: T, salt: T) -> Result<bool> {
        match self {
            PezzottifyHasher::Argon2 => {
                pezzottify_argon2::verify(plain_pw.as_ref().as_bytes(), target_hash)
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CryptoKeyKind {
    Rsa,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActiveChallenge {
    pub nonce: String,
    pub sent_at: Option<SystemTime>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthCredentialsInfo {
    pub user_id: String,
    pub created: SystemTime,
    pub last_tried: SystemTime,
    pub last_used: SystemTime,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AuthCredentialsMethod {
    UsernamePassword {
        salt: String,
        hash: String,
        hasher: PezzottifyHasher,
    },
    CryptoKey {
        kind: CryptoKeyKind,
        pub_key: String,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    pub info: AuthCredentialsInfo,
    pub method: AuthCredentialsMethod,
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
