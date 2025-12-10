# Tree-sitter Parser Module

A comprehensive code parsing module for the Infiniloom engine that extracts symbols from source files across multiple programming languages.

## Features

- **Multi-language Support**: Python, JavaScript, TypeScript, Rust, Go, Java
- **Symbol Extraction**: Functions, classes, methods, structs, enums, interfaces, traits
- **Metadata Capture**:
  - Symbol names and types
  - Function/method signatures
  - Docstrings and comments
  - Line numbers (start/end)
  - Parent relationships (for methods)
- **Import Detection**: Automatically identifies import/use statements
- **Zero Configuration**: Works out of the box with sensible defaults

## Installation

The parser module is already included in the Infiniloom engine. The following dependencies are added to `Cargo.toml`:

```toml
tree-sitter = "0.20"
tree-sitter-python = "0.20"
tree-sitter-javascript = "0.20"
tree-sitter-typescript = "0.20"
tree-sitter-rust = "0.20"
tree-sitter-go = "0.20"
tree-sitter-java = "0.20"
```

## Usage

### Basic Parsing

```rust
use infiniloom_engine::parser::{Parser, Language};

// Create a parser instance
let mut parser = Parser::new();

// Parse Python code
let source_code = r#"
def hello(name: str) -> str:
    """Greet someone by name."""
    return f"Hello, {name}!"
"#;

let symbols = parser.parse(source_code, Language::Python)?;

// Access extracted symbols
for symbol in symbols {
    println!("{}: {} (lines {}-{})",
        symbol.kind.name(),
        symbol.name,
        symbol.start_line,
        symbol.end_line
    );
}
```

### Language Detection

```rust
use infiniloom_engine::parser::Language;

// Detect language from file extension
let lang = Language::from_extension("py");
assert_eq!(lang, Some(Language::Python));

let lang = Language::from_extension("rs");
assert_eq!(lang, Some(Language::Rust));
```

### Accessing Symbol Metadata

```rust
let symbols = parser.parse(source_code, Language::Python)?;

for symbol in symbols {
    // Basic info
    println!("Name: {}", symbol.name);
    println!("Kind: {}", symbol.kind.name());
    println!("Lines: {}-{}", symbol.start_line, symbol.end_line);

    // Optional metadata
    if let Some(signature) = &symbol.signature {
        println!("Signature: {}", signature);
    }

    if let Some(docstring) = &symbol.docstring {
        println!("Documentation: {}", docstring);
    }

    if let Some(parent) = &symbol.parent {
        println!("Parent: {}", parent);
    }
}
```

## Symbol Types

The parser extracts the following symbol types (defined in `SymbolKind`):

| Symbol Kind | Description | Languages |
|-------------|-------------|-----------|
| `Function` | Standalone function | All |
| `Method` | Class/struct method | All |
| `Class` | Class definition | Python, JS, TS, Java |
| `Interface` | Interface definition | TypeScript, Go, Java |
| `Struct` | Struct definition | Rust, Go |
| `Enum` | Enum definition | Rust, TypeScript, Java |
| `Trait` | Trait definition | Rust |
| `Import` | Import/use statement | All |

## Language-Specific Features

### Python

- Extracts functions, classes, and methods
- Captures docstrings (triple-quoted strings)
- Detects `import` and `from ... import` statements
- Preserves method signatures with type hints

```python
def calculate(x: int, y: int) -> int:
    """Calculate sum of two numbers."""
    return x + y

class Calculator:
    def add(self, a, b):
        return a + b
```

### Rust

- Extracts functions, structs, enums, and traits
- Captures doc comments (`///`)
- Detects `use` declarations
- Identifies methods in `impl` blocks

```rust
/// A simple calculator
pub struct Calculator {
    value: i32,
}

impl Calculator {
    /// Create a new calculator
    pub fn new() -> Self {
        Self { value: 0 }
    }
}
```

### JavaScript/TypeScript

- Extracts functions, arrow functions, classes, methods
- Captures JSDoc comments (`/** ... */`)
- TypeScript: interfaces and enums
- Detects ES6 import statements

```javascript
/**
 * Add two numbers
 * @param {number} a
 * @param {number} b
 */
function add(a, b) {
    return a + b;
}

class Counter {
    increment() { }
}
```

### Go

- Extracts functions, methods, structs, interfaces
- Captures comment documentation
- Detects import declarations
- Identifies receiver methods

```go
// User represents a user
type User struct {
    Name string
}

// Greet returns a greeting
func (u *User) Greet() string {
    return "Hello"
}
```

### Java

- Extracts classes, interfaces, enums, methods
- Captures JavaDoc comments (`/** ... */`)
- Detects import statements
- Preserves method visibility modifiers

```java
/**
 * A calculator class
 */
public class Calculator {
    /**
     * Add two numbers
     */
    public int add(int a, int b) {
        return a + b;
    }
}
```

## Architecture

### Parser Structure

```rust
pub struct Parser {
    parsers: HashMap<Language, TSParser>,
    queries: HashMap<Language, Query>,
}
```

The `Parser` maintains:
- A tree-sitter parser instance for each language
- Pre-compiled queries for efficient symbol extraction

### Query System

Each language has custom tree-sitter queries optimized for symbol extraction:

```rust
// Example: Python query
(function_definition
  name: (identifier) @name) @function

(class_definition
  name: (identifier) @name) @class
```

Queries identify:
- Symbol types (function, class, method, etc.)
- Symbol names
- Symbol boundaries (start/end lines)

### Symbol Extraction Pipeline

1. **Parse**: Source code → AST (Abstract Syntax Tree)
2. **Query**: Run language-specific queries on AST
3. **Extract**: Convert query matches to `Symbol` objects
4. **Enhance**: Add signatures, docstrings, parent relationships
5. **Import Detection**: Traverse AST for import statements

## Performance

- **Fast**: Tree-sitter is a high-performance parser (C library)
- **Incremental**: Supports incremental parsing (not yet exposed)
- **Memory Efficient**: Streams over AST without full tree in memory
- **Cached Queries**: Queries compiled once per language

Typical performance:
- Small files (<1000 lines): <1ms
- Medium files (1000-5000 lines): 1-10ms
- Large files (>5000 lines): 10-50ms

## Error Handling

The parser returns `Result<Vec<Symbol>, ParserError>` with the following error types:

- `UnsupportedLanguage`: Language not supported
- `ParseError`: Failed to parse source code
- `QueryError`: Query compilation failed
- `InvalidUtf8`: Source contains invalid UTF-8

```rust
match parser.parse(source, language) {
    Ok(symbols) => {
        // Process symbols
    }
    Err(ParserError::UnsupportedLanguage(lang)) => {
        eprintln!("Language not supported: {}", lang);
    }
    Err(e) => {
        eprintln!("Parse error: {}", e);
    }
}
```

## Testing

Run the comprehensive test suite:

```bash
cargo test parser::tests
```

Tests cover:
- Language detection
- Symbol extraction for all languages
- Metadata capture (signatures, docstrings)
- Import detection
- Edge cases

Run the interactive demo:

```bash
cargo run --example parser_demo
```

## Integration with Infiniloom

The parser integrates seamlessly with the existing Infiniloom types:

```rust
use infiniloom_engine::{RepoFile, Parser, Language};

let mut file = RepoFile::new("/path/to/file.py", "file.py");
let content = std::fs::read_to_string(&file.path)?;

// Detect language
if let Some(lang) = Language::from_extension(file.extension().unwrap()) {
    let mut parser = Parser::new();
    file.symbols = parser.parse(&content, lang)?;
}

// Now file.symbols contains all extracted symbols
for symbol in &file.symbols {
    println!("{}: {}", symbol.kind.name(), symbol.name);
}
```

## Future Enhancements

Potential improvements:

1. **More Languages**: C, C++, C#, PHP, Ruby, Swift, Kotlin
2. **Incremental Parsing**: Reparse only changed regions
3. **Reference Resolution**: Track symbol references and call graphs
4. **Type Information**: Extract and track type definitions
5. **Scope Analysis**: Identify local vs global symbols
6. **Semantic Queries**: "Find all functions that return Promise"
7. **Custom Queries**: Allow users to define custom extraction rules

## Contributing

To add support for a new language:

1. Add the tree-sitter crate to `Cargo.toml`:
   ```toml
   tree-sitter-newlang = "0.20"
   ```

2. Add language variant to `Language` enum:
   ```rust
   pub enum Language {
       // ...
       NewLang,
   }
   ```

3. Implement parser initialization:
   ```rust
   fn init_newlang_parser() -> Result<TSParser, ParserError> {
       let mut parser = TSParser::new();
       parser.set_language(tree_sitter_newlang::language())?;
       Ok(parser)
   }
   ```

4. Create tree-sitter query:
   ```rust
   fn newlang_query() -> Result<Query, ParserError> {
       let query_string = r#"
           (function_definition
             name: (identifier) @name) @function
       "#;
       Query::new(tree_sitter_newlang::language(), query_string)
   }
   ```

5. Add to `Parser::new()` initialization
6. Add tests in `parser::tests`
7. Update documentation

## Resources

- [Tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Tree-sitter Playground](https://tree-sitter.github.io/tree-sitter/playground)
- [Infiniloom Documentation](https://github.com/homotopylabs/infiniloom)

## License

MIT License - see LICENSE file for details.
