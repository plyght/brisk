pub const BRISK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let version = version.trim_start_matches('v');
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.split('-').next()?.parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_accepts_v_prefix() {
        assert_eq!(parse_semver("v0.1.2"), Some((0, 1, 2)));
    }

    #[test]
    fn parse_semver_ignores_prerelease() {
        assert_eq!(parse_semver("1.2.3-beta.1"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_semver_rejects_invalid_versions() {
        assert_eq!(parse_semver("not-a-version"), None);
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn is_newer_detects_upgrade() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.1.1", "0.1.1"));
        assert!(!is_newer("0.1.2", "0.1.1"));
    }
}
