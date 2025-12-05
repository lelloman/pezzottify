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

    #[test]
    fn test_validate_valid_input() {
        let result = DeviceRegistration::validate_and_sanitize(
            "test-uuid-1234",
            "android",
            Some("My Phone"),
            Some("Android 14"),
        );
        assert!(result.is_ok());
        let reg = result.unwrap();
        assert_eq!(reg.device_uuid, "test-uuid-1234");
        assert_eq!(reg.device_type, DeviceType::Android);
        assert_eq!(reg.device_name, Some("My Phone".to_string()));
        assert_eq!(reg.os_info, Some("Android 14".to_string()));
    }

    #[test]
    fn test_validate_uuid_too_short() {
        let result = DeviceRegistration::validate_and_sanitize("short", "web", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("device_uuid"));
    }

    #[test]
    fn test_validate_uuid_too_long() {
        let long_uuid = "a".repeat(65);
        let result = DeviceRegistration::validate_and_sanitize(&long_uuid, "web", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_uuid_invalid_chars() {
        let result =
            DeviceRegistration::validate_and_sanitize("uuid with spaces!", "web", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("alphanumeric"));
    }

    #[test]
    fn test_validate_device_name_truncation() {
        let long_name = "x".repeat(150);
        let result =
            DeviceRegistration::validate_and_sanitize("valid-uuid", "web", Some(&long_name), None);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().device_name.unwrap().len(),
            DEVICE_NAME_MAX_LEN
        );
    }

    #[test]
    fn test_validate_os_info_truncation() {
        let long_info = "y".repeat(250);
        let result =
            DeviceRegistration::validate_and_sanitize("valid-uuid", "web", None, Some(&long_info));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().os_info.unwrap().len(), OS_INFO_MAX_LEN);
    }

    #[test]
    fn test_validate_control_chars_stripped() {
        let result = DeviceRegistration::validate_and_sanitize(
            "valid-uuid",
            "web",
            Some("Name\x00With\x1FControl"),
            Some("OS\nInfo"),
        );
        assert!(result.is_ok());
        let reg = result.unwrap();
        assert_eq!(reg.device_name, Some("NameWithControl".to_string()));
        assert_eq!(reg.os_info, Some("OSInfo".to_string()));
    }

    #[test]
    fn test_validate_whitespace_trimming() {
        let result = DeviceRegistration::validate_and_sanitize(
            "  valid-uuid  ",
            "web",
            Some("  trimmed  "),
            None,
        );
        assert!(result.is_ok());
        let reg = result.unwrap();
        assert_eq!(reg.device_uuid, "valid-uuid");
        assert_eq!(reg.device_name, Some("trimmed".to_string()));
    }

    #[test]
    fn test_validate_empty_optional_becomes_none() {
        let result =
            DeviceRegistration::validate_and_sanitize("valid-uuid", "web", Some(""), Some("   "));
        assert!(result.is_ok());
        let reg = result.unwrap();
        assert!(reg.device_name.is_none());
        assert!(reg.os_info.is_none());
    }
}
