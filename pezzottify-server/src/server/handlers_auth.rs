async fn login(
    State(config): State<ServerConfig>,
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<LoginBody>,
) -> Response {
    let start = Instant::now();

    // Check if password auth is disabled
    if config.disable_password_auth {
        warn!("Password authentication is disabled");
        super::metrics::record_login_attempt("disabled", start.elapsed());
        return (
            StatusCode::FORBIDDEN,
            "Password authentication is disabled. Please use OIDC authentication.",
        )
            .into_response();
    }

    // 1. Validate device info first (fail fast)
    let device_registration = match DeviceRegistration::validate_and_sanitize(
        &body.device_uuid,
        &body.device_type,
        body.device_name.as_deref(),
        body.os_info.as_deref(),
    ) {
        Ok(reg) => reg,
        Err(e) => {
            warn!("Invalid device info in login request: {}", e);
            super::metrics::record_login_attempt("failure", start.elapsed());
            return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
        }
    };

    let mut locked_manager = user_manager.lock().unwrap();
    let credentials = match locked_manager.get_user_credentials(&body.user_handle) {
        Ok(Some(creds)) => creds,
        Ok(None) => {
            super::metrics::record_login_attempt("failure", start.elapsed());
            return StatusCode::UNAUTHORIZED.into_response();
        }
        Err(_) => {
            super::metrics::record_login_attempt("error", start.elapsed());
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Some(password_credentials) = &credentials.username_password {
        if let Ok(true) = password_credentials.hasher.verify(
            &body.password,
            &password_credentials.hash,
            &password_credentials.salt,
        ) {
            // Fetch user permissions
            let permissions = match locked_manager.get_user_permissions(credentials.user_id) {
                Ok(perms) => perms,
                Err(err) => {
                    error!("Error fetching user permissions: {}", err);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            // 2. Register/update device
            let device_id = match locked_manager.register_or_update_device(&device_registration) {
                Ok(id) => id,
                Err(e) => {
                    error!("Device registration failed: {}", e);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            // 3. Associate device with user
            if let Err(e) =
                locked_manager.associate_device_with_user(device_id, credentials.user_id)
            {
                error!("Device association failed: {}", e);
                // Non-fatal, continue with login
            }

            // 4. Enforce per-user device limit
            if let Err(e) =
                locked_manager.enforce_user_device_limit(credentials.user_id, MAX_DEVICES_PER_USER)
            {
                error!("Device limit enforcement failed: {}", e);
                // Non-fatal, continue with login
            }

            // 5. Generate auth token with device_id
            return match locked_manager.generate_auth_token(&credentials, device_id) {
                Ok(auth_token) => {
                    super::metrics::record_login_attempt("success", start.elapsed());
                    let response_body = LoginSuccessResponse {
                        token: auth_token.value.0.clone(),
                        user_handle: body.user_handle.clone(),
                        permissions,
                    };
                    let response_body = serde_json::to_string(&response_body).unwrap();

                    let mut response = response::Builder::new()
                        .status(StatusCode::CREATED)
                        .body(Body::from(response_body))
                        .unwrap();
                    append_session_cookies(
                        &mut response,
                        auth_token.value.0.clone(),
                        None,
                        &config,
                    );
                    response
                }
                Err(err) => {
                    error!("Error with auth token generation: {}", err);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            };
        }
    }
    super::metrics::record_login_attempt("failure", start.elapsed());
    StatusCode::UNAUTHORIZED.into_response()
}

async fn logout(
    State(user_manager): State<GuardedUserManager>,
    State(config): State<ServerConfig>,
    session: Session,
) -> Response {
    // Try to delete auth token from database (for legacy sessions)
    // For OIDC sessions, this will fail since JWT isn't stored in DB - that's OK
    let mut locked_manager = user_manager.lock().unwrap();
    let _ = locked_manager.delete_auth_token(&session.user_id, &AuthTokenValue(session.token));

    // Always clear both cookies using exactly the same attributes used when setting them.
    let mut response = response::Builder::new()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap();
    append_expired_session_cookies(&mut response, &config);
    response
}

// ============================================================================
// OIDC Authentication Handlers
// ============================================================================

/// Query parameters for OIDC login initiation
#[derive(Deserialize, Debug, Default)]
struct OidcLoginQuery {
    /// Device ID for multi-device tracking
    device_id: Option<String>,
    /// Device type (web, android, ios, desktop)
    device_type: Option<String>,
    /// Human-readable device name
    device_name: Option<String>,
}

/// OIDC login initiation - redirects to the OIDC provider
async fn oidc_login(
    Query(params): Query<OidcLoginQuery>,
    State(oidc_client): State<OptionalOidcClient>,
    State(auth_state_store): State<GuardedAuthStateStore>,
) -> Response {
    let oidc_client = match oidc_client {
        Some(client) => client,
        None => {
            error!("OIDC login attempted but OIDC is not configured");
            return (StatusCode::SERVICE_UNAVAILABLE, "OIDC is not configured").into_response();
        }
    };

    // Build device info from query params
    let device_info = if params.device_id.is_some()
        || params.device_type.is_some()
        || params.device_name.is_some()
    {
        Some(crate::oidc::DeviceInfo {
            device_id: params.device_id,
            device_type: params.device_type,
            device_name: params.device_name,
        })
    } else {
        None
    };

    match oidc_client.authorize_url(device_info.as_ref()) {
        Ok((auth_url, state)) => {
            // Store the state for validation in callback
            auth_state_store.store(state.clone()).await;
            debug!(
                "Initiating OIDC login, redirecting to provider with state={}, device_id={:?}",
                state.csrf_token, state.device_id
            );

            // Return redirect response
            response::Builder::new()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, auth_url)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            error!("Failed to generate OIDC authorization URL: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for OIDC callback
#[derive(Deserialize, Debug)]
struct OidcCallbackQuery {
    code: String,
    state: String,
}

/// OIDC callback - exchanges authorization code for tokens
async fn oidc_callback(
    Query(params): Query<OidcCallbackQuery>,
    State(oidc_client): State<OptionalOidcClient>,
    State(auth_state_store): State<GuardedAuthStateStore>,
    State(user_manager): State<GuardedUserManager>,
    State(config): State<ServerConfig>,
) -> Response {
    let start = Instant::now();
    debug!("OIDC callback received with state={}", params.state);

    let oidc_client = match oidc_client {
        Some(client) => client,
        None => {
            error!("OIDC callback received but OIDC is not configured");
            super::metrics::record_login_attempt("error", start.elapsed());
            return (StatusCode::SERVICE_UNAVAILABLE, "OIDC is not configured").into_response();
        }
    };

    // Retrieve and validate stored state
    let stored_state = match auth_state_store.take(&params.state).await {
        Some(state) => state,
        None => {
            warn!("OIDC callback with unknown or expired state");
            super::metrics::record_login_attempt("failure", start.elapsed());
            return (StatusCode::BAD_REQUEST, "Invalid or expired state").into_response();
        }
    };

    // Exchange code for tokens and validate
    let auth_result = match oidc_client
        .exchange_code(&params.code, &params.state, &stored_state)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("OIDC token exchange failed: {}", e);
            super::metrics::record_login_attempt("failure", start.elapsed());
            return (StatusCode::UNAUTHORIZED, "Authentication failed").into_response();
        }
    };

    debug!(
        "OIDC authentication successful for subject={}",
        auth_result.subject
    );

    // Look up or provision local user by OIDC subject
    let user_id = {
        let locked_manager = user_manager.lock().unwrap();

        match locked_manager.get_user_id_by_oidc_subject(&auth_result.subject) {
            Ok(Some(id)) => {
                debug!(
                    "Found existing user for OIDC subject={}",
                    auth_result.subject
                );
                id
            }
            Ok(None) => {
                // Auto-provision new user
                info!(
                    "Provisioning new user for OIDC subject={} (email={:?}, username={:?})",
                    auth_result.subject, auth_result.email, auth_result.preferred_username
                );
                match locked_manager.provision_oidc_user(
                    &auth_result.subject,
                    auth_result.preferred_username.as_deref(),
                    auth_result.email.as_deref(),
                ) {
                    Ok(id) => {
                        info!(
                            "Successfully provisioned new user_id={} for OIDC subject={}",
                            id, auth_result.subject
                        );
                        id
                    }
                    Err(e) => {
                        error!("Failed to provision OIDC user: {}", e);
                        super::metrics::record_login_attempt("error", start.elapsed());
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                }
            }
            Err(e) => {
                error!("Failed to look up user by OIDC subject: {}", e);
                super::metrics::record_login_attempt("error", start.elapsed());
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    // Exchange the provider credential for a local opaque session. This keeps ID
    // tokens out of browser cookies and makes logout/revocation authoritative here.
    let session_token = {
        let mut locked_manager = user_manager.lock().unwrap();
        let device_id = stored_state.device_id.as_deref().and_then(|device_uuid| {
            match locked_manager.get_device_by_uuid(device_uuid) {
                Ok(Some(device)) => {
                    if let Err(error) =
                        locked_manager.associate_device_with_user(device.id, user_id)
                    {
                        debug!(
                            "Could not associate OIDC device {} with user {}: {}",
                            device.id, user_id, error
                        );
                    }
                    Some(device.id)
                }
                Ok(None) => {
                    let device_type = stored_state.device_type.as_deref().unwrap_or("web");
                    let registration = DeviceRegistration::validate_and_sanitize(
                        device_uuid,
                        device_type,
                        Some(device_uuid),
                        stored_state.device_name.as_deref(),
                    )
                    .map_err(|error| {
                        debug!("Ignoring invalid OIDC device information: {error}");
                        error
                    })
                    .ok()?;
                    match locked_manager.register_or_update_device(&registration) {
                        Ok(device_id) => {
                            if let Err(error) =
                                locked_manager.associate_device_with_user(device_id, user_id)
                            {
                                debug!(
                                    "Could not associate OIDC device {} with user {}: {}",
                                    device_id, user_id, error
                                );
                            }
                            Some(device_id)
                        }
                        Err(error) => {
                            debug!("Could not register OIDC device: {error}");
                            None
                        }
                    }
                }
                Err(error) => {
                    debug!("Could not look up OIDC device: {error}");
                    None
                }
            }
        });

        match locked_manager.generate_auth_token_for_user(user_id, device_id) {
            Ok(token) => token.value.0,
            Err(error) => {
                error!("Failed to create local OIDC session: {error}");
                super::metrics::record_login_attempt("error", start.elapsed());
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    super::metrics::record_login_attempt("success", start.elapsed());
    info!("OIDC login successful for user_id={}", user_id);

    // Redirect to the app after successful authentication
    let mut response = response::Builder::new()
        .status(StatusCode::FOUND)
        .header(axum::http::header::LOCATION, "/")
        .body(Body::empty())
        .unwrap();
    append_session_cookies(&mut response, session_token, None, &config);
    response
}

async fn get_session(
    State(user_manager): State<GuardedUserManager>,
    State(config): State<ServerConfig>,
    cookie_jar: CookieJar,
    session: Session,
) -> Response {
    let locked_manager = user_manager.lock().unwrap();

    // Get the user handle from user_id
    let user_handle = match locked_manager.get_user_handle(session.user_id) {
        Ok(Some(handle)) => handle,
        Ok(None) => {
            error!("User handle not found for user_id={}", session.user_id);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(err) => {
            error!(
                "Failed to get user handle for user_id={}: {}",
                session.user_id, err
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response_body = SessionResponse {
        user_handle,
        permissions: session.permissions.clone(),
    };

    let mut response = Json(response_body).into_response();
    // Refresh the browser cookie and issue a CSRF token. This also converts an
    // Authorization-authenticated OIDC session into an HttpOnly cookie for WebSockets.
    let csrf_token = cookie_jar
        .get(crate::server::session_cookie::csrf_cookie_name(
            config.secure_session_cookies,
        ))
        .map(|cookie| cookie.value().to_owned());
    append_session_cookies(&mut response, session.token, csrf_token, &config);
    response
}

