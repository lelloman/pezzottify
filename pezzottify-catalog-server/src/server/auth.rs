use anyhow::{bail, Context, Result};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use std::{collections::HashMap, sync::Mutex, time::SystemTime};

use super::user;

pub type UserId = String;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub struct AuthTokenValue(pub String);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthToken {
    pub user_id: UserId,
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

pub trait AuthStore: Send + Sync {
    fn load_auth_credentials(&self) -> Result<HashMap<UserId, UserAuthCredentials>>;
    fn update_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()>;

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
    store: Mutex<Box<dyn AuthStore>>,
    credentials: HashMap<UserId, UserAuthCredentials>,
    active_challenges: Vec<ActiveChallenge>,
    auth_tokens: HashMap<AuthTokenValue, AuthToken>,
}

impl AuthManager {
    pub fn initialize(store: Box<dyn AuthStore>) -> Result<AuthManager> {
        let credentials = store.load_auth_credentials()?;
        let active_challenges = store.load_challenges()?;
        let auth_tokens = store.load_auth_tokens()?;
        Ok(AuthManager {
            store: Mutex::new(store),
            credentials,
            active_challenges,
            auth_tokens,
        })
    }

    pub fn get_auth_token(&self, value: &AuthTokenValue) -> Option<AuthToken> {
        self.auth_tokens.get(value).cloned()
    }

    pub fn generate_auth_token(&mut self, credentials: &UserAuthCredentials) -> Result<AuthToken> {
        let token = AuthToken {
            user_id: credentials.user_id.clone(),
            value: AuthTokenValue::generate(),
            created: SystemTime::now(),
            last_used: None,
        };
        self.store.lock().unwrap().add_auth_token(&token)?;
        Ok(token)
    }

    fn create_hashed_password(password: String) -> Result<UsernamePasswordCredentials> {
        let hasher = PezzottifyHasher::Argon2;
        let salt = hasher.generate_b64_salt();
        let hash = hasher.hash(password.as_bytes(), &salt)?;
        Ok(UsernamePasswordCredentials {
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
        user_id: &UserId,
        password: String,
    ) -> Result<()> {
        if let Some(true) = self
            .credentials
            .get(user_id)
            .map(|x| x.username_password.is_some())
        {
            bail!("User with id {} already has password credentials method. Maybe you want to modify it?", user_id);
        }

        let new_credentials =
            self.credentials
                .entry(user_id.clone())
                .or_insert_with(|| UserAuthCredentials {
                    user_id: user_id.clone(),
                    username_password: None,
                    keys: vec![],
                });
        new_credentials.username_password = Some(Self::create_hashed_password(password)?);

        self.store
            .lock()
            .unwrap()
            .update_auth_credentials(new_credentials.clone())
    }

    pub fn update_password_credentials(
        &mut self,
        user_id: &UserId,
        password: String,
    ) -> Result<()> {
        let credentials = self
            .credentials
            .get_mut(user_id)
            .with_context(|| format!("User with id {} not found.", user_id))?;
        if let None = credentials.username_password {
            bail!(
                "Cannot update passowrd of user with id {} since it never had one.",
                user_id
            );
        }
        credentials.username_password = Some(Self::create_hashed_password(password)?);
        self.store
            .lock()
            .unwrap()
            .update_auth_credentials(credentials.clone())
    }

    pub fn delete_password_credentials(&mut self, user_id: &UserId) -> Result<()> {
        let credentials = self
            .credentials
            .get_mut(user_id)
            .with_context(|| format!("User with id {} not found.", user_id))?;
        credentials.username_password = None;
        self.store
            .lock()
            .unwrap()
            .update_auth_credentials(credentials.clone())
    }

    pub fn get_user_credentials(&self, user_id: &UserId) -> Option<UserAuthCredentials> {
        self.credentials.get(user_id).cloned()
    }

    pub fn get_user_tokens(&self, user_id: &UserId) -> Vec<AuthToken> {
        self.auth_tokens
            .iter()
            .filter(|(_, v)| &v.user_id == user_id)
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn get_all_user_ids(&self) -> Vec<UserId> {
        self.credentials.keys().cloned().collect()
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
    pub user_id: UserId,
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
