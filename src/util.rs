//! Small platform-agnostic helpers shared by the core engine and every frontend.
//!
//! This module deliberately contains no terminal, OS or UI dependency so that it
//! can be compiled into the Android (`.so`) build of the crate.

/// Normalizes a raw subtitle/language label into a canonical English name.
///
/// Providers return a mix of ISO codes, English names and native scripts
/// (sometimes with Arabic diacritics attached). The terminal UI and the
/// Android UI both display the canonical form.
pub fn sanitize_language_label(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(*c as u32, 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED))
        .collect();

    match cleaned.trim() {
        "العربية" | "Arabic" | "ara" | "ar" => "Arabic".to_string(),
        "اردو" | "أردو" | "Urdu" | "urd" | "ur" => "Urdu".to_string(),
        "বাংলা" | "Bengali" | "ben" | "bn" => "Bengali".to_string(),
        "हिन्दी" | "हिंदी" | "Hindi" | "hin" | "hi" => "Hindi".to_string(),
        "Filipino" | "Tagalog" | "fil" | "tl" => "Filipino".to_string(),
        "Indonesian" | "ind" | "id" | "in_id" => "Indonesian".to_string(),
        "English" | "eng" | "en" => "English".to_string(),
        "Español" | "Spanish" | "spa" | "es" => "Spanish".to_string(),
        "Français" | "French" | "fra" | "fre" => "French".to_string(),
        "Deutsch" | "German" | "deu" | "ger" => "German".to_string(),
        "Italiano" | "Italian" | "ita" | "it" => "Italian".to_string(),
        "Português" | "Portuguese" | "por" | "pt" => "Portuguese".to_string(),
        "Русский" | "Russian" | "rus" | "ru" => "Russian".to_string(),
        "Türkçe" | "Turkish" | "tur" | "tr" => "Turkish".to_string(),
        "Tiếng Việt" | "Vietnamese" | "vie" | "vi" => "Vietnamese".to_string(),
        "中文" | "Chinese" | "zho" | "chi" | "zh" => "Chinese".to_string(),
        "日本語" | "Japanese" | "jpn" | "ja" => "Japanese".to_string(),
        "한국어" | "Korean" | "kor" | "ko" => "Korean".to_string(),
        "ไทย" | "Thai" | "tha" | "th" => "Thai".to_string(),
        "தமிழ்" | "Tamil" | "tam" | "ta" => "Tamil".to_string(),
        "తెలుగు" | "Telugu" | "tel" | "te" => "Telugu".to_string(),
        "മലയാളം" | "Malayalam" | "mal" | "ml" => "Malayalam".to_string(),
        "ಕನ್ನಡ" | "Kannada" | "kan" | "kn" => "Kannada".to_string(),
        "मराठी" | "Marathi" | "mar" | "mr" => "Marathi".to_string(),
        "ગુજરાતી" | "Gujarati" | "guj" | "gu" => "Gujarati".to_string(),
        "ਪੰਜਾਬੀ" | "Punjabi" | "pan" | "pa" => "Punjabi".to_string(),
        "فارسی" | "Persian" | "fas" | "per" | "fa" => "Persian".to_string(),
        "עברית" | "Hebrew" | "heb" | "he" => "Hebrew".to_string(),
        "Ελληνικά" | "Greek" | "ell" | "gre" | "el" => "Greek".to_string(),
        "Polski" | "Polish" | "pol" | "pl" => "Polish".to_string(),
        "Nederlands" | "Dutch" | "nld" | "dut" | "nl" => "Dutch".to_string(),
        "Svenska" | "Swedish" | "swe" | "sv" => "Swedish".to_string(),
        "Norsk" | "Norwegian" | "nor" | "no" => "Norwegian".to_string(),
        "Dansk" | "Danish" | "dan" | "da" => "Danish".to_string(),
        "Suomi" | "Finnish" | "fin" | "fi" => "Finnish".to_string(),
        "Čeština" | "Czech" | "ces" | "cze" | "cs" => "Czech".to_string(),
        "Magyar" | "Hungarian" | "hun" | "hu" => "Hungarian".to_string(),
        "Română" | "Romanian" | "ron" | "rum" | "ro" => "Romanian".to_string(),
        "Українська" | "Ukrainian" | "ukr" | "uk" => "Ukrainian".to_string(),
        "" => "Unknown".to_string(),
        other => other.to_string(),
    }
}

/// Formats a number of seconds as `H:MM:SS` (or `M:SS` under an hour).
pub fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Parses a duration into seconds.
///
/// Accepts `MM:SS`, `HH:MM:SS` and the compact `1h30m` / `45m` / `90s` forms.
pub fn parse_duration_seconds(d: &str) -> Option<u64> {
    let s = d.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("n/a") {
        return None;
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let m: u64 = parts[0].trim().parse().ok()?;
            let s: u64 = parts[1].trim().parse().ok()?;
            return Some(m * 60 + s);
        } else if parts.len() == 3 {
            let h: u64 = parts[0].trim().parse().ok()?;
            let m: u64 = parts[1].trim().parse().ok()?;
            let s: u64 = parts[2].trim().parse().ok()?;
            return Some(h * 3600 + m * 60 + s);
        }
    }
    let mut total = 0u64;
    let mut current_num = String::new();
    let mut found_any = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else if c == 'h' || c == 'H' {
            if let Ok(n) = current_num.parse::<u64>() {
                total += n * 3600;
                found_any = true;
            }
            current_num.clear();
        } else if c == 'm' || c == 'M' {
            if let Ok(n) = current_num.parse::<u64>() {
                total += n * 60;
                found_any = true;
            }
            current_num.clear();
        } else if c == 's' || c == 'S' {
            if let Ok(n) = current_num.parse::<u64>() {
                total += n;
                found_any = true;
            }
            current_num.clear();
        }
    }
    if !current_num.is_empty() && !found_any {
        if let Ok(n) = current_num.parse::<u64>() {
            total += n * 60;
            found_any = true;
        }
    }
    if found_any && total > 0 {
        Some(total)
    } else {
        None
    }
}

/// Extracts the `host[:port]` authority part of an HTTP(S) URL.
///
/// Returns `None` for non-HTTP URLs. The port is preserved when present.
pub fn extract_host_authority(url: &str) -> Option<String> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = after_scheme.split('/').next()?;
    if authority.is_empty() {
        None
    } else {
        Some(authority.to_string())
    }
}

/// Decides whether a request header should be forwarded to the upstream host.
///
/// `forward_all` is set when the target shares the authority of the original
/// stream; otherwise only the headers that are required to keep the CDN happy
/// (`User-Agent`, `Referer`) are forwarded.
pub fn should_forward_header(forward_all: bool, name: &str) -> bool {
    forward_all || name.eq_ignore_ascii_case("user-agent") || name.eq_ignore_ascii_case("referer")
}

/// Whether `address` is reachable from the public internet — used to reject
/// loopback, link-local and private ranges in stream hosts.
pub fn is_public_ip(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(address) => {
            !(address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_broadcast()
                || address.is_documentation()
                || address.is_unspecified())
        }
        std::net::IpAddr::V6(address) => {
            !(address.is_loopback()
                || address.is_unspecified()
                || address.is_unique_local()
                || address.is_unicast_link_local())
        }
    }
}

/// Whether a stream host supplied by an upstream API is safe to request from:
/// localhost, `.local` names and private IP ranges are rejected so a
/// compromised provider cannot point the client at the local network.
pub fn is_safe_stream_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() || host == "localhost" || host.ends_with(".local") {
        return false;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(address) => is_public_ip(address),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_labels_are_canonicalized() {
        assert_eq!(sanitize_language_label("English"), "English");
        assert_eq!(sanitize_language_label("eng"), "English");
        assert_eq!(sanitize_language_label(""), "Unknown");
        assert_eq!(sanitize_language_label("हिन्दी"), "Hindi");
    }

    #[test]
    fn durations_round_trip() {
        assert_eq!(format_duration(3661), "1:01:01");
        assert_eq!(format_duration(61), "1:01");
        assert_eq!(parse_duration_seconds("1:01:01"), Some(3661));
        assert_eq!(parse_duration_seconds("1h30m"), Some(5400));
        assert_eq!(parse_duration_seconds("n/a"), None);
        assert_eq!(parse_duration_seconds(""), None);
    }

    #[test]
    fn stream_hosts_are_validated() {
        assert!(is_safe_stream_host("cdn.example.com"));
        assert!(is_safe_stream_host("8.8.8.8"));
        assert!(!is_safe_stream_host("localhost"));
        assert!(!is_safe_stream_host("printer.local"));
        assert!(!is_safe_stream_host("127.0.0.1"));
        assert!(!is_safe_stream_host("10.1.2.3"));
        assert!(!is_safe_stream_host("169.254.1.1"));
        assert!(!is_safe_stream_host("fd00::1"));
        assert!(!is_safe_stream_host("  "));
    }
}
