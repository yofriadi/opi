//! Image input handling for CLI/TUI attachment (task 3.4).

use std::path::PathBuf;

use opi_ai::message::{ImageSource, InputContent, MediaType};

/// Detect media type from file extension.
pub fn detect_media_type(path: PathBuf) -> Option<MediaType> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(MediaType::Png),
        "jpg" | "jpeg" => Some(MediaType::Jpeg),
        "gif" => Some(MediaType::Gif),
        "webp" => Some(MediaType::WebP),
        _ => None,
    }
}

/// Load an image file and return an `InputContent::Image` with bytes source.
pub fn load_image(path: &PathBuf) -> Result<InputContent, ImageLoadError> {
    let media_type = detect_media_type(path.clone()).ok_or_else(|| ImageLoadError {
        path: path.clone(),
        reason: "unsupported image format (accepted: png, jpg/jpeg, gif, webp)".into(),
    })?;
    let data = std::fs::read(path).map_err(|e| ImageLoadError {
        path: path.clone(),
        reason: format!("failed to read file: {e}"),
    })?;
    Ok(InputContent::Image {
        source: ImageSource::Bytes { data },
        media_type,
    })
}

/// Error from loading an image file.
#[derive(Debug)]
pub struct ImageLoadError {
    pub path: PathBuf,
    pub reason: String,
}

impl std::fmt::Display for ImageLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "image load error for {}: {}",
            self.path.display(),
            self.reason
        )
    }
}

impl std::error::Error for ImageLoadError {}
