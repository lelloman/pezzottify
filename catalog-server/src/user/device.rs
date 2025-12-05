use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// Validation constants
pub const DEVICE_UUID_MIN_LEN: usize = 8;
pub const DEVICE_UUID_MAX_LEN: usize = 64;
pub const DEVICE_NAME_MAX_LEN: usize = 100;
pub const OS_INFO_MAX_LEN: usize = 200;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Web,
    Android,
    Ios,
    Unknown,
}

impl DeviceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "web" => Self::Web,
            "android" => Self::Android,
            "ios" => Self::Ios,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Device {
    pub id: usize,
    pub device_uuid: String,
    pub user_id: Option<usize>,
    pub device_type: DeviceType,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
    pub first_seen: SystemTime,
    pub last_seen: SystemTime,
}

/// Input for registering/updating a device
#[derive(Clone, Debug)]
pub struct DeviceRegistration {
    pub device_uuid: String,
    pub device_type: DeviceType,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
}

impl DeviceRegistration {
    /// Validates and sanitizes a DeviceRegistration from raw input.
    /// Returns error if validation fails.
    pub fn validate_and_sanitize(
        device_uuid: &str,
        device_type: &str,
        device_name: Option<&str>,
        os_info: Option<&str>,
    ) -> Result<Self> {
        // 1. Validate device_uuid
        let device_uuid = device_uuid.trim();
        if device_uuid.len() < DEVICE_UUID_MIN_LEN || device_uuid.len() > DEVICE_UUID_MAX_LEN {
            bail!(
                "device_uuid must be {}-{} characters, got {}",
                DEVICE_UUID_MIN_LEN,
                DEVICE_UUID_MAX_LEN,
                device_uuid.len()
            );
        }
        if !device_uuid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            bail!("device_uuid must contain only alphanumeric characters and hyphens");
        }

        // 2. Validate and sanitize device_name (optional, truncate, strip control chars)
        let device_name = device_name
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.len() > DEVICE_NAME_MAX_LEN {
                    &s[..DEVICE_NAME_MAX_LEN]
                } else {
                    s
                }
            })
            .map(|s| s.chars().filter(|c| !c.is_control()).collect::<String>());

        // 3. Validate and sanitize os_info (optional, truncate, strip control chars)
        let os_info = os_info
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.len() > OS_INFO_MAX_LEN {
                    &s[..OS_INFO_MAX_LEN]
                } else {
                    s
                }
            })
            .map(|s| s.chars().filter(|c| !c.is_control()).collect::<String>());

        Ok(Self {
            device_uuid: device_uuid.to_string(),
            device_type: DeviceType::from_str(device_type),
            device_name,
            os_info,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_from_str_valid() {
        assert_eq!(DeviceType::from_str("web"), DeviceType::Web);
        assert_eq!(DeviceType::from_str("android"), DeviceType::Android);
        assert_eq!(DeviceType::from_str("ios"), DeviceType::Ios);
        assert_eq!(DeviceType::from_str("WEB"), DeviceType::Web); // case insensitive
        assert_eq!(DeviceType::from_str("Android"), DeviceType::Android);
    }

    #[test]
    fn test_device_type_from_str_invalid() {
        assert_eq!(DeviceType::from_str(""), DeviceType::Unknown);
        assert_eq!(DeviceType::from_str("windows"), DeviceType::Unknown);
        assert_eq!(DeviceType::from_str("invalid"), DeviceType::Unknown);
    }

    #[test]
    fn test_device_type_as_str_roundtrip() {
        assert_eq!(
            DeviceType::from_str(DeviceType::Web.as_str()),
            DeviceType::Web
        );
        assert_eq!(
            DeviceType::from_str(DeviceType::Android.as_str()),
            DeviceType::Android
        );
        assert_eq!(
            DeviceType::from_str(DeviceType::Ios.as_str()),
            DeviceType::Ios
        );
        assert_eq!(
            DeviceType::from_str(DeviceType::Unknown.as_str()),
            DeviceType::Unknown
        );
    }
}
