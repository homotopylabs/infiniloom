//! FFI bridge to Zig core library
//!
//! This module provides FFI bindings to the Zig core library for high-performance
//! file scanning, token counting, and code compression.
//!
//! When the `zig-core` feature is enabled, these functions call into the Zig library.
//! When disabled, pure Rust fallbacks are used.

use std::ffi::c_char;
#[cfg(feature = "zig-core")]
use std::ffi::c_void;

#[cfg(feature = "zig-core")]
use std::ffi::{CStr, CString};

// ============================================================================
// C ABI Structures (must match exports.zig exactly)
// ============================================================================

/// Result from Zig scan operation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ScanResult {
    pub file_count: u32,
    pub total_bytes: u64,
    pub total_tokens: u64,
    pub scan_time_ms: i64,
    pub error_code: i32,
}

/// Information about a single file from Zig
#[repr(C)]
#[derive(Debug)]
pub struct FileInfo {
    pub path: *const c_char,
    pub path_len: u32,
    pub relative_path: *const c_char,
    pub relative_path_len: u32,
    pub size_bytes: u64,
    pub token_count_claude: u32,
    pub token_count_gpt4o: u32,
    pub language: *const c_char,
    pub language_len: u8,
    pub importance: f32,
}

/// Token counts for multiple models
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenCounts {
    pub claude: u32,
    pub gpt4o: u32,
    pub gpt4: u32,
    pub gemini: u32,
    pub llama: u32,
}

/// Compression configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CompressionConfig {
    /// 0=none, 1=minimal, 2=balanced, 3=aggressive, 4=extreme
    pub level: u8,
    pub remove_comments: bool,
    pub remove_empty_lines: bool,
    pub preserve_imports: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: 2, // balanced
            remove_comments: true,
            remove_empty_lines: true,
            preserve_imports: true,
        }
    }
}

// ============================================================================
// FFI Function Declarations (link to Zig library)
// ============================================================================

#[cfg(feature = "zig-core")]
#[link(name = "infiniloom-core")]
extern "C" {
    fn infiniloom_init() -> *mut c_void;
    fn infiniloom_free(ctx: *mut c_void);
    fn infiniloom_get_error(ctx: *mut c_void) -> *const c_char;

    fn infiniloom_scan(
        ctx: *mut c_void,
        path: *const c_char,
        include_hidden: bool,
        respect_gitignore: bool,
        max_file_size: u64,
    ) -> ScanResult;

    fn infiniloom_get_file_count(ctx: *mut c_void) -> u32;
    fn infiniloom_get_file(ctx: *mut c_void, index: u32, out: *mut FileInfo) -> bool;
    fn infiniloom_free_file_info(info: *mut FileInfo);

    fn infiniloom_count_tokens(
        ctx: *mut c_void,
        text: *const u8,
        text_len: usize,
        model: u8,
    ) -> u32;

    fn infiniloom_count_tokens_all(
        ctx: *mut c_void,
        text: *const u8,
        text_len: usize,
        out: *mut TokenCounts,
    );

    fn infiniloom_compress(
        ctx: *mut c_void,
        text: *const u8,
        text_len: usize,
        config: CompressionConfig,
        language: u8,
        out_buffer: *mut u8,
        buffer_size: usize,
    ) -> i64;

    fn infiniloom_version() -> *const c_char;
}

// ============================================================================
// High-Level Rust Wrapper
// ============================================================================

/// High-level wrapper for Zig core
pub struct ZigCore {
    #[cfg(feature = "zig-core")]
    ctx: *mut c_void,
    #[cfg(not(feature = "zig-core"))]
    _phantom: std::marker::PhantomData<()>,
}

// Safety: ZigCore is thread-safe as the Zig implementation uses proper synchronization
unsafe impl Send for ZigCore {}
unsafe impl Sync for ZigCore {}

impl ZigCore {
    /// Initialize Zig core
    #[cfg(feature = "zig-core")]
    pub fn new() -> Option<Self> {
        let ctx = unsafe { infiniloom_init() };
        if ctx.is_null() {
            None
        } else {
            Some(Self { ctx })
        }
    }

    /// Initialize Zig core (stub when not linked)
    #[cfg(not(feature = "zig-core"))]
    pub fn new() -> Option<Self> {
        // Return None when Zig core is not available
        None
    }

    /// Check if Zig core is available
    pub fn is_available() -> bool {
        cfg!(feature = "zig-core")
    }

    /// Get library version
    #[cfg(feature = "zig-core")]
    pub fn version() -> String {
        unsafe {
            let ptr = infiniloom_version();
            if ptr.is_null() {
                "unknown".to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn version() -> String {
        "rust-fallback".to_owned()
    }

    /// Get last error message
    #[cfg(feature = "zig-core")]
    pub fn last_error(&self) -> Option<String> {
        unsafe {
            let ptr = infiniloom_get_error(self.ctx);
            if ptr.is_null() {
                None
            } else {
                let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
        }
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn last_error(&self) -> Option<String> {
        None
    }

    /// Scan a directory using Zig's fast walker
    #[cfg(feature = "zig-core")]
    pub fn scan(
        &self,
        path: &str,
        include_hidden: bool,
        respect_gitignore: bool,
        max_file_size: u64,
    ) -> Result<ScanResult, String> {
        let path_c = CString::new(path).map_err(|e| e.to_string())?;

        let result = unsafe {
            infiniloom_scan(
                self.ctx,
                path_c.as_ptr(),
                include_hidden,
                respect_gitignore,
                max_file_size,
            )
        };

        if result.error_code != 0 {
            return Err(self
                .last_error()
                .unwrap_or_else(|| format!("Scan failed with code: {}", result.error_code)));
        }

        Ok(result)
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn scan(
        &self,
        _path: &str,
        _include_hidden: bool,
        _respect_gitignore: bool,
        _max_file_size: u64,
    ) -> Result<ScanResult, String> {
        Err("Zig core not linked - use Rust scanner instead".to_owned())
    }

    /// Get number of files from last scan
    #[cfg(feature = "zig-core")]
    pub fn file_count(&self) -> u32 {
        unsafe { infiniloom_get_file_count(self.ctx) }
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn file_count(&self) -> u32 {
        0
    }

    /// Get file info at index
    #[cfg(feature = "zig-core")]
    pub fn get_file(&self, index: u32) -> Option<ScannedFile> {
        let mut info = FileInfo {
            path: std::ptr::null(),
            path_len: 0,
            relative_path: std::ptr::null(),
            relative_path_len: 0,
            size_bytes: 0,
            token_count_claude: 0,
            token_count_gpt4o: 0,
            language: std::ptr::null(),
            language_len: 0,
            importance: 0.5,
        };

        let success = unsafe { infiniloom_get_file(self.ctx, index, &mut info) };
        if !success {
            return None;
        }

        // Convert C strings to Rust strings
        let path = unsafe {
            if info.path.is_null() {
                String::new()
            } else {
                std::slice::from_raw_parts(info.path as *const u8, info.path_len as usize)
                    .iter()
                    .map(|&c| c as char)
                    .collect()
            }
        };

        let relative_path = unsafe {
            if info.relative_path.is_null() {
                String::new()
            } else {
                std::slice::from_raw_parts(
                    info.relative_path as *const u8,
                    info.relative_path_len as usize,
                )
                .iter()
                .map(|&c| c as char)
                .collect()
            }
        };

        let language = unsafe {
            if info.language.is_null() || info.language_len == 0 {
                None
            } else {
                Some(
                    std::slice::from_raw_parts(
                        info.language as *const u8,
                        info.language_len as usize,
                    )
                    .iter()
                    .map(|&c| c as char)
                    .collect(),
                )
            }
        };

        // Free the Zig-allocated strings now that we've copied them to Rust Strings
        unsafe { infiniloom_free_file_info(&mut info) };

        Some(ScannedFile {
            path,
            relative_path,
            size_bytes: info.size_bytes,
            token_count_claude: info.token_count_claude,
            token_count_gpt4o: info.token_count_gpt4o,
            language,
            importance: info.importance,
        })
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn get_file(&self, _index: u32) -> Option<ScannedFile> {
        None
    }

    /// Count tokens using Zig's tokenizer
    #[cfg(feature = "zig-core")]
    pub fn count_tokens(&self, text: &str, model: TokenizerModel) -> u32 {
        unsafe { infiniloom_count_tokens(self.ctx, text.as_ptr(), text.len(), model as u8) }
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn count_tokens(&self, text: &str, model: TokenizerModel) -> u32 {
        estimate_tokens(text, model.name())
    }

    /// Count tokens for all models
    #[cfg(feature = "zig-core")]
    pub fn count_tokens_all(&self, text: &str) -> TokenCounts {
        let mut counts = TokenCounts::default();
        unsafe {
            infiniloom_count_tokens_all(self.ctx, text.as_ptr(), text.len(), &mut counts);
        }
        counts
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn count_tokens_all(&self, text: &str) -> TokenCounts {
        TokenCounts {
            claude: estimate_tokens(text, "claude"),
            gpt4o: estimate_tokens(text, "gpt4o"),
            gpt4: estimate_tokens(text, "gpt4"),
            gemini: estimate_tokens(text, "gemini"),
            llama: estimate_tokens(text, "llama"),
        }
    }

    /// Compress code using Zig's compressor
    #[cfg(feature = "zig-core")]
    pub fn compress(
        &self,
        text: &str,
        config: CompressionConfig,
        language: LanguageId,
    ) -> Result<String, String> {
        // Allocate output buffer (compressed should be smaller)
        let max_size = text.len() + 1024;
        let mut buffer = vec![0u8; max_size];

        let result_len = unsafe {
            infiniloom_compress(
                self.ctx,
                text.as_ptr(),
                text.len(),
                config,
                language as u8,
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };

        if result_len < 0 {
            return Err(match result_len {
                -1 => "Invalid context".to_string(),
                -2 => self
                    .last_error()
                    .unwrap_or_else(|| "Compression failed".to_string()),
                -3 => "Buffer too small".to_string(),
                _ => format!("Unknown error: {}", result_len),
            });
        }

        buffer.truncate(result_len as usize);
        String::from_utf8(buffer).map_err(|e| e.to_string())
    }

    #[cfg(not(feature = "zig-core"))]
    pub fn compress(
        &self,
        text: &str,
        _config: CompressionConfig,
        _language: LanguageId,
    ) -> Result<String, String> {
        // Basic fallback: just return the text as-is
        Ok(text.to_owned())
    }
}

#[cfg(feature = "zig-core")]
impl Drop for ZigCore {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { infiniloom_free(self.ctx) };
        }
    }
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Scanned file information
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub token_count_claude: u32,
    pub token_count_gpt4o: u32,
    pub language: Option<String>,
    pub importance: f32,
}

/// Tokenizer model IDs (must match Zig enum order)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerModel {
    Claude = 0,
    Gpt4o = 1,
    Gpt4 = 2,
    Gemini = 3,
    Llama = 4,
}

impl TokenizerModel {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gpt4o => "gpt4o",
            Self::Gpt4 => "gpt4",
            Self::Gemini => "gemini",
            Self::Llama => "llama",
        }
    }
}

/// Language IDs for compression (must match Zig enum order)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Python = 0,
    JavaScript = 1,
    TypeScript = 2,
    Rust = 3,
    Go = 4,
    Java = 5,
    Unknown = 255,
}

impl From<&str> for LanguageId {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "python" => Self::Python,
            "javascript" | "js" => Self::JavaScript,
            "typescript" | "ts" => Self::TypeScript,
            "rust" | "rs" => Self::Rust,
            "go" => Self::Go,
            "java" => Self::Java,
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// Pure Rust Fallbacks
// ============================================================================

/// Estimate tokens without Zig core (fast approximation)
pub fn estimate_tokens(text: &str, model: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }

    let chars_per_token = match model {
        "claude" => 3.5,
        "gpt4o" | "gpt-4o" => 4.0,
        "gpt4" | "gpt-4" => 3.7,
        "gemini" => 3.8,
        "llama" => 3.5,
        "codellama" => 3.2,
        _ => 4.0,
    };

    // Basic estimation with adjustments
    let base = text.len() as f32 / chars_per_token;

    // Adjust for whitespace (often merged with adjacent tokens)
    let whitespace = text.chars().filter(|c| c.is_whitespace()).count() as f32;
    let adjusted = base - (whitespace * 0.3);

    // Adjust for code-specific characters
    let special = text
        .chars()
        .filter(|c| matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ':' | ',' | '.'))
        .count() as f32;
    let final_estimate = adjusted + (special * 0.3);

    final_estimate.ceil().max(1.0) as u32
}

/// Detect if content is binary
pub fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8192);
    let sample = &data[..check_len];

    // Check for null bytes
    if sample.contains(&0) {
        return true;
    }

    // Check for high ratio of non-printable characters
    let non_printable = sample
        .iter()
        .filter(|&&b| b < 32 && b != b'\t' && b != b'\n' && b != b'\r')
        .count();

    non_printable * 10 > check_len
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let text = "def hello():\n    print('Hello, World!')\n";
        let tokens = estimate_tokens(text, "claude");
        assert!(tokens > 5);
        assert!(tokens < 30);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens("", "claude"), 0);
    }

    #[test]
    fn test_is_binary() {
        assert!(!is_binary(b"Hello, World!"));
        assert!(is_binary(&[0, 1, 2, 3]));
        assert!(is_binary(b"Hello\x00World"));
    }

    #[test]
    fn test_zig_core_availability() {
        // Should be false unless compiled with zig-core feature
        let available = ZigCore::is_available();
        println!("Zig core available: {}", available);
    }

    #[test]
    fn test_language_id_from_str() {
        assert_eq!(LanguageId::from("python"), LanguageId::Python);
        assert_eq!(LanguageId::from("rust"), LanguageId::Rust);
        assert_eq!(LanguageId::from("unknown"), LanguageId::Unknown);
    }
}
