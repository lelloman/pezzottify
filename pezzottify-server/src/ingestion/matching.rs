// =========================================================================
// Utilities
// =========================================================================

/// Calculate string similarity (0.0 to 1.0).
fn string_similarity(a: &str, b: &str) -> f32 {
    let a = a.to_lowercase();
    let b = b.to_lowercase();

    if a == b {
        return 1.0;
    }

    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Use Levenshtein distance
    let distance = levenshtein_distance(&a, &b);
    let max_len = a.len().max(b.len());

    1.0 - (distance as f32 / max_len as f32)
}

/// Parse metadata from review option label/description.
///
/// Returns (score, track_count, delta_ms) parsed from strings like:
/// - Label: "Artist - Album (75%, 12 tracks)"
/// - Description: "Match: 75%, Delta: 500ms, 12 tracks"
fn parse_option_metadata(label: &str, description: Option<&str>) -> (f32, i32, i64) {
    let mut score = 0.0f32;
    let mut track_count = 0i32;
    let mut delta_ms = 0i64;

    // Try to parse from description first (more structured)
    if let Some(desc) = description {
        // Format: "Match: XX%, Delta: Yms, Z tracks"
        if let Some(match_start) = desc.find("Match: ") {
            let rest = &desc[match_start + 7..];
            if let Some(pct_end) = rest.find('%') {
                if let Ok(pct) = rest[..pct_end].trim().parse::<f32>() {
                    score = pct / 100.0;
                }
            }
        }
        if let Some(delta_start) = desc.find("Delta: ") {
            let rest = &desc[delta_start + 7..];
            if let Some(ms_end) = rest.find("ms") {
                if let Ok(ms) = rest[..ms_end].trim().parse::<i64>() {
                    delta_ms = ms;
                }
            }
        }
        // Parse track count from "N tracks"
        for word in desc.split_whitespace() {
            if let Ok(n) = word.parse::<i32>() {
                // Check if next word is "tracks"
                if desc.contains(&format!("{} tracks", n)) {
                    track_count = n;
                    break;
                }
            }
        }
    }

    // Fallback: parse from label format "Artist - Album (XX%, N tracks)"
    if score == 0.0 {
        if let Some(paren_start) = label.rfind('(') {
            let in_parens = &label[paren_start + 1..];
            if let Some(pct_end) = in_parens.find('%') {
                if let Ok(pct) = in_parens[..pct_end].trim().parse::<f32>() {
                    score = pct / 100.0;
                }
            }
            // Parse track count
            for word in in_parens.split_whitespace() {
                if let Ok(n) = word.parse::<i32>() {
                    if in_parens.contains(&format!("{} tracks", n)) {
                        track_count = n;
                        break;
                    }
                }
            }
        }
    }

    (score, track_count, delta_ms)
}

/// Calculate Levenshtein distance between two strings.
#[allow(clippy::needless_range_loop)]
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

include!("manager_tests.rs");
