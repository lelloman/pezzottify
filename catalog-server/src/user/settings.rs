//! User settings types and serialization.
//!
//! This module defines the typed user settings enum and handles
//! serialization to/from string values for database storage.

use serde::{Deserialize, Serialize};

/// All supported user settings with their typed values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "key", content = "value")]
pub enum UserSetting {
    /// Whether the user wants to be notified when new catalog batches are closed.
    /// When enabled, a push notification will be sent to connected clients
    /// or queued via sync for delivery on next connection.
    #[serde(rename = "notify_whatsnew")]
    NotifyWhatsNew(bool),
}

impl UserSetting {
    /// Get the storage key for this setting.
    pub fn key(&self) -> &'static str {
        match self {
            Self::NotifyWhatsNew(_) => "notify_whatsnew",
        }
    }

    /// Serialize the value to a string for database storage.
    pub fn value_to_string(&self) -> String {
        match self {
            Self::NotifyWhatsNew(enabled) => enabled.to_string(),
        }
    }

    /// Deserialize from key-value strings (used by store implementation).
    ///
    /// Returns `Ok(setting)` if the key is known and value is valid,
    /// `Err` with a description if the key is unknown or value is invalid.
    pub fn from_key_value(key: &str, value: &str) -> Result<Self, String> {
        match key {
            "notify_whatsnew" => {
                let enabled = value
                    .parse::<bool>()
                    .map_err(|_| format!("Invalid boolean value for {}: {}", key, value))?;
                Ok(Self::NotifyWhatsNew(enabled))
            }
            _ => Err(format!("Unknown setting key: {}", key)),
        }
    }

    /// Check if a key is a known setting key.
    pub fn is_known_key(key: &str) -> bool {
        matches!(key, "notify_whatsnew")
    }

    /// Get the default value for a setting by key.
    pub fn default_for_key(key: &str) -> Option<Self> {
        match key {
            "notify_whatsnew" => Some(Self::NotifyWhatsNew(false)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key() {
        let setting = UserSetting::NotifyWhatsNew(true);
        assert_eq!(setting.key(), "notify_whatsnew");
    }

    #[test]
    fn test_value_to_string() {
        assert_eq!(UserSetting::NotifyWhatsNew(true).value_to_string(), "true");
        assert_eq!(
            UserSetting::NotifyWhatsNew(false).value_to_string(),
            "false"
        );
    }

    #[test]
    fn test_from_key_value_valid() {
        assert_eq!(
            UserSetting::from_key_value("notify_whatsnew", "true"),
            Ok(UserSetting::NotifyWhatsNew(true))
        );
        assert_eq!(
            UserSetting::from_key_value("notify_whatsnew", "false"),
            Ok(UserSetting::NotifyWhatsNew(false))
        );
    }

    #[test]
    fn test_from_key_value_invalid_value() {
        let result = UserSetting::from_key_value("notify_whatsnew", "yes");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid boolean value"));
    }

    #[test]
    fn test_from_key_value_unknown_key() {
        let result = UserSetting::from_key_value("unknown_key", "value");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown setting key"));
    }

    #[test]
    fn test_is_known_key() {
        assert!(UserSetting::is_known_key("notify_whatsnew"));
        assert!(!UserSetting::is_known_key("unknown_key"));
    }

    #[test]
    fn test_default_for_key() {
        assert_eq!(
            UserSetting::default_for_key("notify_whatsnew"),
            Some(UserSetting::NotifyWhatsNew(false))
        );
        assert_eq!(UserSetting::default_for_key("unknown_key"), None);
    }

    #[test]
    fn test_serde_serialization() {
        let setting = UserSetting::NotifyWhatsNew(true);
        let json = serde_json::to_string(&setting).unwrap();
        assert_eq!(json, r#"{"key":"notify_whatsnew","value":true}"#);
    }

    #[test]
    fn test_serde_deserialization() {
        let json = r#"{"key":"notify_whatsnew","value":true}"#;
        let setting: UserSetting = serde_json::from_str(json).unwrap();
        assert_eq!(setting, UserSetting::NotifyWhatsNew(true));
    }
}
