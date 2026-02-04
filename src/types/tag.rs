//! Validated tag type with compile-time safety guarantees.
//! 
//! A `Tag` can only be constructed via `Tag::new()`, which validates:
//! - Non-empty after trimming
//! - No HTML special characters
//! - Reasonable length

use crate::error::BuildError;

/// A validated tag that is safe to use in HTML.
/// 
/// Invariants (enforced at construction):
/// - Non-empty
/// - No characters: `<`, `>`, `&`, `"`, `'`, `/`
/// - Max 50 characters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag(String);

impl Tag {
    /// Maximum allowed tag length.
    pub const MAX_LENGTH: usize = 50;

    /// Characters not allowed in tags (HTML-unsafe).
    const FORBIDDEN_CHARS: [char; 6] = ['<', '>', '&', '"', '\'', '/'];

    /// Attempt to create a validated Tag from raw input.
    pub fn new(raw: &str) -> Result<Self, BuildError> {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(BuildError::InvalidTag {
                tag: raw.to_string(),
                reason: "tag is empty",
            });
        }

        if trimmed.len() > Self::MAX_LENGTH {
            return Err(BuildError::InvalidTag {
                tag: raw.to_string(),
                reason: "tag exceeds 50 characters",
            });
        }

        if trimmed.chars().any(|c| Self::FORBIDDEN_CHARS.contains(&c)) {
            return Err(BuildError::InvalidTag {
                tag: raw.to_string(),
                reason: "tag contains HTML special characters",
            });
        }

        Ok(Self(trimmed.to_string()))
    }

    /// Get the validated tag as a string slice.
    /// Safe to use directly in HTML without escaping.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to lowercase for URL/filename use.
    pub fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_tag() {
        let tag = Tag::new("Rust").unwrap();
        assert_eq!(tag.as_str(), "Rust");
    }

    #[test]
    fn trims_whitespace() {
        let tag = Tag::new("  Programming  ").unwrap();
        assert_eq!(tag.as_str(), "Programming");
    }

    #[test]
    fn rejects_empty() {
        assert!(Tag::new("").is_err());
        assert!(Tag::new("   ").is_err());
    }

    #[test]
    fn rejects_html_chars() {
        assert!(Tag::new("<script>").is_err());
        assert!(Tag::new("tag&name").is_err());
        assert!(Tag::new("tag\"name").is_err());
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(51);
        assert!(Tag::new(&long).is_err());
    }

    #[test]
    fn lowercase_for_urls() {
        let tag = Tag::new("GameDev").unwrap();
        assert_eq!(tag.to_lowercase(), "gamedev");
    }
}
