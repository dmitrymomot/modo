use sha2::{Digest, Sha256};
use std::fmt::Write;

/// Compute a server-side fingerprint from stable request attributes.
/// Uses `\x00` separators to prevent ambiguity between inputs.
pub fn compute_fingerprint(
    user_agent: &str,
    accept_language: &str,
    accept_encoding: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_agent.as_bytes());
    hasher.update(b"\x00");
    hasher.update(accept_language.as_bytes());
    hasher.update(b"\x00");
    hasher.update(accept_encoding.as_bytes());
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        write!(s, "{b:02x}").expect("writing to String cannot fail");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_sha256_hex() {
        let fp = compute_fingerprint("test", "en", "gzip");
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_deterministic() {
        let a = compute_fingerprint("Mozilla/5.0", "en-US", "gzip");
        let b = compute_fingerprint("Mozilla/5.0", "en-US", "gzip");
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_varies_on_input_change() {
        let a = compute_fingerprint("Mozilla/5.0", "en-US", "gzip");
        let b = compute_fingerprint("Mozilla/5.0", "fr-FR", "gzip");
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_separator_prevents_collision() {
        let a = compute_fingerprint("ab", "cd", "ef");
        let b = compute_fingerprint("abc", "de", "f");
        assert_ne!(a, b);
    }
}
