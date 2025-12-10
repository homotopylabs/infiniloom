<div align="center">

# 🧵 Infiniloom

**The fastest and most feature-rich way to pack repository context for LLMs**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)

[Installation](#installation) • [Quick Start](#quick-start) • [Features](#features) • [Unique Features](#unique-features) • [Documentation](#documentation)

</div>

---

## What is Infiniloom?

Infiniloom transforms your codebase into optimized context for Large Language Models. It extracts code, symbols, and structure from repositories and outputs them in formats specifically optimized for Claude, GPT-4, Gemini, and other LLMs.

```bash
# Pack your repo for Claude in under a second
infiniloom pack . --format xml --output context.xml
```

**Why Infiniloom?**

- **Blazing Fast**: High-performance Rust + Zig hybrid architecture
- **Smart**: AST-based symbol extraction with PageRank importance ranking
- **Optimized**: Model-specific output formats (XML for Claude, Markdown for GPT)
- **Secure**: Automatic detection and redaction of secrets and API keys
- **Flexible**: Python, Node.js, and WebAssembly bindings included

---

## Installation

### From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/homotopylabs/infiniloom.git
cd infiniloom

# Build release binary
cargo build --release

# Binary is at ./target/release/infiniloom
# Optionally, copy to your PATH:
cp target/release/infiniloom /usr/local/bin/
```

### Prerequisites

| Tool | Version | Installation |
|------|---------|-------------|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Zig | 0.13+ | `brew install zig` or [ziglang.org](https://ziglang.org/download/) |

### Package Managers (Coming Soon)

```bash
# Cargo (Rust)
cargo install infiniloom

# npm (Node.js)
npm install -g infiniloom

# pip (Python)
pip install infiniloom

# Homebrew (macOS/Linux)
brew tap homotopylabs/tap
brew install infiniloom
```

---

## Quick Start

### Basic Commands

```bash
# Pack repository into XML (optimized for Claude)
infiniloom pack /path/to/repo --format xml

# Scan repository and show statistics
infiniloom scan /path/to/repo

# Generate repository map with key symbols
infiniloom map /path/to/repo --budget 2000

# Show repository information
infiniloom info /path/to/repo
```

### Output Formats

```bash
# XML format — optimized for Claude (with prompt caching hints)
infiniloom pack . --format xml --model claude

# Markdown format — optimized for GPT-4/GPT-4o
infiniloom pack . --format markdown --model gpt-4o

# JSON format — for programmatic use
infiniloom pack . --format json

# YAML format — optimized for Gemini
infiniloom pack . --format yaml --model gemini
```

### Working with Git

```bash
# Include recent commits in output
infiniloom pack . --include-logs --logs-count 10

# Include uncommitted changes
infiniloom pack . --include-diffs

# Pack a remote GitHub repository
infiniloom pack github:facebook/react
infiniloom pack https://github.com/tokio-rs/tokio.git
```

### File Selection

```bash
# Include only specific file types
infiniloom pack . --include "*.rs" --include "*.py"

# Exclude directories
infiniloom pack . --exclude "tests/*" --exclude "docs/*"

# Set token budget
infiniloom pack . --budget 50000

# Use compression
infiniloom pack . --compression aggressive
```

### Copy to Clipboard (macOS)

```bash
# Pack and copy directly to clipboard for pasting into Claude/ChatGPT
infiniloom pack . --format xml | pbcopy
```

---

## Features

### Model-Specific Optimization

| Model | Format | Optimizations |
|-------|--------|---------------|
| **Claude** | XML | Prompt caching hints, CDATA sections, structured tags |
| **GPT-4/4o** | Markdown | Tables, code fences, hierarchical headers |
| **Gemini** | YAML | Query at end, hierarchical structure |
| **JSON** | JSON | Full metadata, programmatic access |

### AST-Based Symbol Extraction

Infiniloom uses [Tree-sitter](https://tree-sitter.github.io/) to parse source code and extract symbols:

| Language | Symbols Extracted |
|----------|-------------------|
| Python | Functions, Classes, Methods, Decorators |
| JavaScript/TypeScript | Functions, Classes, Interfaces, Types |
| Rust | Functions, Structs, Enums, Traits, Impl blocks |
| Go | Functions, Methods, Structs, Interfaces |
| Java | Classes, Interfaces, Methods, Enums |
| C/C++ | Functions, Classes, Structs |

### PageRank Symbol Ranking

Important symbols are ranked using PageRank algorithm based on:
- Reference count (how often a symbol is used)
- Import centrality (position in dependency graph)
- File importance (entry points, main modules)

### Security Scanning

Automatically detects and redacts:
- API keys and tokens
- Passwords and secrets
- Private keys (RSA, SSH)
- Database connection strings
- Cloud credentials (AWS, GCP, Azure)

### Compression Levels

| Level | Token Reduction | What's Removed |
|-------|-----------------|----------------|
| `none` | 0% | Nothing |
| `minimal` | 10-20% | Empty lines, trailing whitespace |
| `balanced` | 30-40% | Comments, redundant whitespace |
| `aggressive` | 50-60% | Docstrings, inline comments |
| `extreme` | 70-80% | Everything except signatures |

---

## Language Bindings

### Python

```bash
cd bindings/python
pip install maturin
maturin develop
```

```python
import infiniloom

# Pack a repository
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")

# Get repository statistics
stats = infiniloom.scan("/path/to/repo")
print(f"Files: {stats['total_files']}, Tokens: {stats['total_tokens']}")

# Count tokens
tokens = infiniloom.count_tokens("def hello(): pass", model="claude")
```

### Node.js

```bash
cd bindings/node
npm install
npm run build
```

```javascript
const { pack, scan, Infiniloom } = require('@infiniloom/node');

// Pack a repository
const context = pack('./my-repo', { format: 'xml', model: 'claude' });

// Get statistics
const stats = scan('./my-repo');
console.log(`Files: ${stats.total_files}`);
```

### WebAssembly

```bash
cd bindings/wasm
wasm-pack build --target web
```

```javascript
import init, { pack, scan } from '@infiniloom/wasm';

await init();
const context = pack('/repo', 'xml', 'claude', 'balanced');
```

---

## Performance

Infiniloom is designed for speed and efficiency, significantly outperforming existing solutions through its Rust + Zig hybrid architecture. Typical processing times for medium-sized repositories (100-500 files) are under 100ms.

---

## Unique Features

Infiniloom offers capabilities not found in other repository packing tools:

### Repository Map Generation

Generate a concise map of your codebase showing the most important symbols, ranked by importance:

```bash
infiniloom map /path/to/repo --budget 2000
```

The map uses PageRank algorithm to identify key entry points, heavily-used functions, and central abstractions — giving LLMs a bird's-eye view of your architecture.

### Intelligent Token Budgeting

Set a token budget and Infiniloom will intelligently select the most relevant files:

```bash
infiniloom pack . --budget 50000
```

Files are prioritized based on:
- Symbol importance (PageRank scores)
- File centrality in the dependency graph
- Recent modification time
- Configuration file detection (package.json, Cargo.toml, etc.)

### Multi-Model Token Counting

Accurate token counts for different LLM tokenizers:

```bash
infiniloom scan . --model claude    # Claude tokenizer (cl100k_base)
infiniloom scan . --model gpt-4o    # GPT-4 tokenizer
infiniloom scan . --model gemini    # Gemini tokenizer
```

### Secret Detection & Redaction

Automatically scans for and redacts sensitive information before output:

- API keys (OpenAI, AWS, Stripe, etc.)
- Private keys (RSA, SSH, PGP)
- Database connection strings
- Environment variables with secrets
- Cloud credentials

```bash
infiniloom pack . --redact-secrets  # Enabled by default
infiniloom pack . --no-redact       # Disable if needed
```

### Native Language Bindings

Use Infiniloom directly in your applications — no shell commands needed:

- **Python**: PyO3-based native extension
- **Node.js**: NAPI-RS bindings with TypeScript types
- **WebAssembly**: Run in browsers and edge environments

---

## Configuration

### `.infiniloomignore`

Create a `.infiniloomignore` file to exclude files (in addition to `.gitignore`):

```gitignore
# Build artifacts
target/
dist/
build/

# Dependencies
node_modules/
vendor/

# Large files
*.bin
*.dat
data/

# Generated
*.generated.*
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_MODEL` | Default tokenizer model | `claude` |
| `INFINILOOM_FORMAT` | Default output format | `xml` |
| `INFINILOOM_COMPRESSION` | Default compression | `balanced` |
| `INFINILOOM_BUDGET` | Default token budget | `100000` |

### Configuration File

Run `infiniloom init` to create a `.infiniloom.toml` config file:

```toml
[output]
format = "xml"
model = "claude"
compression = "balanced"

[budget]
max_tokens = 100000
map_budget = 2000

[include]
patterns = ["*.rs", "*.py", "*.ts", "*.go"]

[exclude]
patterns = ["tests/*", "docs/*", "*.test.*"]
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/INFINILOOM_DESIGN.md) | System design and architecture |
| [Output Formats](docs/INFINILOOM_OUTPUT_FORMATS.md) | Detailed format specifications |
| [Release Plan](docs/RELEASE_PLAN.md) | Publishing and distribution |

---

## Development

### Project Structure

```
infiniloom/
├── cli/                 # Rust CLI application
├── engine/              # Core Rust engine
│   ├── src/
│   │   ├── parser.rs    # Tree-sitter AST parsing
│   │   ├── repomap/     # PageRank symbol ranking
│   │   ├── output/      # Format generators
│   │   └── security.rs  # Secret detection
├── core/                # Zig performance core
│   └── src/
│       ├── tokenizer/   # Fast token counting
│       └── compressor/  # Code compression
├── bindings/
│   ├── python/          # PyO3 bindings
│   ├── node/            # NAPI-RS bindings
│   └── wasm/            # WebAssembly bindings
└── docs/                # Documentation
```

### Building

```bash
# Build everything
cargo build --release

# Build with Zig core (maximum performance)
cargo build --release --features zig-core

# Run tests
cargo test

# Run clippy
cargo clippy

# Run benchmarks
cargo bench
```

### Running Tests

```bash
# All tests
cargo test --all

# Specific crate
cargo test -p infiniloom-engine

# With output
cargo test -- --nocapture
```

---

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing`)
3. Make your changes
4. Run tests (`cargo test`) and lints (`cargo clippy`)
5. Commit with clear messages
6. Push and open a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- [Tree-sitter](https://tree-sitter.github.io/) for fast, reliable parsing
- [tiktoken-rs](https://github.com/zurawiki/tiktoken-rs) for token counting
- [Aider](https://github.com/paul-gauthier/aider) for the repo-map concept

### Alternatives & Inspiration

These projects inspired Infiniloom and are great alternatives:

- [repomix](https://github.com/yamadashy/repomix) — Node.js-based repository packer
- [gitingest](https://github.com/cyclotruc/gitingest) — Python-based repository ingestion tool

---

<div align="center">

**[⬆ Back to Top](#-infiniloom)**

Made with 🧵 by [Homotopy Labs](https://github.com/homotopylabs)

</div>
