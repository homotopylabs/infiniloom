# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Infiniloom** is a high-performance repository context generator for Large Language Models. It transforms codebases into optimized formats for Claude, GPT-4, Gemini, and other LLMs. Built in pure Rust for maximum performance and portability.

Key capabilities:
- AST-based symbol extraction using Tree-sitter (30+ languages)
- PageRank-based symbol importance ranking
- Model-specific output formats (XML for Claude, Markdown for GPT, YAML for Gemini)
- Automatic secret detection and redaction
- Accurate token counting via tiktoken-rs for OpenAI models
- Native language bindings (Python, Node.js, WebAssembly)

## Build Commands

```bash
# Build release binary
cargo build --release
# Binary at ./target/release/infiniloom

# Run tests
cargo test --workspace

# Run tests with output
cargo test -- --nocapture

# Run specific crate tests
cargo test -p infiniloom-engine

# Clippy linting (strict)
cargo clippy --workspace --all-targets --all-features

# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run benchmarks
cargo bench --workspace

# Generate documentation
cargo doc --workspace --all-features --no-deps

# Code coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace --all-features --html --output-dir target/coverage
```

### Makefile Shortcuts

```bash
make build          # Debug build
make build-release  # Release build
make test           # Run all tests
make lint           # Run strict clippy
make fmt            # Format all code
make coverage       # Generate HTML coverage report
make ci             # Full CI pipeline (format, lint, test, coverage)
make pre-commit     # Quick pre-commit checks
```

## CLI Usage

```bash
# Pack repository into XML (Claude-optimized)
infiniloom pack /path/to/repo --format xml

# Scan repository and show statistics
infiniloom scan /path/to/repo

# Generate repository map with key symbols
infiniloom map /path/to/repo --budget 2000

# Show version and configuration info
infiniloom info

# Initialize configuration file
infiniloom init
```

## Code Architecture

### Workspace Structure

```
infiniloom/
├── cli/                    # CLI application (clap-based)
│   └── src/
│       ├── main.rs         # Command handling, argument parsing
│       └── scanner.rs      # Repository scanning with parallel processing
├── engine/                 # Core Rust engine library
│   └── src/
│       ├── lib.rs          # Public API exports
│       ├── types.rs        # Core types: Repository, RepoFile, Symbol
│       ├── parser.rs       # Tree-sitter AST parsing (30+ languages)
│       ├── repomap/        # PageRank symbol ranking
│       │   ├── mod.rs      # RepoMapGenerator
│       │   └── graph.rs    # SymbolGraph, PageRank computation
│       ├── output/         # Format generators
│       │   ├── xml.rs      # Claude-optimized XML
│       │   ├── markdown.rs # GPT-optimized Markdown
│       │   └── toon.rs     # Token-efficient TOON format
│       ├── ranking.rs      # File importance ranking
│       ├── security.rs     # Secret detection/redaction
│       ├── tokenizer.rs    # Multi-model token counting (tiktoken-rs)
│       ├── chunking/       # Semantic code chunking
│       ├── config.rs       # Configuration loading (YAML/TOML/JSON)
│       ├── git.rs          # Git operations (log, status, diff)
│       ├── remote.rs       # Remote repository cloning
│       ├── dependencies.rs # Dependency graph resolution
│       └── mmap_scanner.rs # Memory-mapped file scanning
└── bindings/               # Language bindings
    ├── python/             # PyO3 bindings (maturin)
    ├── node/               # NAPI-RS bindings
    └── wasm/               # WebAssembly bindings
```

### Core Types (`engine/src/types.rs`)

- **`Repository`**: Root container with name, path, files, and metadata
- **`RepoFile`**: Single file with path, language, token counts, symbols, importance score
- **`Symbol`**: Extracted code symbol (function, class, etc.) with kind, signature, line numbers
- **`TokenCounts`**: Token counts for multiple models (Claude, GPT-4o, GPT-4, Gemini, Llama)
- **`TokenizerModel`**: Enum for supported LLM tokenizers
- **`CompressionLevel`**: None, Minimal, Balanced, Aggressive, Extreme

### Data Flow

1. **Scanning** (`cli/scanner.rs`): Walk directory with `ignore` crate, filter by gitignore, detect languages
2. **Parsing** (`parser.rs`): Tree-sitter AST extraction for symbols (thread-local parsers for parallelism)
3. **Ranking** (`ranking.rs`, `repomap/`): PageRank-based importance scoring
4. **Formatting** (`output/`): Model-specific output generation
5. **Security** (`security.rs`): Secret detection before output

### Key Patterns

**Parallel File Processing with Thread-Local Parsers**:
```rust
// cli/scanner.rs - Lock-free parallel parsing
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

files.into_par_iter()
    .filter_map(|file| {
        let content = fs::read_to_string(&file.path).ok()?;
        let symbols = THREAD_PARSER.with(|p| p.borrow_mut().parse(&content, lang));
        Some(RepoFile { content, symbols, ... })
    })
    .collect()
```

**PageRank Ranking** (`repomap/graph.rs`):
- Builds symbol graph from imports/references
- Computes PageRank with damping factor 0.85
- Top symbols returned with importance scores

**Accurate Token Counting** (`tokenizer.rs`):
```rust
// Uses tiktoken-rs for exact OpenAI token counts
let tokenizer = Tokenizer::new();
let gpt4_tokens = tokenizer.count(content, TokenModel::Gpt4);   // Exact via tiktoken
let claude_tokens = tokenizer.count(content, TokenModel::Claude); // Estimation
```

**Output Formatting**:
```rust
// OutputFormatter chooses format based on target model
let formatter = OutputFormatter::by_format(OutputFormat::Xml);
let output = formatter.format(&repo, &map);
```

## Feature Flags

```toml
# engine/Cargo.toml features
default = []
async = ["tokio", "async-trait"]     # Async operations
embeddings = ["candle-core", "candle-transformers"]  # Local embeddings
watch = ["notify"]                   # File watching
git = ["gix"]                        # Git operations
full = ["async", "embeddings", "watch", "git"]
```

## Testing

```bash
# Unit tests
cargo test --workspace

# Specific test
cargo test test_generate_repomap

# Integration tests with verbose output
cargo test --workspace -- --nocapture

# Property-based tests (using proptest)
cargo test proptest
```

Test files are in `engine/src/*/tests` modules and `tests/` directories.

## Linting Configuration

The project uses strict clippy lints defined in `Cargo.toml`:
- `correctness` and `perf` are **deny** (errors)
- `suspicious`, `complexity`, `style` are **warn**
- Print macros (`print_stdout`, `print_stderr`) are warned except in CLI

## Language Bindings Development

### Python (PyO3 + Maturin)
```bash
cd bindings/python
pip install maturin
maturin develop  # Development build
maturin build --release  # Release wheel
```

### Node.js (NAPI-RS)
```bash
cd bindings/node
npm install
npm run build
```

### WebAssembly
```bash
cd bindings/wasm
wasm-pack build --target web
```

## Configuration Files

- **`.infiniloom.yaml`** / **`.infiniloom.toml`**: Project configuration
- **`.infiniloomignore`**: Additional ignore patterns (like .gitignore)

Example `.infiniloom.yaml`:
```yaml
output:
  format: xml
  model: claude
  compression: balanced
budget:
  max_tokens: 100000
exclude:
  - "tests/*"
  - "*.test.*"
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_MODEL` | Default tokenizer model | `claude` |
| `INFINILOOM_FORMAT` | Default output format | `xml` |
| `INFINILOOM_COMPRESSION` | Default compression | `balanced` |
| `INFINILOOM_BUDGET` | Default token budget | `100000` |

## CI/CD

GitHub Actions workflow (`.github/workflows/ci.yml`) runs:
1. Format check (`cargo fmt --check`)
2. Clippy linting
3. Build (Ubuntu + macOS)
4. Tests
5. Python/Node.js binding builds
6. Security scan (Trivy)
7. Code coverage (Codecov)

## Performance Architecture

The project is optimized for high performance through careful Rust design patterns.

### Key Performance Features

#### 1. Thread-Local Parsers (`cli/scanner.rs`)
Each Rayon worker thread has its own Tree-sitter parser instance, eliminating mutex contention:
```rust
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}
```

#### 2. Parallel File Processing
Uses Rayon's parallel iterators for concurrent file reading and parsing:
```rust
file_infos
    .into_par_iter()
    .filter_map(process_file_with_content)
    .collect()
```

#### 3. Gitignore-Respecting Walker
Uses the `ignore` crate for fast, gitignore-aware directory traversal:
```rust
WalkBuilder::new(path)
    .hidden(!include_hidden)
    .git_ignore(true)
    .git_global(true)
    .build()
```

#### 4. Accurate Token Counting
Uses `tiktoken-rs` for exact BPE token counts for OpenAI models:
- GPT-4, GPT-4o: Exact tiktoken encoding
- Claude, Gemini, Llama: Calibrated estimation (~95% accuracy)

#### 5. Memory-Mapped I/O (`mmap_scanner.rs`)
Optional mmap-based scanning for large files using `memmap2` crate.

### Performance Tips

1. **Skip symbols for speed**: Use `--skip-symbols` flag for 80x speedup on large repos
2. **Parallel by default**: Rayon auto-scales to available CPU cores
3. **Binary detection**: First 8KB checked, binary files automatically skipped
4. **Gitignore caching**: Patterns compiled once per directory tree
