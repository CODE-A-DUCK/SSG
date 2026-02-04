//! HTML template rendering with type-safe content.

use std::collections::HashSet;

use crate::config::Config;
use crate::types::{HtmlSafe, EscapeHtml, Tag};

/// Render context with optional CSS content and LCP preload.
pub struct RenderContext<'a> {
    pub config: &'a Config,
    pub inline_css: Option<&'a str>,
    pub lcp_image_url: Option<String>, // Owned to avoid lifetime issues
}

impl<'a> RenderContext<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            config,
            inline_css: None,
            lcp_image_url: None,
        }
    }

    pub fn with_css(mut self, css: &'a str) -> Self {
        self.inline_css = Some(css);
        self
    }

    pub fn with_lcp_image(mut self, url: impl Into<String>) -> Self {
        self.lcp_image_url = Some(url.into());
        self
    }
}

/// Render the HTML page template.
pub fn template(
    title: &HtmlSafe,
    content: &str,
    all_tags: &HashSet<Tag>,
    relative_root: &str,
    ctx: &RenderContext<'_>,
) -> String {
    let mut sorted_tags: Vec<_> = all_tags.iter().collect();
    sorted_tags.sort_by_key(|t| t.as_str());
    
    let index_link = format!("{}index.html", relative_root);
    let brand = ctx.config.brand_name.escape_html();
    
    let mut nav_html = format!(
        r#"<div class="nav-section"><a href="{}" class="nav-link main-link">Index</a></div>"#,
        index_link
    );
    
    if !sorted_tags.is_empty() {
        nav_html.push_str(r#"<div class="nav-section"><span class="nav-header">Filter</span>"#);
        for tag in sorted_tags {
            let tag_lower = tag.to_lowercase();
            let link = format!("{}tags/tag_{}.html", relative_root, tag_lower);
            nav_html.push_str(&format!(
                r#"<a href="{}" class="nav-link tag-link">{}</a>"#,
                link, tag
            ));
        }
        nav_html.push_str("</div>");
    }

    // CSS: either inline or external link
    let css_block = if let Some(css) = ctx.inline_css {
        format!("<style>{}</style>", css)
    } else {
        format!(r#"<link rel="stylesheet" href="{}style.css">"#, relative_root)
    };

    // LCP preload hint for first image
    let preload_block = if let Some(ref lcp_url) = ctx.lcp_image_url {
        format!(r#"<link rel="preload" as="image" href="{}" fetchpriority="high">"#, lcp_url)
    } else {
        String::new()
    };

    format!(
r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{brand} | {title}</title>
    <link rel="icon" href="{relative_root}favicon.ico" type="image/x-icon">
    {css_block}
    {preload_block}
</head>
<body>
    <header>
        <span class="brand">[ {brand} ]</span>
        <nav>
            {nav_html}
        </nav>
    </header>
    <article>
        {content}
    </article>
</body>
</html>"##
    )
}

/// Legacy template function for backwards compatibility.
pub fn template_simple(
    title: &HtmlSafe,
    content: &str,
    all_tags: &HashSet<Tag>,
    relative_root: &str,
    config: &Config,
) -> String {
    let ctx = RenderContext::new(config);
    template(title, content, all_tags, relative_root, &ctx)
}

/// Generate metadata header for a post.
pub fn render_post_meta(date: &str, tags: &[Tag]) -> String {
    let tags_html: String = tags
        .iter()
        .map(|t| format!(r#"<span class="tag">#{}</span>"#, t))
        .collect();
    
    let safe_date = date.escape_html();
    
    format!(
        r#"<div class="meta"><span class="meta-item">UPLOAD: {}</span> <span class="meta-item">{}</span></div>"#,
        safe_date, tags_html
    )
}

/// Generate the post list HTML for index/tag pages.
pub fn render_post_list(posts: &[PostListItem], relative_root: &str) -> String {
    let mut html = String::from(r#"<div class="post-list">"#);
    
    for post in posts {
        let tags_html: String = post.tags
            .iter()
            .map(|t| format!(r#"<span class="tag">#{}</span>"#, t))
            .collect();

        let link = format!("{}{}", relative_root, post.filename);
        let safe_date = post.date.escape_html();

        html.push_str(&format!(
            r#"<div class="post-entry"><a href="{}"><span class="entry-title">{} {}</span><span class="entry-date">{}</span></a></div>"#,
            link, post.title, tags_html, safe_date
        ));
    }
    
    html.push_str("</div>");
    html
}

/// Item in the post list (for index/tag pages).
#[derive(Debug, Clone)]
pub struct PostListItem {
    pub title: HtmlSafe,
    pub filename: String,
    pub date: String,
    pub tags: Vec<Tag>,
}
