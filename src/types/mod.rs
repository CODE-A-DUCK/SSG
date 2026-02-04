//! Type-safe wrappers for validated content.

mod tag;
mod html_safe;

pub use tag::Tag;
pub use html_safe::{HtmlSafe, EscapeHtml};
