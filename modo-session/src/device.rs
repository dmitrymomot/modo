/// Parse a human-readable device name from User-Agent string.
/// Returns format like "Chrome on macOS", "Safari on iPhone", etc.
pub fn parse_device_name(user_agent: &str) -> String {
    let browser = parse_browser(user_agent);
    let os = parse_os(user_agent);
    format!("{browser} on {os}")
}

/// Parse device type from User-Agent string.
/// Returns "mobile", "tablet", or "desktop".
pub fn parse_device_type(user_agent: &str) -> String {
    let ua = user_agent.to_lowercase();
    if ua.contains("tablet") || ua.contains("ipad") {
        "tablet".to_string()
    } else if ua.contains("mobile")
        || ua.contains("iphone")
        || (ua.contains("android") && !ua.contains("tablet"))
    {
        "mobile".to_string()
    } else {
        "desktop".to_string()
    }
}

fn parse_browser(ua: &str) -> &str {
    if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Chromium/") {
        "Chromium"
    } else if ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        "Unknown"
    }
}

fn parse_os(ua: &str) -> &str {
    if ua.contains("iPhone") {
        "iPhone"
    } else if ua.contains("iPad") {
        "iPad"
    } else if ua.contains("HarmonyOS") {
        "HarmonyOS"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("CrOS") {
        "ChromeOS"
    } else if ua.contains("Mac OS X") || ua.contains("Macintosh") || ua.contains("OS X") {
        "macOS"
    } else if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("FreeBSD") {
        "FreeBSD"
    } else if ua.contains("OpenBSD") {
        "OpenBSD"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "Unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_on_macos() {
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        assert_eq!(parse_device_name(ua), "Chrome on macOS");
        assert_eq!(parse_device_type(ua), "desktop");
    }

    #[test]
    fn safari_on_iphone() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
        assert_eq!(parse_device_name(ua), "Safari on iPhone");
        assert_eq!(parse_device_type(ua), "mobile");
    }

    #[test]
    fn firefox_on_windows() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0";
        assert_eq!(parse_device_name(ua), "Firefox on Windows");
        assert_eq!(parse_device_type(ua), "desktop");
    }

    #[test]
    fn edge_on_windows() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0";
        assert_eq!(parse_device_name(ua), "Edge on Windows");
        assert_eq!(parse_device_type(ua), "desktop");
    }

    #[test]
    fn chrome_on_android_mobile() {
        let ua = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";
        assert_eq!(parse_device_name(ua), "Chrome on Android");
        assert_eq!(parse_device_type(ua), "mobile");
    }

    #[test]
    fn safari_on_ipad() {
        let ua = "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
        assert_eq!(parse_device_name(ua), "Safari on iPad");
        assert_eq!(parse_device_type(ua), "tablet");
    }

    #[test]
    fn chrome_on_linux() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        assert_eq!(parse_device_name(ua), "Chrome on Linux");
        assert_eq!(parse_device_type(ua), "desktop");
    }

    #[test]
    fn unknown_ua() {
        assert_eq!(parse_device_name("curl/7.88.1"), "Unknown on Unknown");
        assert_eq!(parse_device_type("curl/7.88.1"), "desktop");
    }

    #[test]
    fn empty_ua() {
        assert_eq!(parse_device_name(""), "Unknown on Unknown");
        assert_eq!(parse_device_type(""), "desktop");
    }
}
