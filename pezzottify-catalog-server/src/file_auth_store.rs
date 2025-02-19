use crate::user::auth::{
    ActiveChallenge, AuthStore, AuthToken, AuthTokenValue, UserAuthCredentials,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
};

#[derive(Serialize, Deserialize, Default)]
struct Dump {
    auth_credentials: HashMap<String, UserAuthCredentials>,
    active_challenges: Vec<ActiveChallenge>,
    auth_tokens: HashMap<AuthTokenValue, AuthToken>,
}

pub struct FileAuthStore {
    file_path: PathBuf,
    dump: Mutex<Dump>,
}

impl FileAuthStore {
    fn load_dump_from_file(file_path: &PathBuf) -> Result<Dump> {
        let mut file = File::open(file_path)?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        Ok(serde_json::from_str(&content)?)
    }

    pub fn initialize(file_path: PathBuf) -> FileAuthStore {
        FileAuthStore {
            file_path: file_path.clone(),
            dump: Mutex::new(Self::load_dump_from_file(&file_path).unwrap_or_default()),
        }
    }

    pub fn infer_path() -> Option<PathBuf> {
        let mut current_dir = std::env::current_dir().ok()?;

        loop {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    if path.is_file() {
                        if let Some(s) = path.file_name() {
                            if s.to_string_lossy() == "auth_store.json" {
                                return Some(s.into());
                            }
                        }
                    }
                }
            }

            if let Some(parent) = current_dir.parent() {
                current_dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        None
    }

    fn save_dump(&self) -> Result<()> {
        let json_string = serde_json::to_string_pretty(&*self.dump.lock().unwrap())?;
        let mut file = File::create(&self.file_path)?;
        file.write_all(json_string.as_bytes())?;
        Ok(())
    }
}

impl AuthStore for FileAuthStore {
    fn load_auth_credentials(&self) -> Result<HashMap<String, UserAuthCredentials>> {
        Ok(self.dump.lock().unwrap().auth_credentials.clone())
    }
    fn update_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()> {
        self.dump
            .lock()
            .unwrap()
            .auth_credentials
            .insert(credentials.user_id.clone(), credentials);
        self.save_dump()
    }

    fn load_challenges(&self) -> Result<Vec<ActiveChallenge>> {
        Ok(self.dump.lock().unwrap().active_challenges.clone())
    }
    fn delete_challenge(&self, challenge: ActiveChallenge) -> Result<()> {
        todo!()
    }
    fn flag_sent_challenge(&self, challenge: &ActiveChallenge) -> Result<()> {
        todo!()
    }
    fn add_challenges(&self, challenges: Vec<ActiveChallenge>) -> Result<()> {
        todo!()
    }

    fn load_auth_tokens(&self) -> Result<HashMap<AuthTokenValue, AuthToken>> {
        Ok(self.dump.lock().unwrap().auth_tokens.clone())
    }
    fn delete_auth_token(&self, value: AuthTokenValue) -> Result<()> {
        todo!()
    }
    fn update_auth_token(&self, token: &AuthToken) -> Result<()> {
        todo!()
    }
    fn add_auth_token(&self, token: &AuthToken) -> Result<()> {
        self.dump
            .lock()
            .unwrap()
            .auth_tokens
            .insert(token.value.clone(), token.clone());
        self.save_dump()
    }
}
