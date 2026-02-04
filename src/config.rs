//! Build configuration with typed defaults.

use std::path::{Path, PathBuf};

/// Configuration for the blog generator.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory containing markdown source files.
    pub content_dir: PathBuf,
    
    /// Directory for generated output.
    pub public_dir: PathBuf,
    
    /// Maximum image width (images larger will be resized).
    pub max_image_width: u32,
    
    /// Timezone offset in hours (for display dates).
    pub timezone_offset_hours: i32,
    
    /// Site brand name shown in header.
    pub brand_name: String,
    
    /// Whether to inline CSS into HTML (eliminates render-blocking).
    pub inline_css: bool,
}

impl Config {
    /// Create config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set content directory.
    pub fn content_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.content_dir = path.as_ref().to_path_buf();
        self
    }

    /// Builder: set public (output) directory.
    pub fn public_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.public_dir = path.as_ref().to_path_buf();
        self
    }

    /// Builder: set max image width.
    pub fn max_image_width(mut self, width: u32) -> Self {
        self.max_image_width = width;
        self
    }

    /// Builder: set timezone offset.
    pub fn timezone_offset(mut self, hours: i32) -> Self {
        self.timezone_offset_hours = hours;
        self
    }

    /// Builder: set brand name.
    pub fn brand_name(mut self, name: impl Into<String>) -> Self {
        self.brand_name = name.into();
        self
    }

    /// Get the posts output directory.
    pub fn posts_dir(&self) -> PathBuf {
        self.public_dir.join("posts")
    }

    /// Get the tags output directory.
    pub fn tags_dir(&self) -> PathBuf {
        self.public_dir.join("tags")
    }

    /// Get the images output directory.
    pub fn images_dir(&self) -> PathBuf {
        self.public_dir.join("images")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_dir: PathBuf::from("../content"),
            public_dir: PathBuf::from("../public"),
            max_image_width: 1200,
            timezone_offset_hours: 8, // GMT+8
            brand_name: String::from("CODE A DUCK"),
            inline_css: true, // Eliminate render-blocking CSS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_pattern() {
        let config = Config::new()
            .content_dir("./src")
            .max_image_width(800)
            .brand_name("My Blog");
        
        assert_eq!(config.content_dir, PathBuf::from("./src"));
        assert_eq!(config.max_image_width, 800);
        assert_eq!(config.brand_name, "My Blog");
    }

    #[test]
    fn derived_paths() {
        let config = Config::new().public_dir("./out");
        assert_eq!(config.posts_dir(), PathBuf::from("./out/posts"));
        assert_eq!(config.images_dir(), PathBuf::from("./out/images"));
    }
}
