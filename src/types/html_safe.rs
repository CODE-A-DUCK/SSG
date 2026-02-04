//! HTML-safe string wrapper that prevents XSS.
//!
//! `HtmlSafe` can only be created by escaping raw strings,
//! ensuring that all user content is properly escaped before
//! being embedded in HTML.

use std::borrow::Cow;

/// A string that has been escaped and is safe to embed in HTML.
///
/// Invariant: The inner string contains no unescaped HTML special characters.
/// This type can be directly interpolated into HTML templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlSafe(String);

impl HtmlSafe {
    /// Escape a raw string, making it safe for HTML embedding.
    ///
    /// Escapes: `&` `<` `>` `"` `'`
    pub fn escape(raw: &str) -> Self {
        let mut escaped = String::with_capacity(raw.len());
        
        for ch in raw.chars() {
            match ch {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&#x27;"),
                _ => escaped.push(ch),
            }
        }

        Self(escaped)
    }

    /// Create from a string that is already known to be safe.
    /// 
    /// # Safety (logical, not memory)
    /// The caller guarantees the string contains no unescaped HTML.
    /// Use only for static strings or programmatically generated content.
    pub fn from_trusted(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the escaped string for embedding in HTML.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if the escaped content is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for HtmlSafe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<HtmlSafe> for String {
    fn from(safe: HtmlSafe) -> String {
        safe.0
    }
}

/// Extension trait for convenient escaping.
pub trait EscapeHtml {
    fn escape_html(&self) -> HtmlSafe;
}

impl EscapeHtml for str {
    fn escape_html(&self) -> HtmlSafe {
        HtmlSafe::escape(self)
    }
}

impl EscapeHtml for String {
    fn escape_html(&self) -> HtmlSafe {
        HtmlSafe::escape(self)
    }
}

impl<'a> EscapeHtml for Cow<'a, str> {
    fn escape_html(&self) -> HtmlSafe {
        HtmlSafe::escape(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_ampersand() {
        let safe = HtmlSafe::escape("Tom & Jerry");
        assert_eq!(safe.as_str(), "Tom &amp; Jerry");
    }

    #[test]
    fn escapes_angle_brackets() {
        let safe = HtmlSafe::escape("<script>alert('xss')</script>");
        assert_eq!(safe.as_str(), "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
    }

    #[test]
    fn escapes_quotes() {
        let safe = HtmlSafe::escape(r#"He said "hello""#);
        assert_eq!(safe.as_str(), "He said &quot;hello&quot;");
    }

    #[test]
    fn preserves_safe_content() {
        let safe = HtmlSafe::escape("Hello World 123");
        assert_eq!(safe.as_str(), "Hello World 123");
    }

    #[test]
    fn extension_trait_works() {
        let safe = "test<>".escape_html();
        assert_eq!(safe.as_str(), "test&lt;&gt;");
    }

    #[test]
    fn trusted_bypasses_escape() {
        let safe = HtmlSafe::from_trusted("<b>trusted</b>");
        assert_eq!(safe.as_str(), "<b>trusted</b>");
    }
}
