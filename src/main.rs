use anyhow::{Context, Result};
use pulldown_cmark::{html, Parser, Event, Tag, TagEnd};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use chrono::{DateTime, Utc, FixedOffset};
use image::GenericImageView;
use rayon::prelude::*;

#[derive(Clone)]
struct Post {
    title: String,
    filename: String,
    date: String,
    tags: Vec<String>,
}

struct ImageResult {
    rel_path: String,
    width: u32,
    height: u32,
}

fn main() -> Result<()> {
    let start_time = std::time::Instant::now();
    println!("Building blog (Multi-threaded)...");
    
    let content_dir = Path::new("../content");
    let public_dir = Path::new("../public");
    
    // create directories (idempotent)
    let posts_dir = public_dir.join("posts");
    let tags_dir = public_dir.join("tags");
    let images_dir = public_dir.join("images");

    fs::create_dir_all(&posts_dir).context("Failed to create posts dir")?;
    fs::create_dir_all(&tags_dir).context("Failed to create tags dir")?;
    fs::create_dir_all(&images_dir).context("Failed to create images dir")?;

    let entries = fs::read_dir(content_dir).context("Failed to read content dir")?;
    
    // Collect all valid markdown paths first
    let paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();

    println!("Found {} markdown files.", paths.len());

    // Pass 1: Parse Metadata & Content (Parallel)
    // We collect results to separate successes from failures
    let parsed_results: Vec<Result<(Post, String)>> = paths.par_iter()
        .map(|path| -> Result<(Post, String)> {
            let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();
            
            // get metadata
            let metadata = fs::metadata(path).with_context(|| format!("Failed to read metadata for {:?}", path))?;
            let modified: DateTime<Utc> = metadata.modified()?.into();
            let offset = FixedOffset::east_opt(8 * 3600).context("Invalid offset")?;
            let modified_gmt8 = modified.with_timezone(&offset);
            
            let date_str = modified_gmt8.format("%Y.%m.%d %H:%M").to_string();
            
            let markdown_input = fs::read_to_string(path).with_context(|| format!("Failed to read file {:?}", path))?;
            
            // extract title
            let title = markdown_input.lines()
                .find(|l| l.starts_with("# "))
                .map(|l| l.trim_start_matches("# ").trim())
                .unwrap_or(&file_stem)
                .to_string();

            // extract tags
            let mut tags = Vec::new();
            if let Some(tag_line) = markdown_input.lines().find(|l| l.trim().starts_with("Tags:")) {
                let tag_str = tag_line.trim_start_matches("Tags:").trim();
                for tag in tag_str.split(',') {
                    let t = tag.trim().to_string();
                    if !t.is_empty() {
                        tags.push(t.clone());
                    }
                }
            }

            // Return Post struct and raw content
            Ok((
                Post {
                    title,
                    filename: format!("posts/{file_stem}.html"),
                    date: date_str,
                    tags,
                },
                markdown_input
            ))
        })
        .collect();

    let mut valid_posts_data = Vec::new();
    let mut all_tags = HashSet::new();
    let mut errors = Vec::new();

    for res in parsed_results {
        match res {
            Ok((post, content)) => {
                for t in &post.tags {
                    all_tags.insert(t.clone());
                }
                valid_posts_data.push((post, content));
            }
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() {
        eprintln!("Warning: {} files failed to parse:", errors.len());
        for e in &errors {
            eprintln!("  - {:#}", e);
        }
        // We continue with valid posts
    }

    println!("Parsed {} valid posts. Generating HTML...", valid_posts_data.len());

    // Pass 2: Generate HTML (Parallel)
    // We now have complete `all_tags` for consistent navigation
    let build_results: Vec<Result<()>> = valid_posts_data.par_iter()
        .map(|(post, markdown_input)| -> Result<()> {
            let file_stem = Path::new(&post.filename)
                .file_stem().unwrap().to_string_lossy();

            println!("Processing: {} [{}] Tags: {:?}", post.title, post.date, post.tags);

            let html_output = process_markdown(
                markdown_input, 
                &post.title, 
                &post.date, 
                &post.tags, 
                &all_tags, 
                "../", 
                content_dir, 
                public_dir
            ).with_context(|| format!("Failed to process markdown for {}", post.title))?; 
            
            let output_path = posts_dir.join(format!("{}.html", file_stem));
            fs::write(&output_path, html_output).with_context(|| format!("Failed to write html for {}", post.title))?;
            
            Ok(())
        })
        .collect();

    let mut build_errors = Vec::new();
    for res in build_results {
        if let Err(e) = res {
            build_errors.push(e);
        }
    }

    if !build_errors.is_empty() {
        eprintln!("Error: {} posts failed to build:", build_errors.len());
        for e in &build_errors {
            eprintln!("  - {:#}", e);
        }
    }

    // sort posts for index
    // We need just the Post structs now
    let mut posts: Vec<Post> = valid_posts_data.into_iter().map(|(p, _)| p).collect();
    posts.sort_by(|a, b| b.filename.cmp(&a.filename));

    // generate main index
    generate_list_page(&posts, &all_tags, "Index", public_dir.join("index.html"), "")?;

    // generate tag pages
    for tag in &all_tags {
        let tag_posts: Vec<Post> = posts.iter()
            .filter(|p| p.tags.contains(tag))
            .cloned()
            .collect();
        
        let tag_lower = tag.to_lowercase();
        let filename = format!("tag_{tag_lower}.html");
        generate_list_page(&tag_posts, &all_tags, &format!("Tag: {tag}"), tags_dir.join(&filename), "../")?;
    }
    
    let duration = start_time.elapsed();
    println!("Done! Built in {duration:.2?}");
    
    // Return error if critical failures occurred, otherwise Ok
    if !errors.is_empty() || !build_errors.is_empty() {
        // Optional: return generic error or just Ok if partial success is allowed
        // returning Ok to indicate "process finished", assuming logs are enough.
    }
    
    Ok(())
}

fn generate_list_page(posts: &[Post], all_tags: &HashSet<String>, title: &str, path: PathBuf, relative_root: &str) -> Result<()> {
    let mut posts_html = String::new();
    posts_html.push_str(r#"<div class="post-list">"#);
    for post in posts {
        let tags_html: String = post.tags.iter()
            .map(|t| format!(r#"<span class="tag">#{t}</span>"#))
            .collect();

        let post_filename = &post.filename;
        let link = format!("{relative_root}{post_filename}");

        let post_title = &post.title;
        let post_date = &post.date;
        posts_html.push_str(&format!(
            r#"<div class="post-entry"><a href="{link}"><span class="entry-title">{post_title} {tags_html}</span><span class="entry-date">{post_date}</span></a></div>"#
        ));
    }
    posts_html.push_str("</div>");

    let html = template(title, &format!("<h1>{title}</h1>{posts_html}"), all_tags, relative_root);
    fs::write(path, html)?;
    Ok(())
}

fn optimize_local_image(original_src: &str, content_root: &Path, public_root: &Path) -> Result<ImageResult> {
    // check if it's a local file
    if original_src.starts_with("http") {
         return Ok(ImageResult { rel_path: original_src.to_string(), width: 0, height: 0 });
    }

    let src_path = content_root.join(original_src);
    if !src_path.exists() {
         // fallback if file not found
         return Ok(ImageResult { rel_path: original_src.to_string(), width: 0, height: 0 });
    }

    // hash filename for unique destination
    let file_stem = src_path.file_stem().unwrap().to_string_lossy();
    // simple hash or just use name. let's use name + webp extension.
    let dest_filename = format!("{file_stem}.webp");
    let dest_path = public_root.join("images").join(&dest_filename);
    let webp_rel_path = format!("images/{dest_filename}"); // relative from public root

    // cache check
    if dest_path.exists() {
        // read dimensions from existing webp
        if let Ok(reader) = image::ImageReader::open(&dest_path) {
            if let Ok(dims) = reader.into_dimensions() {
                return Ok(ImageResult {
                    rel_path: webp_rel_path,
                    width: dims.0,
                    height: dims.1,
                });
            }
        }
    }

    // process image
    println!("Optimizing image: {src_path:?}");
    let img = image::open(&src_path).context("Failed to open image")?;
    
    // resize if larger than 1200px width
    let (w, _h) = img.dimensions();
    let target_width = 1200;
    
    let final_img = if w > target_width {
        img.resize(target_width, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let (new_w, new_h) = final_img.dimensions();

    // save as webp
    final_img.save_with_format(&dest_path, image::ImageFormat::WebP)
        .context("Failed to save WebP")?;

    Ok(ImageResult {
        rel_path: webp_rel_path,
        width: new_w,
        height: new_h,
    })
}

fn process_markdown(markdown: &str, title: &str, date: &str, tags: &[String], all_tags: &HashSet<String>, relative_root: &str, content_dir: &Path, public_dir: &Path) -> Result<String> {
    let parser = Parser::new(markdown);
    
    // custom event loop to intercept images
    let mut new_events = Vec::new();
    let mut in_image = false;
    let mut image_url = String::new();
    let mut image_title = String::new();
    let mut image_alt = String::new();
    let mut first_image_processed = false;

    for event in parser {
        match event {
            Event::Start(Tag::Image { link_type: _, dest_url: url, title, id: _ }) => {
                in_image = true;
                image_url = url.to_string();
                image_title = title.to_string();
                image_alt.clear();
            },
            Event::End(TagEnd::Image) => {
                in_image = false;
                
                // optimize image
                let opt_result = optimize_local_image(&image_url, content_dir, public_dir)
                    .unwrap_or(ImageResult { rel_path: image_url.clone(), width: 0, height: 0 });

                // construct relative path for html
                let final_src = if opt_result.rel_path.starts_with("http") {
                    opt_result.rel_path.clone()
                } else {
                    let rel_path = &opt_result.rel_path;
                    format!("{relative_root}{rel_path}")
                };

                let mut width_attr = String::new();
                let mut height_attr = String::new();
                
                if opt_result.width > 0 && opt_result.height > 0 {
                    let w = opt_result.width;
                    let h = opt_result.height;
                    width_attr = format!(r#"width="{w}""#);
                    height_attr = format!(r#"height="{h}""#);
                }

                let clean_title = image_title.trim();
                let mut final_title_attr = String::new();
                let mut is_dimensions = false;
                
                if !clean_title.is_empty() {
                    if let Some(x_pos) = clean_title.find('x') {
                        let (w_str, h_str) = clean_title.split_at(x_pos);
                        let h_str = &h_str[1..];
                        if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                             width_attr = format!(r#"width="{w}""#);
                             height_attr = format!(r#"height="{h}""#);
                             is_dimensions = true;
                        }
                    } else if let Ok(w) = clean_title.parse::<u32>() {
                        width_attr = format!(r#"width="{w}""#);
                        height_attr = String::new();
                        is_dimensions = true;
                    }
                }

                if !is_dimensions && !clean_title.is_empty() {
                     final_title_attr = format!(r#"title="{clean_title}""#);
                }

                let loading_attrs = if !first_image_processed {
                    first_image_processed = true;
                    r#"loading="eager" fetchpriority="high" decoding="sync""#
                } else {
                    r#"loading="lazy" decoding="async""#
                };

                let html = format!(
                    r#"<figure class="image-container">
                        <img src="{final_src}" alt="{image_alt}" {width_attr} {height_attr} {final_title_attr} {loading_attrs} />
                        <figcaption>
                            <a href="{final_src}" target="_blank" class="download-link">[ Download Full Size ]</a>
                        </figcaption>
                    </figure>"#
                );
                new_events.push(Event::Html(html.into()));
            },
            Event::Text(text) => {
                if in_image {
                    image_alt.push_str(&text);
                } else {
                    new_events.push(Event::Text(text));
                }
            },
            Event::Code(text) => {
                if in_image {
                    image_alt.push_str(&text);
                } else {
                    new_events.push(Event::Code(text));
                }
            },
            e => {
                if !in_image {
                    new_events.push(e);
                }
            }
        }
    }

    let mut html_output = String::new();
    html::push_html(&mut html_output, new_events.into_iter());
    
    let tags_str: String = tags.iter().map(|t| format!(r#"<span class="tag">#{t}</span>"#)).collect();
    let content_with_meta = format!(r#"<div class="meta"><span class="meta-item">UPLOAD: {date}</span> <span class="meta-item">{tags_str}</span></div>{html_output}"#);

    let html_page = template(title, &content_with_meta, all_tags, relative_root);
    
    Ok(html_page)
}

fn template(title: &str, content: &str, all_tags: &HashSet<String>, relative_root: &str) -> String {
    let mut sorted_tags: Vec<_> = all_tags.iter().collect();
    sorted_tags.sort();
    
    let index_link = format!("{relative_root}index.html");
    
    let mut nav_html = format!(r#"<div class="nav-section"><a href="{index_link}" class="nav-link main-link">Index</a></div>"#);
    
    if !sorted_tags.is_empty() {
        nav_html.push_str(r#"<div class="nav-section"><span class="nav-header">Filter</span>"#);
        for tag in sorted_tags {
            let tag_lower = tag.to_lowercase();
            let link = format!("{relative_root}tags/tag_{tag_lower}.html");
            nav_html.push_str(&format!(r#"<a href="{link}" class="nav-link tag-link">{tag}</a>"#));
        }
        nav_html.push_str("</div>");
    }

    format!(
r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CODE A DUCK | {title}</title>
    <link rel="stylesheet" href="{relative_root}style.css">
</head>
<body>
    <header>
        <span class="brand">[ CODE A DUCK ]</span>
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