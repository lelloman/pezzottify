"""Playwright browser wrapper for web E2E tests."""

from __future__ import annotations

from playwright.sync_api import Browser, BrowserContext, Page


class WebClient:
    """Wraps a Playwright BrowserContext as a single 'device'.

    Each WebClient has its own isolated browser context (cookies, storage).
    Uses sync Playwright API to avoid event loop conflicts with pytest-asyncio.
    """

    def __init__(self, browser: Browser, base_url: str, name: str = "default"):
        self._browser = browser
        self._base_url = base_url.rstrip("/")
        self._name = name
        self.context: BrowserContext | None = None
        self.page: Page | None = None

    # crypto.randomUUID() and crypto.subtle require a secure context (HTTPS or
    # localhost). E2E tests run over plain HTTP with Docker hostnames, so we
    # polyfill both. The subtle.digest polyfill implements SHA-256 in pure JS
    # (needed for OIDC PKCE code_challenge).
    _CRYPTO_POLYFILL = """
    if (typeof crypto !== 'undefined' && !crypto.randomUUID) {
        crypto.randomUUID = function() {
            return '10000000-1000-4000-8000-100000000000'.replace(
                /[018]/g,
                c => (+c ^ crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> +c / 4).toString(16)
            );
        };
    }
    if (typeof crypto !== 'undefined' && !crypto.subtle) {
        // Minimal SHA-256 for PKCE code_challenge
        const K = new Uint32Array([
            0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
            0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
            0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
            0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
            0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
            0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
            0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
            0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
        ]);
        function sha256(data) {
            const msg = new Uint8Array(data);
            const len = msg.length;
            const bitLen = len * 8;
            const padLen = ((len + 9 + 63) & ~63);
            const buf = new Uint8Array(padLen);
            buf.set(msg);
            buf[len] = 0x80;
            const dv = new DataView(buf.buffer);
            dv.setUint32(padLen - 4, bitLen, false);
            let [h0,h1,h2,h3,h4,h5,h6,h7] = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
            const w = new Uint32Array(64);
            for (let off = 0; off < padLen; off += 64) {
                for (let i = 0; i < 16; i++) w[i] = dv.getUint32(off + i * 4, false);
                for (let i = 16; i < 64; i++) {
                    const s0 = ((w[i-15]>>>7)|(w[i-15]<<25)) ^ ((w[i-15]>>>18)|(w[i-15]<<14)) ^ (w[i-15]>>>3);
                    const s1 = ((w[i-2]>>>17)|(w[i-2]<<15)) ^ ((w[i-2]>>>19)|(w[i-2]<<13)) ^ (w[i-2]>>>10);
                    w[i] = (w[i-16] + s0 + w[i-7] + s1) | 0;
                }
                let [a,b,c,d,e,f,g,h] = [h0,h1,h2,h3,h4,h5,h6,h7];
                for (let i = 0; i < 64; i++) {
                    const S1 = ((e>>>6)|(e<<26)) ^ ((e>>>11)|(e<<21)) ^ ((e>>>25)|(e<<7));
                    const ch = (e & f) ^ (~e & g);
                    const t1 = (h + S1 + ch + K[i] + w[i]) | 0;
                    const S0 = ((a>>>2)|(a<<30)) ^ ((a>>>13)|(a<<19)) ^ ((a>>>22)|(a<<10));
                    const maj = (a & b) ^ (a & c) ^ (b & c);
                    const t2 = (S0 + maj) | 0;
                    h=g; g=f; f=e; e=(d+t1)|0; d=c; c=b; b=a; a=(t1+t2)|0;
                }
                h0=(h0+a)|0; h1=(h1+b)|0; h2=(h2+c)|0; h3=(h3+d)|0;
                h4=(h4+e)|0; h5=(h5+f)|0; h6=(h6+g)|0; h7=(h7+h)|0;
            }
            const out = new Uint8Array(32);
            const odv = new DataView(out.buffer);
            [h0,h1,h2,h3,h4,h5,h6,h7].forEach((v,i) => odv.setUint32(i*4, v, false));
            return out;
        }
        crypto.subtle = {
            digest: function(algorithm, data) {
                return new Promise(function(resolve, reject) {
                    try {
                        const name = typeof algorithm === 'string' ? algorithm : algorithm.name;
                        if (name === 'SHA-256') {
                            resolve(sha256(data).buffer);
                        } else {
                            reject(new Error('Unsupported algorithm: ' + name));
                        }
                    } catch(e) { reject(e); }
                });
            }
        };
    }
    """

    def start(self) -> "WebClient":
        self.context = self._browser.new_context(
            base_url=self._base_url,
            ignore_https_errors=True,
        )
        self.context.add_init_script(self._CRYPTO_POLYFILL)
        self.page = self.context.new_page()
        return self

    def login_password(self, username: str, password: str) -> None:
        """Login via the password form on /login."""
        self.page.goto("/login")
        self.page.locator('input[type="text"]').first.fill(username)
        self.page.locator('input[type="password"]').first.fill(password)
        self.page.locator("button.login-button").first.click()
        # Wait for navigation away from login
        self.page.wait_for_url(
            lambda url: "/login" not in str(url),
            timeout=15000,
        )

    def login_oidc(self) -> None:
        """Login via the OIDC button, filling the LelloAuth form."""
        self.page.goto("/login")
        self.page.locator("button.oidc-button").first.click()
        # LelloAuth form - wait for redirect to OIDC provider
        self.page.wait_for_url(lambda url: "mock-oidc" in str(url), timeout=10000)
        # Fill OIDC login form (LelloAuth test UI)
        self.page.locator('input[name="username"], input[type="text"]').first.fill(
            "testuser"
        )
        self.page.locator('input[name="password"], input[type="password"]').first.fill(
            "testpass123"
        )
        self.page.locator('button[type="submit"]').first.click()
        # Wait for redirect back to app
        self.page.wait_for_url(
            lambda url: "mock-oidc" not in str(url),
            timeout=15000,
        )

    def navigate_to(self, path: str) -> None:
        self.page.goto(path)

    def close(self) -> None:
        if self.context:
            self.context.close()
            self.context = None
            self.page = None

    def __enter__(self) -> "WebClient":
        return self.start()

    def __exit__(self, *args) -> None:
        self.close()
