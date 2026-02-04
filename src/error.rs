//! Error types with semantic recovery strategies.

use std::io;
use std::path::PathBuf;

/// All possible errors during blog generation.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    // ══════════════════════════════════════════════════════════════════════
    // RECOVERABLE: Skip this item and continue with others
    // ══════════════════════════════════════════════════════════════════════
    
    /// A single post failed to parse. Skip it, continue others.
    #[error("Parse failed for {path:?}: {message}")]
    ParseFailed {
        path: PathBuf,
        message: String,
    },

    /// Tag validation failed. Use fallback or skip tag.
    #[error("Invalid tag '{tag}': {reason}")]
    InvalidTag {
        tag: String,
        reason: &'static str,
    },

    /// Image optimization failed. Use original image instead.
    #[error("Image optimization failed for {path:?}")]
    ImageOptFailed {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },

    // ══════════════════════════════════════════════════════════════════════
    // NON-RECOVERABLE: Must abort entire build
    // ══════════════════════════════════════════════════════════════════════
    
    /// Cannot read content directory.
    #[error("Content directory not readable: {path:?}")]
    ContentNotReadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Cannot write to output directory.
    #[error("Output directory not writable: {path:?}")]
    OutputNotWritable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// No valid posts found to build.
    #[error("No valid posts found in {path:?}")]
    NoValidPosts {
        path: PathBuf,
    },

    // ══════════════════════════════════════════════════════════════════════
    // INTERNAL: Should never happen (indicates bug)
    // ══════════════════════════════════════════════════════════════════════
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl BuildError {
    /// Returns true if we can skip this item and continue building.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::ParseFailed { .. } 
            | Self::InvalidTag { .. } 
            | Self::ImageOptFailed { .. }
        )
    }

    /// Returns true if this indicates a bug in the generator.
    pub fn is_internal(&self) -> bool {
        matches!(self, Self::Internal(_))
    }
}

/// Result of a build that may have partial failures.
#[derive(Debug)]
pub struct BuildResult {
    pub successes: usize,
    pub failures: Vec<BuildError>,
}

impl BuildResult {
    pub fn new() -> Self {
        Self {
            successes: 0,
            failures: Vec::new(),
        }
    }

    pub fn record_success(&mut self) {
        self.successes += 1;
    }

    pub fn record_failure(&mut self, error: BuildError) {
        self.failures.push(error);
    }

    /// Returns Err if no posts succeeded or if any non-recoverable error occurred.
    pub fn finalize(self) -> Result<BuildSummary, BuildError> {
        // Check for non-recoverable errors
        for err in &self.failures {
            if !err.is_recoverable() {
                // Return the first non-recoverable error
                return Err(self.failures.into_iter()
                    .find(|e| !e.is_recoverable())
                    .unwrap());
            }
        }

        if self.successes == 0 && !self.failures.is_empty() {
            return Err(BuildError::NoValidPosts {
                path: PathBuf::from("content"),
            });
        }

        Ok(BuildSummary {
            posts_built: self.successes,
            posts_skipped: self.failures.len(),
            warnings: self.failures,
        })
    }
}

impl Default for BuildResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a successful (possibly partial) build.
#[derive(Debug)]
pub struct BuildSummary {
    pub posts_built: usize,
    pub posts_skipped: usize,
    pub warnings: Vec<BuildError>,
}

impl BuildSummary {
    pub fn print_report(&self) {
        println!("✓ Built {} posts", self.posts_built);
        if self.posts_skipped > 0 {
            eprintln!("⚠ Skipped {} posts:", self.posts_skipped);
            for warn in &self.warnings {
                eprintln!("  - {}", warn);
            }
        }
    }
}
