//! Python bindings for Infiniloom
//!
//! This module provides Python bindings using PyO3 for the Infiniloom engine.

use pyo3::prelude::*;
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Import from infiniloom-engine
use infiniloom_engine::{
    CompressionLevel, OutputFormat, OutputFormatter, RepoMap, RepoMapGenerator,
    Repository, TokenizerModel, SecurityScanner,
};

mod scanner;
use scanner::{scan_repository, ScanConfig};

/// Python exception for Infiniloom errors
pyo3::create_exception!(infiniloom, InfiniloomError, pyo3::exceptions::PyException);

/// Convert Rust errors to Python exceptions
fn to_py_err(err: impl std::fmt::Display) -> PyErr {
    InfiniloomError::new_err(format!("{}", err))
}

/// Pack a repository into an LLM-optimized format
///
/// Args:
///     path: Path to the repository
///     format: Output format ("xml", "markdown", "json", "yaml")
///     model: Target LLM model ("claude", "gpt", "gemini")
///     compression: Compression level ("none", "minimal", "balanced", "aggressive", "extreme")
///     map_budget: Token budget for repository map (default: 2000)
///     max_symbols: Maximum number of symbols to include (default: 50)
///
/// Returns:
///     Formatted repository context as a string
///
/// Example:
///     >>> import infiniloom
///     >>> context = infiniloom.pack("/path/to/repo", format="xml", model="claude")
///     >>> print(context)
#[pyfunction]
#[pyo3(signature = (path, format="xml", model="claude", compression="balanced", map_budget=2000, max_symbols=50))]
fn pack(
    path: &str,
    format: &str,
    model: &str,
    compression: &str,
    map_budget: u32,
    max_symbols: usize,
) -> PyResult<String> {
    // Parse format
    let output_format = match format.to_lowercase().as_str() {
        "xml" => OutputFormat::Xml,
        "markdown" | "md" => OutputFormat::Markdown,
        "json" => OutputFormat::Json,
        "yaml" | "yml" => OutputFormat::Yaml,
        _ => return Err(PyValueError::new_err(format!("Invalid format: {}", format))),
    };

    // Parse model
    let tokenizer_model = match model.to_lowercase().as_str() {
        "claude" => TokenizerModel::Claude,
        "gpt" | "gpt-4" => TokenizerModel::Gpt4,
        "gpt-4o" | "gpt4o" => TokenizerModel::Gpt4o,
        "gemini" => TokenizerModel::Gemini,
        "llama" => TokenizerModel::Llama,
        _ => return Err(PyValueError::new_err(format!("Invalid model: {}", model))),
    };

    // Parse compression level
    let _compression_level = match compression.to_lowercase().as_str() {
        "none" => CompressionLevel::None,
        "minimal" => CompressionLevel::Minimal,
        "balanced" => CompressionLevel::Balanced,
        "aggressive" => CompressionLevel::Aggressive,
        "extreme" => CompressionLevel::Extreme,
        "semantic" => CompressionLevel::Semantic,
        _ => return Err(PyValueError::new_err(format!("Invalid compression: {}", compression))),
    };

    // Scan repository
    let path_buf = PathBuf::from(path);
    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 50 * 1024 * 1024, // 50MB
    };

    let repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    // Generate repository map
    let generator = RepoMapGenerator::new(map_budget)
        .with_max_symbols(max_symbols)
        .with_model(tokenizer_model);
    let map = generator.generate(&repo);

    // Format output
    let formatter = OutputFormatter::by_format(output_format);
    let output = formatter.format(&repo, &map);

    Ok(output)
}

/// Scan a repository and return statistics
///
/// Args:
///     path: Path to the repository
///     include_hidden: Include hidden files (default: False)
///     respect_gitignore: Respect .gitignore files (default: True)
///
/// Returns:
///     Dictionary with repository statistics
///
/// Example:
///     >>> import infiniloom
///     >>> stats = infiniloom.scan("/path/to/repo")
///     >>> print(stats["total_files"])
#[pyfunction]
#[pyo3(signature = (path, include_hidden=false, respect_gitignore=true))]
fn scan(
    py: Python,
    path: &str,
    include_hidden: bool,
    respect_gitignore: bool,
) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let config = ScanConfig {
        include_hidden,
        respect_gitignore,
        read_contents: false,
        max_file_size: 50 * 1024 * 1024,
    };

    let repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    // Convert to Python dict
    let dict = PyDict::new(py);
    dict.set_item("name", repo.name)?;
    dict.set_item("path", repo.path.to_string_lossy().to_string())?;
    dict.set_item("total_files", repo.metadata.total_files)?;
    dict.set_item("total_lines", repo.metadata.total_lines)?;

    // Token counts
    let tokens = PyDict::new(py);
    tokens.set_item("claude", repo.metadata.total_tokens.claude)?;
    tokens.set_item("gpt4o", repo.metadata.total_tokens.gpt4o)?;
    tokens.set_item("gpt4", repo.metadata.total_tokens.gpt4)?;
    tokens.set_item("gemini", repo.metadata.total_tokens.gemini)?;
    tokens.set_item("llama", repo.metadata.total_tokens.llama)?;
    dict.set_item("total_tokens", tokens)?;

    // Languages
    let languages = PyList::new(
        py,
        repo.metadata.languages.iter().map(|lang| {
            let lang_dict = PyDict::new(py);
            lang_dict.set_item("language", &lang.language).unwrap();
            lang_dict.set_item("files", lang.files).unwrap();
            lang_dict.set_item("lines", lang.lines).unwrap();
            lang_dict.set_item("percentage", lang.percentage).unwrap();
            lang_dict
        }),
    );
    dict.set_item("languages", languages)?;

    // Optional metadata
    if let Some(branch) = repo.metadata.branch {
        dict.set_item("branch", branch)?;
    }
    if let Some(commit) = repo.metadata.commit {
        dict.set_item("commit", commit)?;
    }
    if let Some(framework) = repo.metadata.framework {
        dict.set_item("framework", framework)?;
    }

    Ok(dict.into())
}

/// Count tokens in text for a specific model
///
/// Args:
///     text: Text to count tokens for
///     model: Target LLM model ("claude", "gpt", "gemini")
///
/// Returns:
///     Number of tokens
///
/// Example:
///     >>> import infiniloom
///     >>> tokens = infiniloom.count_tokens("Hello, world!", model="claude")
///     >>> print(tokens)
#[pyfunction]
#[pyo3(signature = (text, model="claude"))]
fn count_tokens(text: &str, model: &str) -> PyResult<u32> {
    // Simple estimation based on character count
    // Real implementation would use actual tokenizers
    let len = text.len() as f32;

    let tokens = match model.to_lowercase().as_str() {
        "claude" => (len / 3.5) as u32,
        "gpt" | "gpt-4" => (len / 3.7) as u32,
        "gpt-4o" | "gpt4o" => (len / 4.0) as u32,
        "gemini" => (len / 3.8) as u32,
        "llama" => (len / 3.5) as u32,
        _ => return Err(PyValueError::new_err(format!("Invalid model: {}", model))),
    };

    Ok(tokens)
}

/// Scan repository for security issues
///
/// Args:
///     path: Path to the repository
///
/// Returns:
///     List of security findings
///
/// Example:
///     >>> import infiniloom
///     >>> findings = infiniloom.scan_security("/path/to/repo")
///     >>> for finding in findings:
///     ...     print(finding["severity"], finding["message"])
#[pyfunction]
fn scan_security(py: Python, path: &str) -> PyResult<PyObject> {
    let path_buf = PathBuf::from(path);
    let config = ScanConfig {
        include_hidden: false,
        respect_gitignore: true,
        read_contents: true,
        max_file_size: 10 * 1024 * 1024, // 10MB for security scan
    };

    let repo = scan_repository(&path_buf, config).map_err(to_py_err)?;

    let scanner = SecurityScanner::new();
    let mut all_findings = Vec::new();

    // Scan each file's content
    for file in &repo.files {
        if let Some(content) = &file.content {
            let findings = scanner.scan(content, &file.relative_path);
            all_findings.extend(findings);
        }
    }

    // Convert findings to Python list
    let results = PyList::new(
        py,
        all_findings.iter().map(|finding| {
            let dict = PyDict::new(py);
            dict.set_item("file", &finding.file).unwrap();
            dict.set_item("line", finding.line).unwrap();
            dict.set_item("severity", format!("{:?}", finding.severity)).unwrap();
            dict.set_item("kind", finding.kind.name()).unwrap();
            dict.set_item("pattern", &finding.pattern).unwrap();
            dict
        }),
    );

    Ok(results.into())
}

/// Infiniloom class for object-oriented interface
///
/// Example:
///     >>> from infiniloom import Infiniloom
///     >>> loom = Infiniloom("/path/to/repo")
///     >>> stats = loom.stats()
///     >>> context = loom.pack(format="xml", model="claude")
#[pyclass]
struct Infiniloom {
    path: PathBuf,
    repo: Option<Repository>,
}

#[pymethods]
impl Infiniloom {
    /// Create a new Infiniloom instance
    ///
    /// Args:
    ///     path: Path to the repository
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Err(PyIOError::new_err(format!("Path does not exist: {}", path)));
        }

        Ok(Infiniloom {
            path: path_buf,
            repo: None,
        })
    }

    /// Scan the repository and load it into memory
    fn load(&mut self, include_hidden: bool, respect_gitignore: bool) -> PyResult<()> {
        let config = ScanConfig {
            include_hidden,
            respect_gitignore,
            read_contents: true,
            max_file_size: 50 * 1024 * 1024,
        };

        let repo = scan_repository(&self.path, config).map_err(to_py_err)?;
        self.repo = Some(repo);
        Ok(())
    }

    /// Get repository statistics
    fn stats(&mut self, py: Python) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();

        let dict = PyDict::new(py);
        dict.set_item("name", &repo.name)?;
        dict.set_item("path", repo.path.to_string_lossy().to_string())?;
        dict.set_item("total_files", repo.metadata.total_files)?;
        dict.set_item("total_lines", repo.metadata.total_lines)?;

        let tokens = PyDict::new(py);
        tokens.set_item("claude", repo.metadata.total_tokens.claude)?;
        tokens.set_item("gpt4o", repo.metadata.total_tokens.gpt4o)?;
        tokens.set_item("gpt4", repo.metadata.total_tokens.gpt4)?;
        tokens.set_item("gemini", repo.metadata.total_tokens.gemini)?;
        tokens.set_item("llama", repo.metadata.total_tokens.llama)?;
        dict.set_item("tokens", tokens)?;

        Ok(dict.into())
    }

    /// Pack the repository into an LLM-optimized format
    #[pyo3(signature = (format="xml", model="claude", compression="balanced", map_budget=2000))]
    fn pack(
        &mut self,
        format: &str,
        model: &str,
        compression: &str,
        map_budget: u32,
    ) -> PyResult<String> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();

        // Parse format
        let output_format = match format.to_lowercase().as_str() {
            "xml" => OutputFormat::Xml,
            "markdown" | "md" => OutputFormat::Markdown,
            "json" => OutputFormat::Json,
            "yaml" | "yml" => OutputFormat::Yaml,
            _ => return Err(PyValueError::new_err(format!("Invalid format: {}", format))),
        };

        // Parse model
        let tokenizer_model = match model.to_lowercase().as_str() {
            "claude" => TokenizerModel::Claude,
            "gpt" | "gpt-4" => TokenizerModel::Gpt4,
            "gpt-4o" | "gpt4o" => TokenizerModel::Gpt4o,
            "gemini" => TokenizerModel::Gemini,
            "llama" => TokenizerModel::Llama,
            _ => return Err(PyValueError::new_err(format!("Invalid model: {}", model))),
        };

        // Generate repository map
        let generator = RepoMapGenerator::new(map_budget).with_model(tokenizer_model);
        let map = generator.generate(repo);

        // Format output
        let formatter = OutputFormatter::by_format(output_format);
        let output = formatter.format(repo, &map);

        Ok(output)
    }

    /// Get the repository map
    #[pyo3(signature = (map_budget=2000, max_symbols=50))]
    fn map(&mut self, py: Python, map_budget: u32, max_symbols: usize) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();
        let generator = RepoMapGenerator::new(map_budget).with_max_symbols(max_symbols);
        let map = generator.generate(repo);

        // Convert to Python dict
        let dict = PyDict::new(py);
        dict.set_item("summary", &map.summary)?;
        dict.set_item("token_count", map.token_count)?;

        // Key symbols
        let symbols = PyList::new(
            py,
            map.key_symbols.iter().map(|sym| {
                let sym_dict = PyDict::new(py);
                sym_dict.set_item("name", &sym.name).unwrap();
                sym_dict.set_item("kind", &sym.kind).unwrap();
                sym_dict.set_item("file", &sym.file).unwrap();
                sym_dict.set_item("line", sym.line).unwrap();
                sym_dict.set_item("rank", sym.rank).unwrap();
                sym_dict.set_item("importance", sym.importance).unwrap();
                if let Some(sig) = &sym.signature {
                    sym_dict.set_item("signature", sig).unwrap();
                }
                sym_dict
            }),
        );
        dict.set_item("key_symbols", symbols)?;

        Ok(dict.into())
    }

    /// Scan for security issues
    fn scan_security(&mut self, py: Python) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();
        let scanner = SecurityScanner::new();
        let mut all_findings = Vec::new();

        // Scan each file's content
        for file in &repo.files {
            if let Some(content) = &file.content {
                let findings = scanner.scan(content, &file.relative_path);
                all_findings.extend(findings);
            }
        }

        let results = PyList::new(
            py,
            all_findings.iter().map(|finding| {
                let dict = PyDict::new(py);
                dict.set_item("file", &finding.file).unwrap();
                dict.set_item("line", finding.line).unwrap();
                dict.set_item("severity", format!("{:?}", finding.severity)).unwrap();
                dict.set_item("kind", finding.kind.name()).unwrap();
                dict.set_item("pattern", &finding.pattern).unwrap();
                dict
            }),
        );

        Ok(results.into())
    }

    /// Get list of files in the repository
    fn files(&mut self, py: Python) -> PyResult<PyObject> {
        if self.repo.is_none() {
            self.load(false, true)?;
        }

        let repo = self.repo.as_ref().unwrap();

        let files = PyList::new(
            py,
            repo.files.iter().map(|file| {
                let dict = PyDict::new(py);
                dict.set_item("path", &file.relative_path).unwrap();
                if let Some(lang) = &file.language {
                    dict.set_item("language", lang).unwrap();
                }
                dict.set_item("size_bytes", file.size_bytes).unwrap();
                dict.set_item("tokens", file.token_count.claude).unwrap();
                dict.set_item("importance", file.importance).unwrap();
                dict
            }),
        );

        Ok(files.into())
    }

    fn __repr__(&self) -> String {
        format!("Infiniloom('{}')", self.path.display())
    }

    fn __str__(&self) -> String {
        format!("Infiniloom repository at {}", self.path.display())
    }
}

/// Python module definition
#[pymodule]
fn _infiniloom(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Functions
    m.add_function(wrap_pyfunction!(pack, m)?)?;
    m.add_function(wrap_pyfunction!(scan, m)?)?;
    m.add_function(wrap_pyfunction!(count_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(scan_security, m)?)?;

    // Classes
    m.add_class::<Infiniloom>()?;

    // Exceptions
    m.add("InfiniloomError", _py.get_type::<InfiniloomError>())?;

    Ok(())
}
