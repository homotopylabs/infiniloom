//! Accurate token counting using actual BPE tokenizers
//!
//! This module provides accurate token counts using tiktoken for OpenAI models
//! and estimation-based counting for other models.

use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

/// Supported LLM models for token counting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenModel {
    /// Claude (Anthropic) - uses estimation based on their ~3.5 chars/token
    Claude,
    /// GPT-4o - uses o200k_base encoding (most efficient)
    Gpt4o,
    /// GPT-4/3.5 Turbo - uses cl100k_base encoding
    Gpt4,
    /// Gemini - estimation based on ~3.8 chars/token
    Gemini,
    /// Llama 2/3 - estimation based on ~3.5 chars/token
    Llama,
    /// CodeLlama - more granular for code (~3.2 chars/token)
    CodeLlama,
}

impl TokenModel {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4 => "gpt-4",
            Self::Gemini => "gemini",
            Self::Llama => "llama",
            Self::CodeLlama => "codellama",
        }
    }

    /// Get average characters per token (for estimation fallback)
    pub fn chars_per_token(&self) -> f32 {
        match self {
            Self::Claude => 3.5,
            Self::Gpt4o => 4.0,  // o200k is more efficient
            Self::Gpt4 => 3.7,
            Self::Gemini => 3.8,
            Self::Llama => 3.5,
            Self::CodeLlama => 3.2,
        }
    }

    /// Whether this model has an exact tokenizer available
    pub fn has_exact_tokenizer(&self) -> bool {
        matches!(self, Self::Gpt4o | Self::Gpt4)
    }
}

/// Global tokenizer instances (lazy initialized, thread-safe)
static GPT4O_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
static GPT4_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

/// Get or initialize the GPT-4o tokenizer (o200k_base)
fn get_gpt4o_tokenizer() -> &'static CoreBPE {
    GPT4O_TOKENIZER.get_or_init(|| {
        o200k_base().expect("Failed to initialize o200k_base tokenizer")
    })
}

/// Get or initialize the GPT-4 tokenizer (cl100k_base)
fn get_gpt4_tokenizer() -> &'static CoreBPE {
    GPT4_TOKENIZER.get_or_init(|| {
        cl100k_base().expect("Failed to initialize cl100k_base tokenizer")
    })
}

/// Accurate token counter with fallback to estimation
pub struct Tokenizer {
    /// Use exact tokenization when available
    use_exact: bool,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer {
    /// Create a new tokenizer with exact mode enabled
    pub fn new() -> Self {
        Self { use_exact: true }
    }

    /// Create a tokenizer that only uses estimation (faster but less accurate)
    pub fn estimation_only() -> Self {
        Self { use_exact: false }
    }

    /// Count tokens for a specific model
    pub fn count(&self, text: &str, model: TokenModel) -> u32 {
        if text.is_empty() {
            return 0;
        }

        if self.use_exact && model.has_exact_tokenizer() {
            self.count_exact(text, model)
        } else {
            self.estimate(text, model)
        }
    }

    /// Count tokens using exact BPE encoding
    fn count_exact(&self, text: &str, model: TokenModel) -> u32 {
        match model {
            TokenModel::Gpt4o => {
                let tokenizer = get_gpt4o_tokenizer();
                tokenizer.encode_ordinary(text).len() as u32
            }
            TokenModel::Gpt4 => {
                let tokenizer = get_gpt4_tokenizer();
                tokenizer.encode_ordinary(text).len() as u32
            }
            _ => self.estimate(text, model),
        }
    }

    /// Estimate tokens using character-based heuristics
    fn estimate(&self, text: &str, model: TokenModel) -> u32 {
        if text.is_empty() {
            return 0;
        }

        let chars_per_token = model.chars_per_token();
        let len = text.len() as f32;

        // Base estimation
        let mut estimate = len / chars_per_token;

        // Count whitespace (often merged with adjacent tokens)
        let whitespace_count = text.chars().filter(|c| *c == ' ' || *c == '\t').count() as f32;
        estimate -= whitespace_count * 0.3;

        // Count newlines (usually single tokens)
        let newline_count = text.chars().filter(|c| *c == '\n').count() as f32;
        estimate += newline_count * 0.5;

        // Adjust for special characters (often separate tokens)
        let special_chars = text.chars().filter(|c| {
            matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ':' | ',' | '.' |
                    '=' | '+' | '-' | '*' | '/' | '<' | '>' | '!' | '&' | '|' |
                    '@' | '#' | '$' | '%' | '^' | '~' | '`' | '"' | '\'')
        }).count() as f32;

        // Code-focused models handle special chars differently
        if matches!(model, TokenModel::CodeLlama | TokenModel::Claude) {
            estimate += special_chars * 0.3;
        }

        estimate.ceil().max(1.0) as u32
    }

    /// Count tokens for all supported models at once
    pub fn count_all(&self, text: &str) -> TokenCounts {
        TokenCounts {
            claude: self.count(text, TokenModel::Claude),
            gpt4o: self.count(text, TokenModel::Gpt4o),
            gpt4: self.count(text, TokenModel::Gpt4),
            gemini: self.count(text, TokenModel::Gemini),
            llama: self.count(text, TokenModel::Llama),
        }
    }

    /// Estimate which model will have the lowest token count
    pub fn most_efficient_model(&self, text: &str) -> (TokenModel, u32) {
        let counts = self.count_all(text);
        let models = [
            (TokenModel::Gpt4o, counts.gpt4o),
            (TokenModel::Claude, counts.claude),
            (TokenModel::Gpt4, counts.gpt4),
            (TokenModel::Gemini, counts.gemini),
            (TokenModel::Llama, counts.llama),
        ];

        models.into_iter()
            .min_by_key(|(_, count)| *count)
            .unwrap()
    }

    /// Truncate text to fit within a token budget
    pub fn truncate_to_budget<'a>(&self, text: &'a str, model: TokenModel, budget: u32) -> &'a str {
        let current = self.count(text, model);
        if current <= budget {
            return text;
        }

        // Binary search for the right truncation point
        let mut low = 0usize;
        let mut high = text.len();

        while low < high {
            let mid = (low + high).div_ceil(2);
            // Find valid UTF-8 boundary
            let mid = text.floor_char_boundary(mid);
            let count = self.count(&text[..mid], model);

            if count <= budget {
                low = mid;
            } else {
                high = mid.saturating_sub(1);
            }
        }

        // Try to truncate at word boundary
        let mut end = low;
        while end > 0 {
            let c = text.as_bytes().get(end - 1).copied().unwrap_or(0);
            if c == b' ' || c == b'\n' {
                break;
            }
            end -= 1;
        }

        if end > 0 {
            &text[..end]
        } else {
            let low = text.floor_char_boundary(low);
            &text[..low]
        }
    }

    /// Check if text exceeds a token budget
    pub fn exceeds_budget(&self, text: &str, model: TokenModel, budget: u32) -> bool {
        self.count(text, model) > budget
    }
}

/// Token counts for multiple models
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenCounts {
    pub claude: u32,
    pub gpt4o: u32,
    pub gpt4: u32,
    pub gemini: u32,
    pub llama: u32,
}

impl TokenCounts {
    /// Create zero counts
    pub fn zero() -> Self {
        Self::default()
    }

    /// Get count for a specific model
    pub fn get(&self, model: TokenModel) -> u32 {
        match model {
            TokenModel::Claude => self.claude,
            TokenModel::Gpt4o => self.gpt4o,
            TokenModel::Gpt4 => self.gpt4,
            TokenModel::Gemini => self.gemini,
            TokenModel::Llama | TokenModel::CodeLlama => self.llama,
        }
    }

    /// Sum all counts
    pub fn total(&self) -> u64 {
        self.claude as u64 + self.gpt4o as u64 + self.gpt4 as u64 +
        self.gemini as u64 + self.llama as u64
    }

    /// Add counts from another TokenCounts
    pub fn add(&mut self, other: &TokenCounts) {
        self.claude += other.claude;
        self.gpt4o += other.gpt4o;
        self.gpt4 += other.gpt4;
        self.gemini += other.gemini;
        self.llama += other.llama;
    }
}

impl std::ops::Add for TokenCounts {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            claude: self.claude + rhs.claude,
            gpt4o: self.gpt4o + rhs.gpt4o,
            gpt4: self.gpt4 + rhs.gpt4,
            gemini: self.gemini + rhs.gemini,
            llama: self.llama + rhs.llama,
        }
    }
}

impl std::iter::Sum for TokenCounts {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + x)
    }
}

/// Quick estimation without creating a Tokenizer instance
pub fn quick_estimate(text: &str, model: TokenModel) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let chars_per_token = model.chars_per_token();
    (text.len() as f32 / chars_per_token).ceil().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_gpt4o_counting() {
        let tokenizer = Tokenizer::new();
        let text = "Hello, world!";
        let count = tokenizer.count(text, TokenModel::Gpt4o);

        // o200k_base should give exact count
        assert!(count > 0);
        assert!(count < 10); // Should be around 3-4 tokens
    }

    #[test]
    fn test_exact_gpt4_counting() {
        let tokenizer = Tokenizer::new();
        let text = "def hello():\n    print('Hello, World!')\n";
        let count = tokenizer.count(text, TokenModel::Gpt4);

        // cl100k_base should give exact count
        assert!(count > 5);
        assert!(count < 30);
    }

    #[test]
    fn test_estimation_claude() {
        let tokenizer = Tokenizer::new();
        let text = "This is a test string for token estimation.";
        let count = tokenizer.count(text, TokenModel::Claude);

        // Estimation should be reasonable
        assert!(count > 5);
        assert!(count < 30);
    }

    #[test]
    fn test_count_all() {
        let tokenizer = Tokenizer::new();
        let text = "function hello() { console.log('hello'); }";
        let counts = tokenizer.count_all(text);

        assert!(counts.claude > 0);
        assert!(counts.gpt4o > 0);
        assert!(counts.gpt4 > 0);
        assert!(counts.gemini > 0);
        assert!(counts.llama > 0);
    }

    #[test]
    fn test_empty_string() {
        let tokenizer = Tokenizer::new();
        assert_eq!(tokenizer.count("", TokenModel::Claude), 0);
        assert_eq!(tokenizer.count("", TokenModel::Gpt4o), 0);
    }

    #[test]
    fn test_truncate_to_budget() {
        let tokenizer = Tokenizer::new();
        let text = "This is a fairly long string that we want to truncate to fit within a smaller token budget for testing purposes.";

        let truncated = tokenizer.truncate_to_budget(text, TokenModel::Gpt4, 10);
        let count = tokenizer.count(truncated, TokenModel::Gpt4);

        assert!(count <= 10);
        assert!(truncated.len() < text.len());
    }

    #[test]
    fn test_quick_estimate() {
        let count = quick_estimate("Hello world", TokenModel::Claude);
        assert!(count > 0);
        assert!(count < 10);
    }

    #[test]
    fn test_token_counts_add() {
        let a = TokenCounts { claude: 10, gpt4o: 8, gpt4: 9, gemini: 8, llama: 10 };
        let b = TokenCounts { claude: 5, gpt4o: 4, gpt4: 5, gemini: 4, llama: 5 };
        let sum = a + b;

        assert_eq!(sum.claude, 15);
        assert_eq!(sum.gpt4o, 12);
        assert_eq!(sum.gpt4, 14);
    }

    #[test]
    fn test_most_efficient_model() {
        let tokenizer = Tokenizer::new();
        let text = "const x = 42;";
        let (model, count) = tokenizer.most_efficient_model(text);

        // GPT-4o with o200k should usually be most efficient
        assert!(count > 0);
        println!("Most efficient: {:?} with {} tokens", model, count);
    }
}
