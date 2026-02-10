"""Minimal OIDC provider for e2e tests.

Implements just enough of the OIDC spec to support the pezzottify
client-side OIDC flow (authorization code + PKCE).
"""

import base64
import hashlib
import json
import os
import time
import uuid
from urllib.parse import urlencode, urlparse, parse_qs, urlunparse

import jwt
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives import serialization
from flask import Flask, request, jsonify, redirect, Response

app = Flask(__name__)

# --- Configuration -----------------------------------------------------------

ISSUER = os.environ.get("MOCK_OIDC_ISSUER", "http://mock-oidc:8080")
_users_raw = os.environ.get("MOCK_OIDC_USERS", "testuser:testpass123,admin:adminpass123")
USERS = {}
for pair in _users_raw.split(","):
    pair = pair.strip()
    if ":" in pair:
        u, p = pair.split(":", 1)
        USERS[u] = p

# --- RSA key pair (generated once at startup) --------------------------------

_private_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
_public_key = _private_key.public_key()
_kid = "mock-oidc-key-1"

_private_pem = _private_key.private_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PrivateFormat.PKCS8,
    encryption_algorithm=serialization.NoEncryption(),
)

_public_numbers = _public_key.public_numbers()


def _int_to_base64url(n: int) -> str:
    byte_length = (n.bit_length() + 7) // 8
    return base64.urlsafe_b64encode(n.to_bytes(byte_length, "big")).rstrip(b"=").decode()


JWKS = {
    "keys": [
        {
            "kty": "RSA",
            "kid": _kid,
            "use": "sig",
            "alg": "RS256",
            "n": _int_to_base64url(_public_numbers.n),
            "e": _int_to_base64url(_public_numbers.e),
        }
    ]
}

# --- In-memory stores --------------------------------------------------------

# auth_code -> {client_id, redirect_uri, code_challenge, nonce, username, scope}
_auth_codes: dict[str, dict] = {}

# refresh_token -> {client_id, username, scope}
_refresh_tokens: dict[str, dict] = {}

# --- Helpers ------------------------------------------------------------------


def _make_id_token(username: str, client_id: str, nonce: str | None = None) -> str:
    now = int(time.time())
    claims = {
        "iss": ISSUER,
        "sub": username,
        "aud": client_id,
        "exp": now + 3600,
        "iat": now,
        "email": f"{username}@test.local",
        "preferred_username": username,
        "device_id": "e2e-device",
        "device_type": "web",
        "device_name": "E2E Test Browser",
    }
    if nonce:
        claims["nonce"] = nonce
    return jwt.encode(claims, _private_pem, algorithm="RS256", headers={"kid": _kid})


# --- Endpoints ----------------------------------------------------------------


@app.route("/.well-known/openid-configuration")
def discovery():
    return jsonify(
        {
            "issuer": ISSUER,
            "authorization_endpoint": f"{ISSUER}/authorize",
            "token_endpoint": f"{ISSUER}/token",
            "jwks_uri": f"{ISSUER}/jwks",
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "scopes_supported": ["openid", "profile", "email", "offline_access"],
            "token_endpoint_auth_methods_supported": [
                "client_secret_post",
                "client_secret_basic",
                "none",
            ],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
        }
    )


@app.route("/jwks")
def jwks():
    return jsonify(JWKS)


@app.route("/authorize", methods=["GET"])
def authorize_form():
    params = request.args
    html = f"""<!DOCTYPE html>
<html>
<head><title>Mock OIDC Login</title></head>
<body>
<h1>Mock OIDC Login</h1>
<form method="POST" action="/authorize">
  <input type="hidden" name="client_id" value="{params.get('client_id', '')}">
  <input type="hidden" name="redirect_uri" value="{params.get('redirect_uri', '')}">
  <input type="hidden" name="response_type" value="{params.get('response_type', 'code')}">
  <input type="hidden" name="scope" value="{params.get('scope', 'openid')}">
  <input type="hidden" name="state" value="{params.get('state', '')}">
  <input type="hidden" name="nonce" value="{params.get('nonce', '')}">
  <input type="hidden" name="code_challenge" value="{params.get('code_challenge', '')}">
  <input type="hidden" name="code_challenge_method" value="{params.get('code_challenge_method', '')}">
  <label>Username: <input type="text" name="username"></label><br>
  <label>Password: <input type="password" name="password"></label><br>
  <button type="submit">Login</button>
</form>
</body>
</html>"""
    return Response(html, content_type="text/html")


@app.route("/authorize", methods=["POST"])
def authorize_submit():
    username = request.form.get("username", "")
    password = request.form.get("password", "")
    redirect_uri = request.form.get("redirect_uri", "")
    client_id = request.form.get("client_id", "")
    state = request.form.get("state", "")
    nonce = request.form.get("nonce", "")
    code_challenge = request.form.get("code_challenge", "")
    scope = request.form.get("scope", "openid")

    if username not in USERS or USERS[username] != password:
        return Response("Invalid credentials", status=401)

    code = str(uuid.uuid4())
    _auth_codes[code] = {
        "client_id": client_id,
        "redirect_uri": redirect_uri,
        "code_challenge": code_challenge,
        "nonce": nonce,
        "username": username,
        "scope": scope,
    }

    parsed = urlparse(redirect_uri)
    qs = parse_qs(parsed.query)
    qs["code"] = [code]
    if state:
        qs["state"] = [state]
    new_query = urlencode(qs, doseq=True)
    target = urlunparse(parsed._replace(query=new_query))
    return redirect(target)


@app.route("/token", methods=["POST"])
def token():
    grant_type = request.form.get("grant_type")

    if grant_type == "authorization_code":
        return _handle_authorization_code()
    elif grant_type == "refresh_token":
        return _handle_refresh_token()
    else:
        return jsonify({"error": "unsupported_grant_type"}), 400


def _handle_authorization_code():
    code = request.form.get("code", "")
    code_verifier = request.form.get("code_verifier", "")

    stored = _auth_codes.pop(code, None)
    if not stored:
        return jsonify({"error": "invalid_grant", "error_description": "Unknown code"}), 400

    # PKCE S256 validation
    if stored["code_challenge"]:
        digest = hashlib.sha256(code_verifier.encode()).digest()
        expected = base64.urlsafe_b64encode(digest).rstrip(b"=").decode()
        if expected != stored["code_challenge"]:
            return jsonify({"error": "invalid_grant", "error_description": "PKCE mismatch"}), 400

    username = stored["username"]
    client_id = stored["client_id"]
    nonce = stored["nonce"]
    scope = stored["scope"]

    id_token = _make_id_token(username, client_id, nonce)

    refresh_token = str(uuid.uuid4())
    _refresh_tokens[refresh_token] = {
        "client_id": client_id,
        "username": username,
        "scope": scope,
    }

    return jsonify(
        {
            "access_token": str(uuid.uuid4()),
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": id_token,
            "refresh_token": refresh_token,
            "scope": scope,
        }
    )


def _handle_refresh_token():
    refresh_token = request.form.get("refresh_token", "")
    stored = _refresh_tokens.get(refresh_token)
    if not stored:
        return jsonify({"error": "invalid_grant", "error_description": "Unknown refresh token"}), 400

    username = stored["username"]
    client_id = stored["client_id"]
    scope = stored["scope"]

    id_token = _make_id_token(username, client_id)

    new_refresh_token = str(uuid.uuid4())
    _refresh_tokens[new_refresh_token] = stored
    del _refresh_tokens[refresh_token]

    return jsonify(
        {
            "access_token": str(uuid.uuid4()),
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": id_token,
            "refresh_token": new_refresh_token,
            "scope": scope,
        }
    )


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8080)
