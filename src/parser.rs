//! Markdown parsing with structured metadata extraction.

use std::path::Path;

use pulldown_cmark::{Event, Parser, Tag, TagEnd, html};

use crate::config::Config;
use crate::error::BuildError;
use crate::image::{OptimizedImage, optimize_image};
use crate::types::{HtmlSafe, EscapeHtml, Tag as BlogTag};

/// Parsed metadata from a markdown post.
#[derive(Debug, Clone)]
pub struct PostMetadata {
    pub title: HtmlSafe,
    pub tags: Vec<BlogTag>,
    pub raw_title: String,
}

/// Extract metadata (title, tags) from markdown content.
pub fn extract_metadata(markdown: &str, fallback_title: &str) -> PostMetadata {
    // Extract title from first H1
    let raw_title = markdown
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim())
        .unwrap_or(fallback_title)
        .to_string();

    // Extract tags from "Tags:" line
    let mut tags = Vec::new();
    if let Some(tag_line) = markdown.lines().find(|l| l.trim().starts_with("Tags:")) {
        let tag_str = tag_line.trim_start_matches("Tags:").trim();
        for tag in tag_str.split(',') {
            match BlogTag::new(tag) {
                Ok(t) => tags.push(t),
                Err(e) => {
                    // Log but don't fail - skip invalid tags
                    eprintln!("  ⚠ Skipping invalid tag: {}", e);
                }
            }
        }
    }

    PostMetadata {
        title: raw_title.escape_html(),
        tags,
        raw_title,
    }
}

/// Convert markdown to HTML with custom image handling.
pub fn render_markdown(
    markdown: &str,
    config: &Config,
    content_dir: &Path,
    public_dir: &Path,
    relative_root: &str,
) -> Result<String, BuildError> {
    let parser = Parser::new(markdown);
    
    let mut events: Vec<Event<'_>> = Vec::new();
    let mut in_image = false;
    let mut image_url = String::new();
    let mut image_title = String::new();
    let mut image_alt = String::new();
    let mut first_image = true;

    for event in parser {
        match event {
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                in_image = true;
                image_url = dest_url.to_string();
                image_title = title.to_string();
                image_alt.clear();
            }
            Event::End(TagEnd::Image) => {
                in_image = false;
                
                // Optimize image
                let opt = optimize_image(
                    &image_url,
                    content_dir,
                    public_dir,
                    config.max_image_width,
                ).unwrap_or_else(|_| OptimizedImage::missing(&image_url));

                // Build final src URL
                let final_src = if opt.is_external() {
                    opt.rel_path.clone()
                } else {
                    format!("{}{}", relative_root, opt.rel_path)
                };
                let final_src_escaped = final_src.escape_html();

                // Build dimension attributes
                let (width_attr, height_attr) = parse_dimensions_or_image(
                    &image_title,
                    opt.width,
                    opt.height,
                );

                // Escape alt text for XSS prevention
                let safe_alt = image_alt.escape_html();
                
                // Title attribute (only if not a dimension spec)
                let title_attr = if !is_dimension_spec(&image_title) && !image_title.is_empty() {
                    let safe_title = image_title.escape_html();
                    format!(r#"title="{}""#, safe_title)
                } else {
                    String::new()
                };

                // Loading strategy
                let loading_attrs = if first_image {
                    first_image = false;
                    r#"loading="eager" fetchpriority="high" decoding="sync""#
                } else {
                    r#"loading="lazy" decoding="async""#
                };

                let html = format!(
                    r#"<figure class="image-container">
                        <img src="{}" alt="{}" {} {} {} {} />
                        <figcaption>
                            <a href="{}" target="_blank" class="download-link">[ Download Full Size ]</a>
                        </figcaption>
                    </figure>"#,
                    final_src_escaped,
                    safe_alt,
                    width_attr,
                    height_attr,
                    title_attr,
                    loading_attrs,
                    final_src_escaped,
                );
                events.push(Event::Html(html.into()));
            }
            Event::Text(text) if in_image => {
                image_alt.push_str(&text);
            }
            Event::Code(text) if in_image => {
                image_alt.push_str(&text);
            }
            e if !in_image => {
                events.push(e);
            }
            _ => {}
        }
    }

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    
    Ok(html_output)
}

/// Parse dimension specification from title or use from image.
fn parse_dimensions_or_image(title: &str, img_w: u32, img_h: u32) -> (String, String) {
    let clean = title.trim();
    
    // Try "WxH" format
    if let Some(x_pos) = clean.find('x') {
        let (w_str, h_str) = clean.split_at(x_pos);
        let h_str = &h_str[1..];
        if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
            return (format!(r#"width="{}""#, w), format!(r#"height="{}""#, h));
        }
    }
    
    // Try single width value
    if let Ok(w) = clean.parse::<u32>() {
        return (format!(r#"width="{}""#, w), String::new());
    }
    
    // Use image dimensions if available
    if img_w > 0 && img_h > 0 {
        return (format!(r#"width="{}""#, img_w), format!(r#"height="{}""#, img_h));
    }
    
    (String::new(), String::new())
}

/// Check if title is a dimension specification.
fn is_dimension_spec(title: &str) -> bool {
    let clean = title.trim();
    if clean.is_empty() {
        return false;
    }
    
    // "WxH" format
    if let Some(x_pos) = clean.find('x') {
        let (w_str, h_str) = clean.split_at(x_pos);
        let h_str = &h_str[1..];
        return w_str.parse::<u32>().is_ok() && h_str.parse::<u32>().is_ok();
    }
    
    // Single number
    clean.parse::<u32>().is_ok()
}
