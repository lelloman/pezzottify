use crate::catalog::{self, Catalog};

use super::{
    auth::PezzottifyHasher, user_models::LikedContentType, AuthToken, AuthTokenValue,
    UserAuthCredentials, UserPlaylist, UserStore, UsernamePasswordCredentials,
};
use anyhow::{bail, Context, Result};
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct UserManager {
    catalog: Arc<Mutex<Catalog>>,
    user_store: Arc<Mutex<Box<dyn UserStore>>>,
}

impl UserManager {
    pub fn new(catalog: Arc<Mutex<Catalog>>, user_store: Box<dyn UserStore>) -> Self {
        Self {
            catalog,
            user_store: Arc::new(Mutex::new(user_store)),
        }
    }

    pub fn set_user_liked_content(
        &self,
        user_id: usize,
        content_id: &str,
        liked: bool,
    ) -> anyhow::Result<()> {
        let content_type = LikedContentType::from_id(content_id);
        self.user_store.lock().unwrap().set_user_liked_content(
            user_id,
            content_id,
            content_type,
            liked,
        )
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

    pub fn get_user_liked_content(
        &self,
        user_id: usize,
        conten_type: LikedContentType,
    ) -> Result<Vec<String>> {
        self.user_store
            .lock()
            .unwrap()
            .get_user_liked_content(user_id, conten_type)
    }

    pub fn create_user_playlist(
        &self,
        user_id: usize,
        playlist_name: &str,
        creator_id: usize,
        track_ids: Vec<String>,
    ) -> Result<String> {
        self.user_store.lock().unwrap().create_user_playlist(
            user_id,
            playlist_name,
            creator_id,
            track_ids,
        )
    }

    pub fn update_user_playlist(
        &self,
        playlist_id: &str,
        user_id: usize,
        playlist_name: Option<String>,
        track_ids: Option<Vec<String>>,
    ) -> Result<()> {
        self.user_store.lock().unwrap().update_user_playlist(
            playlist_id,
            user_id,
            playlist_name,
            track_ids,
        )
    }

    pub fn delete_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<()> {
        self.user_store
            .lock()
            .unwrap()
            .delete_user_playlist(playlist_id, user_id)
    }

    pub fn get_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<UserPlaylist> {
        self.user_store
            .lock()
            .unwrap()
            .get_user_playlist(playlist_id, user_id)
    }

    pub fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>> {
        self.user_store.lock().unwrap().get_user_playlists(user_id)
    }

    pub fn add_playlist_tracks(
        &self,
        playlist_id: &str,
        user_id: usize,
        track_ids: Vec<String>,
    ) -> Result<()> {
        let playlist = self
            .user_store
            .lock()
            .unwrap()
            .get_user_playlist(playlist_id, user_id)?;

        // verify that all tracks to add exist
        for track_id in &track_ids {
            if let None = self.catalog.lock().unwrap().get_track(track_id) {
                bail!("Track with id {} does not exist.", track_id);
            }
        }

        let mut new_tracks = playlist.tracks.clone();
        new_tracks.extend(track_ids);
        self.update_user_playlist(playlist_id, user_id, None, Some(new_tracks))
    }
}
