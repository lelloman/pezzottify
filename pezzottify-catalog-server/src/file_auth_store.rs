use crate::server::{
    ActiveChallenge, AuthCredentials, AuthCredentialsMethod, AuthStore, AuthToken, AuthTokenValue,
    UserId,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::HashMap, fs::File, io::Read, path::PathBuf};

pub struct FileAuthStore {
    file_path: PathBuf,
    dump: RefCell<Option<Dump>>,
}

#[derive(Serialize, Deserialize, Default)]
struct Dump {
    auth_credentials: HashMap<UserId, Vec<AuthCredentials>>,
    active_challenges: Vec<ActiveChallenge>,
    auth_tokens: HashMap<AuthTokenValue, AuthToken>,
}

impl FileAuthStore {
    pub fn new(file_path: PathBuf) -> FileAuthStore {
        FileAuthStore {
            file_path,
            dump: RefCell::new(None),
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

    fn load_dump_from_file(&self) -> Result<()> {
        let mut file = File::open("data.json")?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let data: Dump = serde_json::from_str(&content)?;
        self.dump.replace(Some(data));
        Ok(())
    }

    fn load_dump(&self) -> Result<()> {
        if let Err(_) = self.load_dump_from_file() {            
            self.dump.replace(Some(Dump::default()));            
        }
        Ok(())
    }

    fn save_dump(&self, dump: &Dump) -> Result<()> {
        todo!();
    }

    fn with_loaded_dump<F, O>(&self, f: F) -> Result<O>
    where
        F: Fn(&Dump) -> Result<O>,
    {
        let tmp_dump = self.dump.borrow();
        if let None = *tmp_dump {
            drop(tmp_dump);
            self.load_dump()?;
        }
        let dump = self.dump.borrow_mut();

        match dump.as_ref() {
            None => anyhow::bail!(""),
            Some(dump) => Ok(f(dump)?),
        }
    }

    fn update_dump<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Dump) -> Result<()>,
    {
        if let None = *self.dump.borrow() {
            self.load_dump()?;
        }
        let mut dump = self.dump.borrow_mut();

        match dump.as_mut() {
            None => anyhow::bail!("No loaded dump."),
            Some(dump) => {
                f(dump)?;
                self.save_dump(dump)
            }
        }
    }
}

impl AuthStore for FileAuthStore {
    fn load_auth_credentials(&self) -> Result<HashMap<UserId, Vec<AuthCredentials>>> {
        self.with_loaded_dump(|dump| Ok(dump.auth_credentials.clone()))
    }
    fn update_auth_credentials(&self, credentials: AuthCredentials) -> Result<()> {
        self.update_dump(|dump| {
            let old_credentials: &mut Vec<AuthCredentials> =
                match dump.auth_credentials.get_mut(&credentials.info.user_id) {
                    Some(x) => x,
                    None => &mut Vec::new(),
                };
            match &credentials.method {
                AuthCredentialsMethod::UsernamePassword { .. } => {
                    old_credentials.retain(|m| match m.method {
                        AuthCredentialsMethod::CryptoKey { .. } => true,
                        _ => false,
                    });
                    old_credentials.push(credentials);
                }
                AuthCredentialsMethod::CryptoKey { kind, pub_key } => {
                    bail!("Cannot update CryptoKey. Just delete or create a new one.")
                }
            }
            Ok(())
        })
    }

    fn load_challenges(&self) -> Result<Vec<ActiveChallenge>> {
        self.with_loaded_dump(|dump| Ok(dump.active_challenges.clone()))
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
        self.with_loaded_dump(|dump| Ok(dump.auth_tokens.clone()))
    }
    fn delete_auth_token(&self, value: AuthTokenValue) -> Result<()> {
        todo!()
    }
    fn update_auth_token(&self, token: &AuthToken) -> Result<()> {
        todo!()
    }
    fn add_auth_token(&self, token: &AuthToken) -> Result<()> {
        todo!()
    }
}
