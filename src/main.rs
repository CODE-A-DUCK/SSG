//! Blog generator main entry point.
//!
//! Orchestrates the build process using the library modules.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, Utc};
use rayon::prelude::*;

use generator::config::Config;
use generator::error::{BuildError, BuildResult};
use generator::parser::{extract_metadata, render_markdown, PostMetadata};
use generator::renderer::{template, render_post_meta, render_post_list, PostListItem, RenderContext};
use generator::types::{HtmlSafe, Tag};

fn main() -> Result<(), BuildError> {
    let start_time = std::time::Instant::now();
    println!("Building blog (Multi-threaded)...");
    
    let config = Config::new();
    
    // Create output directories
    fs::create_dir_all(config.posts_dir()).map_err(|e| BuildError::OutputNotWritable {
        path: config.posts_dir(),
        source: e,
    })?;
    fs::create_dir_all(config.tags_dir()).map_err(|e| BuildError::OutputNotWritable {
        path: config.tags_dir(),
        source: e,
    })?;
    fs::create_dir_all(config.images_dir()).map_err(|e| BuildError::OutputNotWritable {
        path: config.images_dir(),
        source: e,
    })?;

    // Load CSS for inlining (eliminates render-blocking)
    let css_content = if config.inline_css {
        let css_path = config.content_dir.join("style.css");
        match fs::read_to_string(&css_path) {
            Ok(css) => {
                println!("  → CSS will be inlined ({} bytes)", css.len());
                Some(css)
            }
            Err(_) => {
                eprintln!("  ⚠ CSS file not found for inlining, using external link");
                None
            }
        }
    } else {
        None
    };

    // Copy static assets (only favicon now if CSS is inlined, or both if not)
    let static_files: Vec<&str> = if css_content.is_some() {
        vec!["favicon.ico"]
    } else {
        vec!["favicon.ico", "style.css"]
    };
    
    for file in static_files {
        let src = config.content_dir.join(file);
        if src.exists() {
            if let Err(e) = fs::copy(&src, config.public_dir.join(file)) {
                eprintln!("  ⚠ Failed to copy {}: {}", file, e);
            }
        }
    }

    // Phase 1: Discover markdown files (IO-bound, sequential)
    let entries = fs::read_dir(&config.content_dir).map_err(|e| BuildError::ContentNotReadable {
        path: config.content_dir.clone(),
        source: e,
    })?;
    
    let paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();

    println!("Found {} markdown files.", paths.len());

    // Phase 2: Parse metadata (CPU-bound, parallel)
    let parsed_results: Vec<_> = paths.par_iter()
        .map(|path| parse_post(path, &config))
        .collect();

    // Collect results and tags
    let mut build_result = BuildResult::new();
    let mut valid_posts: Vec<ParsedPost> = Vec::new();
    let mut all_tags: HashSet<Tag> = HashSet::new();

    for res in parsed_results {
        match res {
            Ok(post) => {
                for tag in &post.metadata.tags {
                    all_tags.insert(tag.clone());
                }
                valid_posts.push(post);
                build_result.record_success();
            }
            Err(e) => build_result.record_failure(e),
        }
    }

    println!("Parsed {} valid posts. Generating HTML...", valid_posts.len());

    // Phase 3: Render HTML (CPU-bound, parallel)
    let css_ref = css_content.as_deref();
    let render_results: Vec<_> = valid_posts.par_iter()
        .map(|post| render_post(post, &all_tags, &config, css_ref))
        .collect();

    for res in render_results {
        if let Err(e) = res {
            build_result.record_failure(e);
        }
    }

    // Phase 4: Generate index pages (sequential)
    let post_items: Vec<PostListItem> = valid_posts.iter()
        .map(|p| PostListItem {
            title: p.metadata.title.clone(),
            filename: format!("posts/{}.html", p.file_stem),
            date: p.date.clone(),
            tags: p.metadata.tags.clone(),
        })
        .collect();

    // Sort by filename (newest first based on naming convention)
    let mut sorted_items = post_items;
    sorted_items.sort_by(|a, b| b.filename.cmp(&a.filename));

    // Generate main index
    generate_list_page(&sorted_items, &all_tags, "Index", config.public_dir.join("index.html"), "", &config, css_ref)?;

    // Generate tag pages
    for tag in &all_tags {
        let tag_posts: Vec<_> = sorted_items.iter()
            .filter(|p| p.tags.contains(tag))
            .cloned()
            .collect();
        
        let filename = format!("tag_{}.html", tag.to_lowercase());
        let title = format!("Tag: {}", tag);
        generate_list_page(&tag_posts, &all_tags, &title, config.tags_dir().join(&filename), "../", &config, css_ref)?;
    }
    
    let duration = start_time.elapsed();
    
    // Finalize and report
    match build_result.finalize() {
        Ok(summary) => {
            summary.print_report();
            println!("Done! Built in {duration:.2?}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Build failed: {}", e);
            Err(e)
        }
    }
}

/// Intermediate parsed post data.
struct ParsedPost {
    file_stem: String,
    metadata: PostMetadata,
    date: String,
    content: String,
    first_image_url: Option<String>,
}

/// Parse a single markdown file.
fn parse_post(path: &PathBuf, config: &Config) -> Result<ParsedPost, BuildError> {
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| BuildError::ParseFailed {
            path: path.clone(),
            message: "Invalid filename".to_string(),
        })?
        .to_string();

    // Get modification time
    let metadata = fs::metadata(path).map_err(|e| BuildError::ParseFailed {
        path: path.clone(),
        message: format!("Failed to read metadata: {}", e),
    })?;
    
    let modified: DateTime<Utc> = metadata
        .modified()
        .map_err(|e| BuildError::ParseFailed {
            path: path.clone(),
            message: format!("Failed to get mtime: {}", e),
        })?
        .into();
    
    let offset = FixedOffset::east_opt(config.timezone_offset_hours * 3600)
        .ok_or_else(|| BuildError::Internal("Invalid timezone offset".to_string()))?;
    let modified_local = modified.with_timezone(&offset);
    let date_str = modified_local.format("%Y.%m.%d %H:%M").to_string();

    let content = fs::read_to_string(path).map_err(|e| BuildError::ParseFailed {
        path: path.clone(),
        message: format!("Failed to read file: {}", e),
    })?;

    let post_metadata = extract_metadata(&content, &file_stem);
    
    // Extract first image URL for LCP preload
    let first_image_url = extract_first_image(&content);

    println!("  ✓ {} [{}] Tags: {:?}", 
        post_metadata.raw_title,
        date_str,
        post_metadata.tags.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );

    Ok(ParsedPost {
        file_stem,
        metadata: post_metadata,
        date: date_str,
        content,
        first_image_url,
    })
}

/// Extract first image URL from markdown for LCP preload.
fn extract_first_image(content: &str) -> Option<String> {
    // Simple regex-free extraction: find ![...](...) pattern
    let start = content.find("![")?;
    let after_alt = content[start..].find("](")?;
    let url_start = start + after_alt + 2;
    let url_end = content[url_start..].find(')')?;
    Some(content[url_start..url_start + url_end].to_string())
}

/// Render a single post to HTML file.
fn render_post(post: &ParsedPost, all_tags: &HashSet<Tag>, config: &Config, css: Option<&str>) -> Result<(), BuildError> {
    let html_content = render_markdown(
        &post.content,
        config,
        &config.content_dir,
        &config.public_dir,
        "../",
    )?;

    let meta_html = render_post_meta(&post.date, &post.metadata.tags);
    let full_content = format!("{}{}", meta_html, html_content);

    // Build render context with CSS and LCP preload
    let mut ctx = RenderContext::new(config);
    if let Some(css_str) = css {
        ctx = ctx.with_css(css_str);
    }
    if let Some(ref img_url) = post.first_image_url {
        // Convert to proper relative URL for the post page
        let lcp_url = if img_url.starts_with("http") {
            img_url.clone()
        } else {
            format!("../images/{}.webp", 
                std::path::Path::new(img_url)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(img_url))
        };
        ctx = ctx.with_lcp_image(lcp_url);
    }

    let html_page = template(
        &post.metadata.title,
        &full_content,
        all_tags,
        "../",
        &ctx,
    );

    let output_path = config.posts_dir().join(format!("{}.html", post.file_stem));
    fs::write(&output_path, html_page).map_err(|e| BuildError::OutputNotWritable {
        path: output_path,
        source: e,
    })?;

    Ok(())
}

/// Generate a list page (index or tag page).
fn generate_list_page(
    posts: &[PostListItem],
    all_tags: &HashSet<Tag>,
    title: &str,
    path: PathBuf,
    relative_root: &str,
    config: &Config,
    css: Option<&str>,
) -> Result<(), BuildError> {
    let posts_html = render_post_list(posts, relative_root);
    let safe_title = HtmlSafe::escape(title);
    let content = format!("<h1>{}</h1>{}", safe_title, posts_html);

    let mut ctx = RenderContext::new(config);
    if let Some(css_str) = css {
        ctx = ctx.with_css(css_str);
    }

    let html = template(&safe_title, &content, all_tags, relative_root, &ctx);
    
    fs::write(&path, html).map_err(|e| BuildError::OutputNotWritable {
        path,
        source: e,
    })?;

    Ok(())
}