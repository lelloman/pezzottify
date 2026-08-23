async fn login(
    State(config): State<ServerConfig>,
    State(database): State<DatabaseHandles>,
    State(password_work): State<PasswordWorkPool>,
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

    let user_handle = body.user_handle.clone();
    let credentials = match database
        .user_manager
        .run(DbPriority::Critical, move |manager| {
            manager.get_user_credentials(&user_handle)
        })
        .await
    {
        Ok(Some(creds)) => creds,
        Ok(None) => {
            super::metrics::record_login_attempt("failure", start.elapsed());
            return StatusCode::UNAUTHORIZED.into_response();
        }
        Err(error) => {
            super::metrics::record_login_attempt("error", start.elapsed());
            return ApiError::from(error).into_response();
        }
    };

    if let Some(password_credentials) = &credentials.username_password {
        let verified = password_work
            .verify(
                password_credentials.hasher.clone(),
                body.password.clone(),
                password_credentials.hash.clone(),
                password_credentials.salt.clone(),
            )
            .await;

        let verified = match verified {
            Err(error) => {
                super::metrics::record_login_attempt("error", start.elapsed());
                return ApiError::from(error).into_response();
            }
            Ok(verified) => verified,
        };

        if verified {
            let login_result = database
                .user_manager
                .run(DbPriority::Critical, move |manager| {
                    let permissions = manager.get_user_permissions(credentials.user_id)?;
                    let device_id = manager.register_or_update_device(&device_registration)?;

                    if let Err(error) =
                        manager.associate_device_with_user(device_id, credentials.user_id)
                    {
                        error!("Device association failed: {}", error);
                    }
                    if let Err(error) = manager
                        .enforce_user_device_limit(credentials.user_id, MAX_DEVICES_PER_USER)
                    {
                        error!("Device limit enforcement failed: {}", error);
                    }

                    let auth_token = manager.generate_auth_token(&credentials, device_id)?;
                    Ok((permissions, auth_token))
                })
                .await;

            return match login_result {
                Ok((permissions, auth_token)) => {
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
                Err(error) => {
                    error!("Error completing login database operations: {}", error);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    ApiError::from(error).into_response()
                }
            };
        }
    }
    super::metrics::record_login_attempt("failure", start.elapsed());
    StatusCode::UNAUTHORIZED.into_response()
}

async fn logout(
    State(database): State<DatabaseHandles>,
    State(config): State<ServerConfig>,
    session: Session,
) -> Response {
    let user_id = session.user_id;
    let token = AuthTokenValue(session.token);
    if let Err(error) = database
        .user_manager
        .run(DbPriority::Critical, move |manager| {
            // A raw provider token is not present in the local token table. Preserve
            // logout's best-effort revocation semantics while still surfacing executor
            // saturation and shutdown failures to the client.
            let _ = manager.delete_auth_token(&user_id, &token);
            Ok(())
        })
        .await
    {
        return ApiError::from(error).into_response();
    }

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
    State(database): State<DatabaseHandles>,
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

    // Exchange the provider credential for a local opaque session. This keeps ID
    // tokens out of browser cookies and makes logout/revocation authoritative here.
    let oidc_session = database
        .user_manager
        .run(DbPriority::Critical, move |manager| {
            complete_oidc_login(manager, auth_result, stored_state)
        })
        .await;
    let (user_id, session_token) = match oidc_session {
        Ok(session) => session,
        Err(error) => {
            error!("Failed to create local OIDC session: {error}");
            super::metrics::record_login_attempt("error", start.elapsed());
            return ApiError::from(error).into_response();
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

fn complete_oidc_login(
    manager: &crate::user::UserManager,
    auth_result: crate::oidc::AuthResult,
    stored_state: crate::oidc::AuthState,
) -> anyhow::Result<(usize, String)> {
    let user_id = match manager.get_user_id_by_oidc_subject(&auth_result.subject)? {
        Some(id) => {
            debug!(
                "Found existing user for OIDC subject={}",
                auth_result.subject
            );
            id
        }
        None => {
            info!(
                "Provisioning new user for OIDC subject={} (email={:?}, username={:?})",
                auth_result.subject, auth_result.email, auth_result.preferred_username
            );
            let id = manager.provision_oidc_user(
                &auth_result.subject,
                auth_result.preferred_username.as_deref(),
                auth_result.email.as_deref(),
            )?;
            info!(
                "Successfully provisioned new user_id={} for OIDC subject={}",
                id, auth_result.subject
            );
            id
        }
    };

    let device_id = stored_state.device_id.as_deref().and_then(|device_uuid| {
        match manager.get_device_by_uuid(device_uuid) {
            Ok(Some(device)) => {
                if let Err(error) = manager.associate_device_with_user(device.id, user_id) {
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
                match manager.register_or_update_device(&registration) {
                    Ok(device_id) => {
                        if let Err(error) =
                            manager.associate_device_with_user(device_id, user_id)
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

    let token = manager.generate_auth_token_for_user(user_id, device_id)?;
    Ok((user_id, token.value.0))
}

async fn get_session(
    State(database): State<DatabaseHandles>,
    State(config): State<ServerConfig>,
    cookie_jar: CookieJar,
    session: Session,
) -> Response {
    // Get the user handle from user_id
    let user_id = session.user_id;
    let user_handle = match database
        .user_manager
        .run(DbPriority::Critical, move |manager| {
            manager.get_user_handle(user_id)
        })
        .await
    {
        Ok(Some(handle)) => handle,
        Ok(None) => {
            error!("User handle not found for user_id={}", session.user_id);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(error) => {
            error!(
                "Failed to get user handle for user_id={}: {}",
                session.user_id, error
            );
            return ApiError::from(error).into_response();
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
