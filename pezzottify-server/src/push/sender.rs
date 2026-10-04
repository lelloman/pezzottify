//! Building and sending one encrypted Web Push wake-up (RFC 8030/8291/8292).

use super::VapidKeys;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use web_push::{
    ContentEncoding, SubscriptionInfo, Urgency, VapidSignatureBuilder, WebPushMessageBuilder,
};

pub const MAX_ENDPOINT_LEN: usize = 2048;
/// Undelivered wake-ups are useless after a day: the app syncs on its own by then.
pub const PUSH_TTL_SECS: u32 = 86_400;

/// How a wake-up is sent: notification-worthy events are normal urgency and kept a
/// day; background sync wake-ups are low urgency, kept an hour, and share a topic so
/// a push service can replace an undelivered one with the newer one (RFC 8030 5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeupOptions {
    pub ttl_secs: u32,
    pub low_urgency: bool,
    pub topic: Option<&'static str>,
}

impl WakeupOptions {
    pub const NOTIFICATION: Self = Self {
        ttl_secs: PUSH_TTL_SECS,
        low_urgency: false,
        topic: None,
    };
    pub const SYNC: Self = Self {
        ttl_secs: 3_600,
        low_urgency: true,
        topic: Some("sync"),
    };
}

/// Why a registration was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    Endpoint(&'static str),
    Keys(&'static str),
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Endpoint(reason) => write!(f, "invalid endpoint: {reason}"),
            Self::Keys(reason) => write!(f, "invalid keys: {reason}"),
        }
    }
}

/// Check a push endpoint URL from a client.
pub fn validate_endpoint(endpoint: &str, allow_insecure: bool) -> Result<(), RegistrationError> {
    if endpoint.is_empty() || endpoint.len() > MAX_ENDPOINT_LEN {
        return Err(RegistrationError::Endpoint("must be 1..2048 bytes"));
    }
    let url =
        reqwest::Url::parse(endpoint).map_err(|_| RegistrationError::Endpoint("not a URL"))?;
    match url.scheme() {
        "https" => {}
        "http" if allow_insecure => {}
        _ => return Err(RegistrationError::Endpoint("must use https")),
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(RegistrationError::Endpoint("missing host"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(RegistrationError::Endpoint("must not contain credentials"));
    }
    Ok(())
}

/// Check the receiver's Web Push keys: a 65-byte uncompressed P-256 point and a
/// 16-byte auth secret, both base64url (padding tolerated).
pub fn validate_keys(p256dh: &str, auth: &str) -> Result<(), RegistrationError> {
    let decode = |value: &str| URL_SAFE_NO_PAD.decode(value.trim_end_matches('='));
    let point = decode(p256dh).map_err(|_| RegistrationError::Keys("p256dh is not base64url"))?;
    if point.len() != 65 || point[0] != 0x04 {
        return Err(RegistrationError::Keys(
            "p256dh must be an uncompressed P-256 point",
        ));
    }
    let secret = decode(auth).map_err(|_| RegistrationError::Keys("auth is not base64url"))?;
    if secret.len() != 16 {
        return Err(RegistrationError::Keys("auth must be 16 bytes"));
    }
    Ok(())
}

/// Result of one delivery attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Delivered,
    /// The push service no longer knows this endpoint (404/410): drop it.
    Gone,
    Failed(String),
}

/// Where a wake-up goes: a UnifiedPush endpoint and its Web Push keys.
#[derive(Debug, Clone, Copy)]
pub struct PushTarget<'a> {
    pub endpoint: &'a str,
    /// Base64url P-256 public key of the receiving app.
    pub p256dh: &'a str,
    /// Base64url authentication secret.
    pub auth: &'a str,
}

/// Encrypt `payload` for one registration, sign with VAPID and POST it.
pub async fn send_wakeup(
    client: &reqwest::Client,
    vapid: &VapidKeys,
    subject: &str,
    target: PushTarget<'_>,
    payload: &[u8],
    options: WakeupOptions,
) -> DeliveryOutcome {
    let subscription = SubscriptionInfo::new(
        target.endpoint,
        target.p256dh.trim_end_matches('='),
        target.auth.trim_end_matches('='),
    );
    let message = (|| {
        let mut signature =
            VapidSignatureBuilder::from_base64(vapid.private_key_b64(), &subscription)?;
        signature.add_claim("sub", subject);
        let mut builder = WebPushMessageBuilder::new(&subscription);
        builder.set_payload(ContentEncoding::Aes128Gcm, payload);
        builder.set_ttl(options.ttl_secs);
        builder.set_urgency(if options.low_urgency {
            Urgency::Low
        } else {
            Urgency::Normal
        });
        builder.set_vapid_signature(signature.build()?);
        builder.build()
    })();
    let message = match message {
        Ok(message) => message,
        Err(error) => return DeliveryOutcome::Failed(format!("build: {error}")),
    };

    let mut request = client
        .post(message.endpoint.to_string())
        .header("TTL", message.ttl.to_string());
    if let Some(urgency) = message.urgency {
        request = request.header("Urgency", urgency.to_string());
    }
    if let Some(topic) = options.topic {
        request = request.header("Topic", topic);
    }
    if let Some(payload) = message.payload {
        request = request
            .header("Content-Encoding", payload.content_encoding.to_str())
            .header("Content-Type", "application/octet-stream");
        for (name, value) in payload.crypto_headers {
            request = request.header(name, value);
        }
        request = request.body(payload.content);
    }

    match request.send().await {
        Ok(response) => match response.status().as_u16() {
            200..=299 => DeliveryOutcome::Delivered,
            404 | 410 => DeliveryOutcome::Gone,
            status => DeliveryOutcome::Failed(format!("HTTP {status}")),
        },
        Err(error) => DeliveryOutcome::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_validation() {
        assert!(validate_endpoint("https://push.example.org/up/abc", false).is_ok());
        assert!(validate_endpoint("http://push.example.org/up/abc", false).is_err());
        assert!(validate_endpoint("http://127.0.0.1:9/up", true).is_ok());
        assert!(validate_endpoint("ftp://push.example.org/x", true).is_err());
        assert!(validate_endpoint("not a url", false).is_err());
        assert!(validate_endpoint("", false).is_err());
        assert!(validate_endpoint("https://user:pw@push.example.org/x", false).is_err());
        let long = format!("https://push.example.org/{}", "a".repeat(MAX_ENDPOINT_LEN));
        assert!(validate_endpoint(&long, false).is_err());
    }

    #[test]
    fn key_validation() {
        let point = URL_SAFE_NO_PAD.encode([&[4u8][..], &[7u8; 64][..]].concat());
        let auth = URL_SAFE_NO_PAD.encode([9u8; 16]);
        assert!(validate_keys(&point, &auth).is_ok());
        assert!(validate_keys(&format!("{point}="), &format!("{auth}==")).is_ok());
        assert!(validate_keys(&URL_SAFE_NO_PAD.encode([4u8; 33]), &auth).is_err());
        assert!(validate_keys(&point, &URL_SAFE_NO_PAD.encode([9u8; 8])).is_err());
        assert!(validate_keys("!!", &auth).is_err());
    }
}
