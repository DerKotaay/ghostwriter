//! Firmware version checks that change how the rM2 screen is stored.

/// Parses (major, minor) from /etc/os-release content, e.g. IMG_VERSION="3.29.1.0".
pub fn version(os_release: &str) -> Option<(u32, u32)> {
    let line = os_release.lines().find(|l| l.starts_with("IMG_VERSION=") || l.starts_with("VERSION_ID="))?;
    let value = line.split('=').nth(1)?.trim().trim_matches('"');
    let mut parts = value.split('.').map(|p| p.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??))
}

/// From 3.24 the rM2 framebuffer is 1404x1872 BGRA instead of rotated 16-bit grey.
pub fn rm2_uses_bgra(os_release: &str) -> bool {
    matches!(version(os_release), Some((major, minor)) if major > 3 || (major == 3 && minor >= 24))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_new_format() {
        assert!(rm2_uses_bgra("NAME=\"Codex Linux\"\nIMG_VERSION=\"3.29.1.0\"\n"));
        assert!(rm2_uses_bgra("IMG_VERSION=3.24.0.149"));
        assert!(rm2_uses_bgra("IMG_VERSION=\"4.0.1.2\""));
    }

    #[test]
    fn keeps_old_format() {
        assert!(!rm2_uses_bgra("IMG_VERSION=\"3.23.0.64\""));
        assert!(!rm2_uses_bgra("garbage"));
        assert!(!rm2_uses_bgra(""));
    }
}
