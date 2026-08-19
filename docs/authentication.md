# Authentication lifecycle

Pezzottify supports OIDC and the original username/password authentication flow during the
migration period.

## OIDC sessions

The authorization-code callback validates the provider ID token, including its signature,
issuer, audience, expiry, and the authorization-flow nonce. Pezzottify then creates a random
opaque application session and stores that session in the local user database. Provider access
and ID tokens are not placed in the browser cookie.

This makes `POST /v1/auth/logout` server-side authoritative: it deletes the local session before
expiring the browser cookies. The normal device and unused-session cleanup mechanisms apply to
OIDC-created sessions as well.

OIDC discovery metadata and signing keys are cached for 15 minutes. The cache is refreshed when
it expires and once immediately when token validation cannot find a matching signing key.
Failure-triggered refresh attempts are limited to one every 30 seconds across requests to avoid
turning invalid input or a provider outage into an outbound-request flood.

Bearer ID tokens remain accepted for non-browser clients during migration. They are validated by
the OIDC library against the discovered issuer, client audience, provider-advertised signing
algorithms, signing keys, and time claims. A nonce is required and checked at the authorization
callback; it is not applicable when revalidating an already-issued bearer token.

## Legacy authentication migration

The password endpoint remains available by default for existing clients. Set
`oidc.disable_password_auth: true` after every supported client uses the OIDC authorization-code
flow. Before removing legacy authentication entirely:

1. verify that password-login traffic has reached zero for the intended deprecation window;
2. revoke remaining legacy sessions and communicate that users must sign in again;
3. remove bearer ID-token compatibility after native clients exchange OIDC credentials for local
   application sessions; and
4. remove the password route, password credential storage, and the OIDC fallback branch from the
   session extractor in the same release.

Do not log authorization codes, ID/access tokens, session values, or full claims while operating
either authentication path.
