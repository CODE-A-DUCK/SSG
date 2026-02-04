//! Image optimization with caching and modification time checking.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use image::GenericImageView;

use crate::error::BuildError;

/// Result of image optimization.
#[derive(Debug, Clone)]
pub struct OptimizedImage {
    /// Relative path from public root (e.g., "images/photo.webp").
    pub rel_path: String,
    
    /// Image width in pixels (0 if unknown).
    pub width: u32,
    
    /// Image height in pixels (0 if unknown).
    pub height: u32,
}

impl OptimizedImage {
    /// Create for external URLs (no processing needed).
    pub fn external(url: &str) -> Self {
        Self {
            rel_path: url.to_string(),
            width: 0,
            height: 0,
        }
    }

    /// Create for missing/invalid images.
    pub fn missing(original_path: &str) -> Self {
        Self {
            rel_path: original_path.to_string(),
            width: 0,
            height: 0,
        }
    }

    /// Check if this is an external URL.
    pub fn is_external(&self) -> bool {
        self.rel_path.starts_with("http://") || self.rel_path.starts_with("https://")
    }
}

/// Optimize a local image to WebP format with caching.
///
/// # Cache behavior
/// - If destination exists and is newer than source, returns cached version
/// - Otherwise, regenerates the optimized image
///
/// # Arguments
/// * `original_src` - Source path relative to content_dir
/// * `content_dir` - Root directory for content
/// * `public_dir` - Root directory for output
/// * `max_width` - Maximum width (larger images are resized)
pub fn optimize_image(
    original_src: &str,
    content_dir: &Path,
    public_dir: &Path,
    max_width: u32,
) -> Result<OptimizedImage, BuildError> {
    // External URLs pass through unchanged
    if original_src.starts_with("http://") || original_src.starts_with("https://") {
        return Ok(OptimizedImage::external(original_src));
    }

    let src_path = content_dir.join(original_src);
    
    // Check source exists
    if !src_path.exists() {
        // Not an error, just fallback to original path
        return Ok(OptimizedImage::missing(original_src));
    }

    // Generate destination path
    let file_stem = src_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| BuildError::Internal(format!(
            "Invalid image filename: {:?}", src_path
        )))?;
    
    let dest_filename = format!("{file_stem}.webp");
    let dest_path = public_dir.join("images").join(&dest_filename);
    let rel_path = format!("images/{dest_filename}");

    // Cache check: compare modification times
    if dest_path.exists() {
        if let (Ok(src_meta), Ok(dest_meta)) = (fs::metadata(&src_path), fs::metadata(&dest_path)) {
            let src_mtime = src_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let dest_mtime = dest_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            
            // Cache hit: destination is newer
            if dest_mtime >= src_mtime {
                return read_cached_dimensions(&dest_path, rel_path);
            }
        }
    }

    // Process image
    println!("  → Optimizing: {:?}", src_path);
    
    let img = image::open(&src_path).map_err(|e| BuildError::ImageOptFailed {
        path: src_path.clone(),
        source: e,
    })?;

    let (width, _) = img.dimensions();
    
    let final_img = if width > max_width {
        img.resize(max_width, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let (new_width, new_height) = final_img.dimensions();

    // Save as WebP
    final_img
        .save_with_format(&dest_path, image::ImageFormat::WebP)
        .map_err(|e| BuildError::ImageOptFailed {
            path: dest_path.clone(),
            source: e,
        })?;

    Ok(OptimizedImage {
        rel_path,
        width: new_width,
        height: new_height,
    })
}

/// Read dimensions from a cached WebP file.
fn read_cached_dimensions(path: &Path, rel_path: String) -> Result<OptimizedImage, BuildError> {
    match image::ImageReader::open(path) {
        Ok(reader) => match reader.into_dimensions() {
            Ok((w, h)) => Ok(OptimizedImage {
                rel_path,
                width: w,
                height: h,
            }),
            Err(_) => Ok(OptimizedImage {
                rel_path,
                width: 0,
                height: 0,
            }),
        },
        Err(_) => Ok(OptimizedImage {
            rel_path,
            width: 0,
            height: 0,
        }),
    }
}
