use std::path::{Path, PathBuf};

use platform_kernel::error::AppError;

pub fn image_root() -> PathBuf {
    if let Ok(dir) = std::env::var("IMAGE_DIR") {
        return PathBuf::from(dir);
    }
    for candidate in [".", "../.."] {
        let root = Path::new(candidate);
        if root.join("config.toml").is_file() {
            return root.join("data/item-images");
        }
    }
    PathBuf::from("data/item-images")
}

pub fn known_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "jpg" => Some("jpg"),
        "png" => Some("png"),
        "webp" => Some("webp"),
        "gif" => Some("gif"),
        _ => None,
    }
}

pub fn image_file(id: i32, ext: &str) -> Option<PathBuf> {
    let ext = known_ext(ext)?;
    Some(image_root().join(format!("{id}.{ext}")))
}

pub fn content_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    }
}

pub fn sniff_image(bytes: &[u8]) -> Result<&'static str, AppError> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(AppError::BadRequest("Use a picture under 2 MB.".into()));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("jpg");
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Ok("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok("webp");
    }
    Err(AppError::BadRequest("Use a JPEG, PNG, WebP, or GIF.".into()))
}

pub fn write_image(id: i32, ext: &str, bytes: &[u8]) -> Result<(), AppError> {
    let dir = image_root();
    std::fs::create_dir_all(&dir)
        .map_err(|_| AppError::BadRequest("The picture folder could not be created.".into()))?;
    let path = image_file(id, ext)
        .ok_or_else(|| AppError::BadRequest("Use a JPEG, PNG, WebP, or GIF.".into()))?;
    let tmp = dir.join(format!("{id}.{ext}.tmp"));
    std::fs::write(&tmp, bytes)
        .map_err(|_| AppError::BadRequest("That picture could not be stored.".into()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|_| AppError::BadRequest("That picture could not be stored.".into()))?;
    Ok(())
}

pub fn remove_image_file(id: i32, ext: &str) {
    if let Some(path) = image_file(id, ext) {
        let _ = std::fs::remove_file(path);
    }
}
