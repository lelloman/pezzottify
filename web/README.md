# pezzottify-web

Vue 3 web frontend for Pezzottify music streaming platform.

## Recommended IDE Setup

[VSCode](https://code.visualstudio.com/) + [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar) (and disable Vetur).

## Configuration

Copy `.env.example` to `.env.local` and configure the required OIDC settings:

```sh
cp .env.example .env.local
```

Edit `.env.local` with your OIDC provider settings:
- `VITE_OIDC_AUTHORITY`: Your OIDC provider URL (required)
- `VITE_OIDC_CLIENT_ID`: Client ID registered with the provider (required)

See `.env.example` for additional optional configuration.

## Project Setup

```sh
npm install
```

### Compile and Hot-Reload for Development

```sh
npm run dev
```

### Compile and Minify for Production

```sh
npm run build
```

### Lint with [ESLint](https://eslint.org/)

```sh
npm run lint
```

### Format with Prettier

```sh
npm run format
```

## Authentication

The app uses OIDC (OpenID Connect) for authentication with the following flow:
1. User clicks "Sign in" on the login page
2. Browser redirects to OIDC provider for authentication
3. After successful auth, provider redirects back to `/auth/callback`
4. App exchanges authorization code for tokens (ID token + refresh token)
5. Tokens are stored in localStorage and used for API requests
6. On token expiry (401 response), tokens are automatically refreshed

The app sends the ID token in the `Authorization` header for API requests and sets a session cookie for WebSocket connections.
