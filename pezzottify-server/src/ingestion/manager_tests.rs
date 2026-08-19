#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = IngestionManagerConfig::default();
        assert_eq!(config.target_bitrate, 320);
        assert_eq!(config.bitrate_tolerance, 50);
        assert_eq!(config.max_iterations, 20);
        assert!((config.auto_match_threshold - 0.85).abs() < 0.001);
    }

    fn queue_item(id: &str, owner: Option<&str>) -> QueueItemInfo {
        QueueItemInfo {
            id: id.to_string(),
            content_id: "album".to_string(),
            content_name: None,
            artist_name: None,
            requested_by_user_id: owner.map(str::to_string),
        }
    }

    #[test]
    fn download_request_context_requires_matching_owner_and_id() {
        let item = queue_item("request-1", Some("42"));
        assert!(
            IngestionManager::validate_download_request_owner("42", "request-1", &item, false)
                .is_ok()
        );

        assert!(
            IngestionManager::validate_download_request_owner("7", "request-1", &item, false)
                .is_err()
        );
        assert!(IngestionManager::validate_download_request_owner(
            "42",
            "different-request",
            &item,
            true
        )
        .is_err());
    }

    #[test]
    fn server_admin_may_use_foreign_download_request_context() {
        let item = queue_item("request-1", Some("42"));
        assert!(
            IngestionManager::validate_download_request_owner("7", "request-1", &item, true)
                .is_ok()
        );
    }

    #[test]
    fn test_string_similarity() {
        assert!((string_similarity("Abbey Road", "Abbey Road") - 1.0).abs() < 0.001);
        assert!((string_similarity("abbey road", "Abbey Road") - 1.0).abs() < 0.001);
        assert!(string_similarity("Abbey Road", "The Beatles") < 0.5);
        // "Abbey Rd" vs "Abbey Road": distance=2, max_len=10 -> 0.8 similarity
        assert!(string_similarity("Abbey Rd", "Abbey Road") >= 0.8);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    /// Test bitrate decision logic with default config (320 kbps ± 50 kbps)
    fn bitrate_to_conversion_reason(
        config: &IngestionManagerConfig,
        bitrate: Option<i32>,
    ) -> ConversionReason {
        let min_bitrate = config.target_bitrate as i32 - config.bitrate_tolerance as i32;
        let max_bitrate = config.target_bitrate as i32 + config.bitrate_tolerance as i32;

        let bitrate = match bitrate {
            Some(b) if b > 0 => b,
            _ => return ConversionReason::UndetectableBitrate,
        };

        if bitrate < min_bitrate {
            return ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: bitrate,
            };
        }

        if bitrate > max_bitrate {
            return ConversionReason::HighBitrate {
                original_bitrate: bitrate,
            };
        }

        ConversionReason::NoConversionNeeded
    }

    #[test]
    fn test_bitrate_conversion_decision() {
        let config = IngestionManagerConfig::default();

        // Test undetectable bitrate (None)
        let result = bitrate_to_conversion_reason(&config, None);
        assert!(matches!(result, ConversionReason::UndetectableBitrate));

        // Test undetectable bitrate (Some(0))
        let result = bitrate_to_conversion_reason(&config, Some(0));
        assert!(matches!(result, ConversionReason::UndetectableBitrate));

        // Test low bitrate (< 270 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(128));
        assert!(matches!(
            result,
            ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: 128
            }
        ));

        // Test low bitrate at boundary (269 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(269));
        assert!(matches!(
            result,
            ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: 269
            }
        ));

        // Test acceptable bitrate (270 kbps - lower boundary)
        let result = bitrate_to_conversion_reason(&config, Some(270));
        assert!(matches!(result, ConversionReason::NoConversionNeeded));

        // Test acceptable bitrate (320 kbps - target)
        let result = bitrate_to_conversion_reason(&config, Some(320));
        assert!(matches!(result, ConversionReason::NoConversionNeeded));

        // Test acceptable bitrate (370 kbps - upper boundary)
        let result = bitrate_to_conversion_reason(&config, Some(370));
        assert!(matches!(result, ConversionReason::NoConversionNeeded));

        // Test high bitrate (371 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(371));
        assert!(matches!(
            result,
            ConversionReason::HighBitrate {
                original_bitrate: 371
            }
        ));

        // Test high bitrate (500 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(500));
        assert!(matches!(
            result,
            ConversionReason::HighBitrate {
                original_bitrate: 500
            }
        ));
    }
}
