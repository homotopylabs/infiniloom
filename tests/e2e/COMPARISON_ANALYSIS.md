# CodeLoom vs Repomix: Feature Comparison Analysis

## Executive Summary

**Critical Finding**: CodeLoom CLI currently only outputs **scan statistics**, NOT **file contents**. Repomix outputs the **complete repository content** formatted for LLMs.

This is the fundamental difference explaining the ~90x size difference in outputs.

| Metric | Repomix (httpie) | CodeLoom (httpie) |
|--------|------------------|-------------------|
| Output Size | 921 KB | 10 KB |
| Contains File Contents | ✅ Yes | ❌ No |
| Contains Directory Tree | ✅ Yes | ❌ No |
| Contains Metadata | ✅ Yes | ✅ Yes (partial) |
| Token Count | ✅ Yes | ✅ Yes |
| Language Detection | ✅ Yes | ✅ Yes |

---

## What Repomix Outputs

Repomix generates a single packed file containing:

### 1. Header Summary
```xml
<file_summary>
  <purpose>Packed representation of repository for AI systems</purpose>
  <file_format>Description of structure</file_format>
  <usage_guidelines>Instructions for AI</usage_guidelines>
  <notes>Exclusion rules applied</notes>
</file_summary>
```

### 2. Directory Structure
```xml
<directory_structure>
.github/
  workflows/
    tests.yml
httpie/
  cli/
    argparser.py
  core.py
tests/
  ...
</directory_structure>
```

### 3. Complete File Contents
```xml
<files>
<file path="httpie/core.py">
import argparse
import os
import platform
...
def raw_main(parser, main_program, args=sys.argv, env=Environment()):
    ...
</file>
<file path="httpie/client.py">
...
</file>
</files>
```

---

## What CodeLoom CLI Currently Outputs

CodeLoom's CLI (`codeloom-scan`) only outputs **scan statistics**:

```
Scanning: /path/to/repo
Model: claude

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Scan Results
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Files:        238
  Total Size:   KB
  Scan Time:    60ms

  Skipped:
    Binary:     1
    Size:       0
    Gitignore:  0
    Hidden:     6

  Languages:
    python: 133
    json: 26
    markdown: 21
    ...

  Token Estimates:
    claude: ~277467

  Files:
    CODE_OF_CONDUCT.md (KB)
    httpie/core.py (KB)
    ...
```

**Note**: File list only shows names, NOT contents.

---

## What CodeLoom Engine (Rust) CAN Output

The Rust engine has complete formatter implementations that ARE NOT exposed in the CLI:

### engine/src/output/xml.rs - Claude-optimized XML
```rust
// Full XML output with:
// - <metadata> with stats, languages
// - <repository_map> with key symbols, module graph
// - <file_index> with importance rankings
// - <files> with FULL CONTENT and line numbers
// - Claude prompt caching markers
```

### engine/src/output/markdown.rs - GPT-optimized Markdown
```rust
// Full Markdown output with:
// - Repository overview
// - File listings with contents
// - Code blocks with language hints
```

### engine/src/output/mod.rs - JSON/YAML formatters
```rust
// JSON: Generic, includes Repository struct with content
// YAML: Gemini-optimized with query placeholder
```

---

## Gap Analysis

### ❌ Missing in CodeLoom CLI (but exists in engine)

| Feature | Repomix | Engine (Rust) | CLI (Zig) |
|---------|---------|---------------|-----------|
| Output file contents | ✅ | ✅ | ❌ |
| Directory tree | ✅ | ❌ | ❌ |
| XML output format | ✅ | ✅ | ❌ |
| Markdown output format | ✅ | ✅ | ❌ |
| JSON output format | ✅ | ✅ | ❌ |
| Output to file (-o) | ✅ | ✅ | ❌ |
| RepoMap (symbol graph) | ❌ | ✅ | ❌ |
| File importance ranking | ❌ | ✅ | ❌ |
| Prompt cache markers | ❌ | ✅ | ❌ |

### ✅ CodeLoom Advantages (when fully wired)

| Feature | Repomix | CodeLoom |
|---------|---------|----------|
| Performance | ~6s | 60ms (100x faster) |
| Multi-model tokenizers | ❌ | ✅ (claude, gpt4o, gemini, llama) |
| RepoMap (code graph) | ❌ | ✅ |
| Symbol extraction | ❌ | ✅ |
| Semantic chunking | ❌ | ✅ |
| File importance scoring | ❌ | ✅ |
| Security scanning | ❌ | ✅ |
| Prompt caching optimization | ❌ | ✅ |

---

## Root Cause: CLI-Engine Disconnect

The Zig CLI (`cli.zig`) does NOT call the Rust engine formatters. It only uses:
1. `scanner/walker.zig` - File discovery
2. `tokenizer/counter.zig` - Token estimation

The Rust engine with full formatting (`engine/src/output/*.rs`) is built but:
1. Not exposed via FFI to Zig
2. Not callable from CLI
3. File contents are not read and passed to formatters

---

## Recommendations

### Priority 1: Wire Engine Formatters to CLI

```
CLI (Zig) → FFI → Engine (Rust) → XML/MD/JSON output
```

Need to:
1. Add FFI exports in `engine/src/lib.rs` for formatters
2. Call from Zig CLI with `--format xml|md|json`
3. Add `-o output.xml` file output option

### Priority 2: Read File Contents

Current: Walker collects file metadata but NOT content
Needed: Read file content and pass to Repository struct

### Priority 3: Add Directory Tree

Repomix includes `<directory_structure>` - useful for AI navigation

### Priority 4: Add Instructions Header

Repomix includes `<file_summary>` with AI instructions

---

## Quick Fix for Testing

To generate Repomix-equivalent output now, use Rust engine directly:

```rust
use codeloom_engine::{
    output::XmlFormatter,
    types::Repository,
    repomap::RepoMapGenerator,
};

let repo = Repository::scan("/path/to/repo")?;
let map = RepoMapGenerator::new(10000).generate(&repo);
let output = XmlFormatter::claude().format(&repo, &map);
```

---

## Performance Comparison (Valid Metrics)

Even though output differs, scanning performance is valid:

| Repository | Files | Repomix | CodeLoom | Speedup |
|------------|-------|---------|----------|---------|
| httpie | 238 | 6.6s | 60ms | **109x** |
| ripgrep | 199 | 2.4s | 35ms | **70x** |
| excalidraw | 813 | 5.1s | 179ms | **29x** |
| lazygit | 1,001 | 3.2s | 185ms | **17x** |
| deno | 9,269 | 21.2s | 2.2s | **10x** |
| material-ui | 31,213 | 125.7s | 8.9s | **14x** |

CodeLoom's Zig scanner is **10-109x faster** at file discovery and language detection.

---

## Conclusion

CodeLoom has all the pieces for a superior tool:
- ✅ Blazing fast Zig scanner
- ✅ Rich Rust formatters (XML, MD, JSON, YAML)
- ✅ Advanced features (RepoMap, security, chunking)

**Missing**: CLI integration to produce actual LLM-consumable output.

The current CLI is essentially a "preview mode" showing what would be processed, not the final packed output.
