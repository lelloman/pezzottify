use anyhow::{bail, Context, Result};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use std::str::FromStr;
use std::{collections::HashMap, sync::Mutex, time::SystemTime};

use super::UserStore;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct AuthTokenValue(pub String);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthToken {
    pub user_id: usize,
    pub created: SystemTime,
    pub last_used: Option<SystemTime>,
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

pub struct AuthManager {
    user_store: Mutex<Box<dyn UserStore>>,
}

impl AuthManager {
    pub fn initialize(user_store: Box<dyn UserStore>) -> Result<AuthManager> {
        Ok(AuthManager {
            user_store: Mutex::new(user_store),
        })
    }

    pub fn get_auth_token(&self, value: &AuthTokenValue) -> Option<AuthToken> {
        self.user_store.lock().unwrap().get_user_auth_token(value)
    }

    pub fn generate_auth_token(&mut self, credentials: &UserAuthCredentials) -> Result<AuthToken> {
        let token = AuthToken {
            user_id: credentials.user_id.clone(),
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        self.user_store
            .lock()
            .unwrap()
            .add_user_auth_token(token.clone())?;
        Ok(token)
    }

    fn create_hashed_password(
        user_id: usize,
        password: String,
    ) -> Result<UsernamePasswordCredentials> {
        let hasher = PezzottifyHasher::Argon2;
        let salt = hasher.generate_b64_salt();
        let hash = hasher.hash(password.as_bytes(), &salt)?;
        Ok(UsernamePasswordCredentials {
            user_id,
            salt,
            hash,
            hasher,
            created: SystemTime::now(),
            last_tried: None,
            last_used: None,
        })
    }

    pub fn create_password_credentials(
        &mut self,
        user_handle: &String,
        password: String,
    ) -> Result<()> {
        let user_store = self.user_store.lock().unwrap();
        if let Some(true) = user_store
            .get_user_auth_credentials(user_handle)
            .map(|x| x.username_password.is_some())
        {
            bail!("User with handle {} already has password credentials method. Maybe you want to modify it?", user_handle);
        }

        let user_id = user_store
            .get_user_id(&user_handle)
            .with_context(|| format!("User with handle {} not found.", user_handle))?;

        let mut new_credentials = user_store
            .get_user_auth_credentials(user_handle)
            .unwrap_or_else(|| UserAuthCredentials {
                user_id,
                username_password: None,
                keys: vec![],
            });
        new_credentials.username_password = Some(Self::create_hashed_password(user_id, password)?);

        user_store.update_user_auth_credentials(new_credentials.clone())
    }

    pub fn update_password_credentials(
        &mut self,
        user_handle: &String,
        password: String,
    ) -> Result<()> {
        let user_store = self.user_store.lock().unwrap();
        let mut credentials = user_store
            .get_user_auth_credentials(user_handle)
            .with_context(|| format!("User with handle {} not found.", user_handle))?;
        if let None = credentials.username_password {
            bail!(
                "Cannot update passowrd of user with handle {} since it never had one.",
                user_handle
            );
        }
        credentials.username_password =
            Some(Self::create_hashed_password(credentials.user_id, password)?);
        user_store.update_user_auth_credentials(credentials.clone())
    }

    pub fn delete_password_credentials(&mut self, user_handle: &String) -> Result<()> {
        let mut credentials = self
            .user_store
            .lock()
            .unwrap()
            .get_user_auth_credentials(user_handle)
            .with_context(|| format!("User with handle {} not found.", user_handle))?;
        credentials.username_password = None;
        self.user_store
            .lock()
            .unwrap()
            .update_user_auth_credentials(credentials.clone())
    }

    pub fn get_user_credentials(&self, user_handle: &String) -> Option<UserAuthCredentials> {
        self.user_store
            .lock()
            .unwrap()
            .get_user_auth_credentials(user_handle)
    }

    pub fn delete_auth_token(
        &mut self,
        user_id: &usize,
        token_value: &AuthTokenValue,
    ) -> Result<()> {
        let removed = self
            .user_store
            .lock()
            .unwrap()
            .delete_user_auth_token(token_value);
        match removed {
            Some(removed) => {
                if &removed.user_id == user_id {
                    Ok(())
                } else {
                    self.user_store
                        .lock()
                        .unwrap()
                        .add_user_auth_token(removed.clone());
                    bail!("Tried to delete auth token {}, but the authenticated user {} was not the owner {} of the token.", token_value.0, user_id, &removed.user_id)
                }
            }
            None => bail!("Did not found auth token {}", token_value.0),
        }
    }

    pub fn get_user_tokens(&self, user_handle: &String) -> Vec<AuthToken> {
        self.user_store
            .lock()
            .unwrap()
            .get_all_user_auth_tokens(user_handle)
    }

    pub fn get_all_user_handles(&self) -> Vec<String> {
        self.user_store.lock().unwrap().get_all_user_handles()
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

    pub fn verify<T: AsRef<str>>(&self, plain_pw: T, target_hash: T, salt: T) -> Result<bool> {
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
