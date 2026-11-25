use super::moon::ThemePreview;
use std::collections::HashMap;
use std::io::Read;
use tracing::warn;

pub(super) fn extract_preview(vars: &HashMap<String, String>) -> ThemePreview {
    ThemePreview {
        bg: vars
            .get("--bg-main")
            .or_else(|| vars.get("--bg-card"))
            .or_else(|| vars.get("--bg-sidebar"))
            .cloned()
            .unwrap_or_else(|| "#0c0c0c".into()),
        accent: vars
            .get("--accent")
            .cloned()
            .unwrap_or_else(|| "#ffffff".into()),
        text: vars
            .get("--text-primary")
            .cloned()
            .unwrap_or_else(|| "#d8d8d8".into()),
    }
}

const MAX_ICON_SIZE: u64 = 2 * 1024 * 1024;
const MAX_BG_SIZE: u64 = 25 * 1024 * 1024;

#[derive(Debug)]
pub(super) enum BgValidationError {
    TooLarge,
    InvalidImage,
    Io,
}

pub(super) fn validate_background_image(path: &std::path::Path) -> Result<(), BgValidationError> {
    let mut file = std::fs::File::open(path).map_err(|_| BgValidationError::Io)?;
    let meta = file.metadata().map_err(|_| BgValidationError::Io)?;
    if meta.len() > MAX_BG_SIZE {
        return Err(BgValidationError::TooLarge);
    }

    let mut buf = [0u8; 16];
    file.read_exact(&mut buf)
        .map_err(|_| BgValidationError::Io)?;

    if infer::is_image(&buf) {
        Ok(())
    } else {
        Err(BgValidationError::InvalidImage)
    }
}

pub(super) fn validate_theme_icon(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("svg") => std::fs::metadata(path).is_ok(),
        Some("png" | "webp" | "jpg" | "jpeg") => {
            if let Ok(meta) = std::fs::metadata(path)
                && meta.len() > MAX_ICON_SIZE
            {
                warn!("Icon too large ({} bytes): {}", meta.len(), path);
                return false;
            }
            let is_image = std::fs::File::open(path)
                .ok()
                .and_then(|mut f| {
                    let mut buf = [0u8; 16];
                    f.read_exact(&mut buf).ok()?;
                    Some(infer::is_image(&buf))
                })
                .unwrap_or(false);
            if !is_image {
                warn!("Icon is not a valid image: {}", path);
            }
            is_image
        }
        _ => {
            warn!("Unsupported icon extension: {}", path);
            false
        }
    }
}
