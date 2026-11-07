//! Reads text highlights straight from .rm v6 page files.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const HEADER_LEN: usize = 43;
const GLYPH_BLOCK: u8 = 0x03;
const TEXT_TAG: u8 = 0x5c;
const RECENT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(1500);

fn varuint(data: &[u8], mut pos: usize) -> Option<(usize, usize)> {
    let (mut value, mut shift) = (0usize, 0);
    loop {
        let byte = *data.get(pos)?;
        value |= ((byte & 0x7f) as usize) << shift;
        pos += 1;
        if byte & 0x80 == 0 {
            return Some((value, pos));
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
}

fn text_in_block(block: &[u8]) -> Option<String> {
    for i in 0..block.len() {
        if block[i] != TEXT_TAG || i + 5 > block.len() {
            continue;
        }
        let length = u32::from_le_bytes([block[i + 1], block[i + 2], block[i + 3], block[i + 4]]) as usize;
        let Some((count, pos)) = varuint(block, i + 5) else { continue };
        let end = pos + 1 + count;
        if count > 0 && block.get(pos) == Some(&1) && end == i + 5 + length && end <= block.len() {
            if let Ok(text) = std::str::from_utf8(&block[pos + 1..end]) {
                return Some(text.trim().to_string());
            }
        }
    }
    None
}

/// Highlighted strings on a page, in file order. Tolerates a file caught mid-write.
pub fn extract(data: &[u8]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut pos = HEADER_LEN;
    while pos + 8 <= data.len() {
        let length = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let kind = data[pos + 7];
        let end = (pos + 8).saturating_add(length).min(data.len());
        if kind == GLYPH_BLOCK {
            if let Some(text) = text_in_block(&data[pos + 8..end]) {
                if !text.is_empty() && !found.contains(&text) {
                    found.push(text);
                }
            }
        }
        pos = end;
    }
    found
}

fn newest_page(root: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for doc in std::fs::read_dir(root).ok()?.flatten() {
        if !doc.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(pages) = std::fs::read_dir(doc.path()) else { continue };
        for page in pages.flatten() {
            let path = page.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rm") {
                continue;
            }
            if let Ok(modified) = page.metadata().and_then(|m| m.modified()) {
                if newest.as_ref().map(|(_, best)| modified > *best).unwrap_or(true) {
                    newest = Some((path, modified));
                }
            }
        }
    }
    newest
}

/// The tablet saves a new highlight a little after it is made, so this waits for that save.
pub fn wait_for_highlights(root: &Path, timeout: Duration) -> Vec<String> {
    let started = std::time::Instant::now();
    loop {
        if let Some((path, modified)) = newest_page(root) {
            let age = modified.elapsed().unwrap_or(Duration::ZERO);
            if age < RECENT || started.elapsed() >= timeout {
                let found = std::fs::read(&path).map(|d| extract(&d)).unwrap_or_default();
                if !found.is_empty() || started.elapsed() >= timeout {
                    return found;
                }
            }
        }
        if started.elapsed() >= timeout {
            return Vec::new();
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = include_bytes!("../hershey-check/fixtures/highlight.rm");

    #[test]
    fn reads_real_highlights() {
        assert_eq!(extract(SAMPLE), vec!["bruise", "interviewed"]);
    }

    #[test]
    fn survives_truncated_files() {
        for cut in 0..SAMPLE.len() {
            extract(&SAMPLE[..cut]);
        }
    }

    #[test]
    fn ignores_files_without_highlights() {
        assert!(extract(&SAMPLE[..43]).is_empty());
    }
}
