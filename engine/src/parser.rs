//! Tree-sitter based code parser for extracting symbols from source files
//!
//! This module provides a unified interface for parsing source code across
//! multiple programming languages and extracting symbols (functions, classes,
//! methods, structs, enums, etc.) with their metadata.
//!
//! # Supported Languages
//!
//! - Python
//! - JavaScript
//! - TypeScript
//! - Rust
//! - Go
//! - Java
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::parser::{Parser, Language};
//!
//! let parser = Parser::new();
//! let source_code = std::fs::read_to_string("example.py")?;
//! let symbols = parser.parse(&source_code, Language::Python)?;
//!
//! for symbol in symbols {
//!     println!("{}: {} (lines {}-{})",
//!         symbol.kind.name(),
//!         symbol.name,
//!         symbol.start_line,
//!         symbol.end_line
//!     );
//! }
//! ```

use crate::types::{Symbol, SymbolKind};
use std::collections::HashMap;
use thiserror::Error;
use tree_sitter::{Node, Parser as TSParser, Query, QueryCursor, Tree};

/// Parser errors
#[derive(Debug, Error)]
pub enum ParserError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Invalid UTF-8 in source code")]
    InvalidUtf8,
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Rust,
    Go,
    Java,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "py" | "pyw" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            _ => None,
        }
    }

    /// Get language name as string
    pub fn name(&self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Java => "java",
        }
    }
}

/// Main parser struct for extracting code symbols
/// Uses lazy initialization - parsers are only created when first needed
pub struct Parser {
    parsers: HashMap<Language, TSParser>,
    queries: HashMap<Language, Query>,
}

impl Parser {
    /// Create a new parser instance with lazy initialization
    /// Parsers and queries are created on-demand when parse() is called
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            queries: HashMap::new(),
        }
    }

    /// Ensure parser and query are initialized for a language
    fn ensure_initialized(&mut self, language: Language) -> Result<(), ParserError> {
        use std::collections::hash_map::Entry;
        if let Entry::Vacant(parser_entry) = self.parsers.entry(language) {
            let (parser, query) = match language {
                Language::Python => (Self::init_python_parser()?, Self::python_query()?),
                Language::JavaScript => (Self::init_javascript_parser()?, Self::javascript_query()?),
                Language::TypeScript => (Self::init_typescript_parser()?, Self::typescript_query()?),
                Language::Rust => (Self::init_rust_parser()?, Self::rust_query()?),
                Language::Go => (Self::init_go_parser()?, Self::go_query()?),
                Language::Java => (Self::init_java_parser()?, Self::java_query()?),
            };
            parser_entry.insert(parser);
            self.queries.insert(language, query);
        }
        Ok(())
    }

    /// Parse source code and extract symbols
    pub fn parse(
        &mut self,
        source_code: &str,
        language: Language,
    ) -> Result<Vec<Symbol>, ParserError> {
        // Lazy initialization - only init parser for this language
        self.ensure_initialized(language)?;

        let parser = self
            .parsers
            .get_mut(&language)
            .ok_or_else(|| ParserError::UnsupportedLanguage(language.name().to_owned()))?;

        let tree = parser
            .parse(source_code, None)
            .ok_or_else(|| ParserError::ParseError("Failed to parse source code".to_owned()))?;

        let query = self
            .queries
            .get(&language)
            .ok_or_else(|| ParserError::QueryError("No query available".to_owned()))?;

        self.extract_symbols(&tree, source_code, query, language)
    }

    /// Extract symbols from the parse tree using tree-sitter queries
    fn extract_symbols(
        &self,
        tree: &Tree,
        source_code: &str,
        query: &Query,
        language: Language,
    ) -> Result<Vec<Symbol>, ParserError> {
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        let matches = cursor.matches(query, root_node, source_code.as_bytes());

        for m in matches {
            if let Some(symbol) = self.process_match(&m, source_code, query, language) {
                symbols.push(symbol);
            }
        }

        // Also extract imports using a simpler approach
        symbols.extend(self.extract_imports(root_node, source_code, language)?);

        Ok(symbols)
    }

    /// Process a single query match and create a symbol
    fn process_match(
        &self,
        m: &tree_sitter::QueryMatch<'_, '_>,
        source_code: &str,
        query: &Query,
        language: Language,
    ) -> Option<Symbol> {
        let captures = &m.captures;
        let capture_names: Vec<_> = query.capture_names().iter().map(|s| s.as_str()).collect();

        // Find name and kind captures
        let name_node = captures
            .iter()
            .find(|c| {
                capture_names
                    .get(c.index as usize)
                    .map(|n| *n == "name")
                    .unwrap_or(false)
            })?
            .node;

        let kind_capture = captures.iter().find(|c| {
            capture_names
                .get(c.index as usize)
                .map(|n| {
                    [
                        "function",
                        "class",
                        "method",
                        "struct",
                        "enum",
                        "interface",
                        "trait",
                    ]
                    .contains(n)
                })
                .unwrap_or(false)
        })?;

        let kind_name = capture_names.get(kind_capture.index as usize)?;
        let symbol_kind = self.map_symbol_kind(kind_name);

        let name = name_node.utf8_text(source_code.as_bytes()).ok()?;

        // Find the definition node (usually the largest capture)
        let def_node = captures
            .iter()
            .max_by_key(|c| c.node.byte_range().len())
            .map(|c| c.node)
            .unwrap_or(name_node);

        let start_line = def_node.start_position().row as u32 + 1;
        let end_line = def_node.end_position().row as u32 + 1;

        // Extract signature if available
        let signature = self.extract_signature(def_node, source_code, language);

        // Extract docstring if available
        let docstring = self.extract_docstring(def_node, source_code, language);

        // Extract parent if this is a method
        let parent = if symbol_kind == SymbolKind::Method {
            self.extract_parent(def_node, source_code)
        } else {
            None
        };

        let mut symbol = Symbol::new(name, symbol_kind);
        symbol.start_line = start_line;
        symbol.end_line = end_line;
        symbol.signature = signature;
        symbol.docstring = docstring;
        symbol.parent = parent;

        Some(symbol)
    }

    /// Map query capture name to SymbolKind
    fn map_symbol_kind(&self, capture_name: &str) -> SymbolKind {
        match capture_name {
            "function" => SymbolKind::Function,
            "class" => SymbolKind::Class,
            "method" => SymbolKind::Method,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "interface" => SymbolKind::Interface,
            "trait" => SymbolKind::Trait,
            _ => SymbolKind::Function,
        }
    }

    /// Extract function/method signature
    fn extract_signature(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> Option<String> {
        // Find the signature node based on language
        let sig_node = match language {
            Language::Python => {
                // For Python, find function_definition and get first line
                if node.kind() == "function_definition" {
                    // Get the line from 'def' to ':'
                    let start = node.start_byte();
                    let mut end = start;
                    for byte in &source_code.as_bytes()[start..] {
                        end += 1;
                        if *byte == b':' {
                            break;
                        }
                        if *byte == b'\n' {
                            break;
                        }
                    }
                    return Some(
                        source_code[start..end]
                            .trim().to_owned()
                            .replace('\n', " "),
                    );
                }
                None
            }
            Language::JavaScript | Language::TypeScript => {
                // For JS/TS, try to find the function declaration
                if node.kind().contains("function") || node.kind().contains("method") {
                    // Get first line up to opening brace
                    let start = node.start_byte();
                    let mut end = start;
                    let mut brace_count = 0;
                    for byte in &source_code.as_bytes()[start..] {
                        if *byte == b'{' {
                            brace_count += 1;
                            if brace_count == 1 {
                                break;
                            }
                        }
                        end += 1;
                    }
                    return Some(
                        source_code[start..end]
                            .trim().to_owned()
                            .replace('\n', " "),
                    );
                }
                None
            }
            Language::Rust => {
                // For Rust, get the function signature
                if node.kind() == "function_item" {
                    // Get everything before the body
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "block" {
                            let start = node.start_byte();
                            let end = child.start_byte();
                            return Some(
                                source_code[start..end]
                                    .trim().to_owned()
                                    .replace('\n', " "),
                            );
                        }
                    }
                }
                None
            }
            Language::Go => {
                // For Go, get function declaration
                if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "block" {
                            let start = node.start_byte();
                            let end = child.start_byte();
                            return Some(
                                source_code[start..end]
                                    .trim().to_owned()
                                    .replace('\n', " "),
                            );
                        }
                    }
                }
                None
            }
            Language::Java => {
                // For Java, get method declaration
                if node.kind() == "method_declaration" {
                    for child in node.children(&mut node.walk()) {
                        if child.kind() == "block" {
                            let start = node.start_byte();
                            let end = child.start_byte();
                            return Some(
                                source_code[start..end]
                                    .trim().to_owned()
                                    .replace('\n', " "),
                            );
                        }
                    }
                }
                None
            }
        };

        sig_node.or_else(|| {
            // Fallback: get first line of the node
            let start = node.start_byte();
            let end = std::cmp::min(start + 200, source_code.len());
            let text = &source_code[start..end];
            text.lines().next().map(|s| s.trim().to_owned())
        })
    }

    /// Extract docstring/documentation comment
    fn extract_docstring(
        &self,
        node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> Option<String> {
        match language {
            Language::Python => {
                // Look for string literal as first child of function body
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" {
                        // Look for first expression_statement with string
                        for stmt in child.children(&mut child.walk()) {
                            if stmt.kind() == "expression_statement" {
                                for expr in stmt.children(&mut stmt.walk()) {
                                    if expr.kind() == "string" {
                                        if let Ok(text) = expr.utf8_text(source_code.as_bytes()) {
                                            // Remove quotes and clean up
                                            return Some(
                                                text.trim_matches(|c| c == '"' || c == '\'')
                                                    .trim().to_owned(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                None
            }
            Language::JavaScript | Language::TypeScript => {
                // Look for JSDoc comment immediately before the node
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            if text.starts_with("/**") {
                                return Some(self.clean_jsdoc(text));
                            }
                        }
                    }
                }
                None
            }
            Language::Rust => {
                // Look for doc comment (///) above the node
                let start_byte = node.start_byte();
                let lines_before: Vec<_> = source_code[..start_byte]
                    .lines()
                    .rev()
                    .take_while(|line| line.trim().starts_with("///") || line.trim().is_empty())
                    .collect();

                if !lines_before.is_empty() {
                    let doc: Vec<String> = lines_before
                        .into_iter()
                        .rev()
                        .filter_map(|line| {
                            let trimmed = line.trim();
                            trimmed.strip_prefix("///").map(|s| s.trim().to_owned())
                        })
                        .collect();

                    if !doc.is_empty() {
                        return Some(doc.join(" "));
                    }
                }
                None
            }
            Language::Go => {
                // Look for comment immediately before
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            return Some(text.trim_start_matches("//").trim().to_owned());
                        }
                    }
                }
                None
            }
            Language::Java => {
                // Look for JavaDoc comment
                if let Some(prev_sibling) = node.prev_sibling() {
                    if prev_sibling.kind() == "block_comment" {
                        if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                            if text.starts_with("/**") {
                                return Some(self.clean_javadoc(text));
                            }
                        }
                    }
                }
                None
            }
        }
    }

    /// Extract parent class/struct name for methods
    fn extract_parent(&self, node: Node<'_>, source_code: &str) -> Option<String> {
        let mut current = node.parent()?;

        while let Some(parent) = current.parent() {
            if ["class_definition", "class_declaration", "struct_item", "impl_item"]
                .contains(&parent.kind())
            {
                // Find the name node
                for child in parent.children(&mut parent.walk()) {
                    if child.kind() == "identifier" || child.kind() == "type_identifier" {
                        if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                            return Some(name.to_owned());
                        }
                    }
                }
            }
            current = parent;
        }

        None
    }

    /// Extract import statements (only top-level for performance)
    fn extract_imports(
        &self,
        root_node: Node<'_>,
        source_code: &str,
        language: Language,
    ) -> Result<Vec<Symbol>, ParserError> {
        let mut imports = Vec::new();

        let import_kinds = match language {
            Language::Python => vec!["import_statement", "import_from_statement"],
            Language::JavaScript | Language::TypeScript => vec!["import_statement"],
            Language::Rust => vec!["use_declaration"],
            Language::Go => vec!["import_declaration"],
            Language::Java => vec!["import_declaration"],
        };

        // Only check top-level children (imports are typically at module level)
        // This is much faster than recursive traversal for large files
        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if import_kinds.contains(&child.kind()) {
                if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                    let mut symbol = Symbol::new(text.trim(), SymbolKind::Import);
                    symbol.start_line = child.start_position().row as u32 + 1;
                    symbol.end_line = child.end_position().row as u32 + 1;
                    imports.push(symbol);
                }
            }
            // For Go, imports are inside import_declaration > import_spec_list
            if child.kind() == "import_declaration" {
                let mut inner = child.walk();
                for spec in child.children(&mut inner) {
                    if spec.kind() == "import_spec" || spec.kind() == "import_spec_list" {
                        if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                            let mut symbol = Symbol::new(text.trim(), SymbolKind::Import);
                            symbol.start_line = child.start_position().row as u32 + 1;
                            symbol.end_line = child.end_position().row as u32 + 1;
                            imports.push(symbol);
                            break; // Only add once per import_declaration
                        }
                    }
                }
            }
        }

        Ok(imports)
    }

    /// Clean JSDoc comment
    fn clean_jsdoc(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                line.trim()
                    .trim_start_matches("/**")
                    .trim_start_matches("/*")
                    .trim_start_matches('*')
                    .trim_end_matches("*/")
                    .trim()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Clean JavaDoc comment
    fn clean_javadoc(&self, text: &str) -> String {
        self.clean_jsdoc(text) // Same format as JSDoc
    }

    // Language-specific parser initializers

    fn init_python_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(tree_sitter_python::language())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_javascript_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(tree_sitter_javascript::language())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_typescript_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(tree_sitter_typescript::language_typescript())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_rust_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(tree_sitter_rust::language())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_go_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(tree_sitter_go::language())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    fn init_java_parser() -> Result<TSParser, ParserError> {
        let mut parser = TSParser::new();
        parser
            .set_language(tree_sitter_java::language())
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    // Language-specific queries

    fn python_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_definition
              name: (identifier) @name) @function

            (class_definition
              name: (identifier) @name) @class

            (class_definition
              body: (block
                (function_definition
                  name: (identifier) @name))) @method
        "#;

        Query::new(tree_sitter_python::language(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn javascript_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (identifier) @name) @function

            (class_declaration
              name: (identifier) @name) @class

            (method_definition
              name: (property_identifier) @name) @method

            (arrow_function) @function

            (function_expression) @function
        "#;

        Query::new(tree_sitter_javascript::language(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn typescript_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (identifier) @name) @function

            (class_declaration
              name: (type_identifier) @name) @class

            (interface_declaration
              name: (type_identifier) @name) @interface

            (method_definition
              name: (property_identifier) @name) @method

            (enum_declaration
              name: (identifier) @name) @enum
        "#;

        Query::new(tree_sitter_typescript::language_typescript(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn rust_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_item
              name: (identifier) @name) @function

            (struct_item
              name: (type_identifier) @name) @struct

            (enum_item
              name: (type_identifier) @name) @enum

            (trait_item
              name: (type_identifier) @name) @trait
        "#;

        Query::new(tree_sitter_rust::language(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn go_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (function_declaration
              name: (identifier) @name) @function

            (method_declaration
              name: (field_identifier) @name) @method

            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (struct_type))) @struct

            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (interface_type))) @interface
        "#;

        Query::new(tree_sitter_go::language(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }

    fn java_query() -> Result<Query, ParserError> {
        let query_string = r#"
            (method_declaration
              name: (identifier) @name) @method

            (class_declaration
              name: (identifier) @name) @class

            (interface_declaration
              name: (identifier) @name) @interface

            (enum_declaration
              name: (identifier) @name) @enum
        "#;

        Query::new(tree_sitter_java::language(), query_string)
            .map_err(|e| ParserError::QueryError(e.to_string()))
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_parse_python() {
        let mut parser = Parser::new();
        let source = r#"
def hello_world():
    """This is a docstring"""
    print("Hello, World!")

class MyClass:
    def method(self, x):
        return x * 2
"#;

        let symbols = parser.parse(source, Language::Python).unwrap();
        assert!(!symbols.is_empty());

        // Find function
        let func = symbols
            .iter()
            .find(|s| s.name == "hello_world" && s.kind == SymbolKind::Function);
        assert!(func.is_some());

        // Find class
        let class = symbols
            .iter()
            .find(|s| s.name == "MyClass" && s.kind == SymbolKind::Class);
        assert!(class.is_some());

        // Find method
        let method = symbols
            .iter()
            .find(|s| s.name == "method" && s.kind == SymbolKind::Method);
        assert!(method.is_some());
    }

    #[test]
    fn test_parse_rust() {
        let mut parser = Parser::new();
        let source = r#"
/// A test function
fn test_function() -> i32 {
    42
}

struct MyStruct {
    field: i32,
}

enum MyEnum {
    Variant1,
    Variant2,
}
"#;

        let symbols = parser.parse(source, Language::Rust).unwrap();
        assert!(!symbols.is_empty());

        // Find function
        let func = symbols
            .iter()
            .find(|s| s.name == "test_function" && s.kind == SymbolKind::Function);
        assert!(func.is_some());

        // Find struct
        let struct_sym = symbols
            .iter()
            .find(|s| s.name == "MyStruct" && s.kind == SymbolKind::Struct);
        assert!(struct_sym.is_some());

        // Find enum
        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "MyEnum" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some());
    }

    #[test]
    fn test_parse_javascript() {
        let mut parser = Parser::new();
        let source = r#"
function testFunction() {
    return 42;
}

class TestClass {
    testMethod() {
        return "test";
    }
}

const arrowFunc = () => {
    console.log("arrow");
};
"#;

        let symbols = parser.parse(source, Language::JavaScript).unwrap();
        assert!(!symbols.is_empty());

        // Find function
        let func = symbols
            .iter()
            .find(|s| s.name == "testFunction" && s.kind == SymbolKind::Function);
        assert!(func.is_some());

        // Find class
        let class = symbols
            .iter()
            .find(|s| s.name == "TestClass" && s.kind == SymbolKind::Class);
        assert!(class.is_some());
    }

    #[test]
    fn test_parse_typescript() {
        let mut parser = Parser::new();
        let source = r#"
interface TestInterface {
    method(): void;
}

enum TestEnum {
    Value1,
    Value2
}

class TestClass implements TestInterface {
    method(): void {
        console.log("test");
    }
}
"#;

        let symbols = parser.parse(source, Language::TypeScript).unwrap();
        assert!(!symbols.is_empty());

        // Find interface
        let interface = symbols
            .iter()
            .find(|s| s.name == "TestInterface" && s.kind == SymbolKind::Interface);
        assert!(interface.is_some());

        // Find enum
        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "TestEnum" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some());
    }

    #[test]
    fn test_symbol_metadata() {
        let mut parser = Parser::new();
        let source = r#"
def test_func(x, y):
    """A test function with params"""
    return x + y
"#;

        let symbols = parser.parse(source, Language::Python).unwrap();
        let func = symbols
            .iter()
            .find(|s| s.name == "test_func")
            .expect("Function not found");

        assert!(func.start_line > 0);
        assert!(func.end_line >= func.start_line);
        assert!(func.signature.is_some());
        assert!(func.docstring.is_some());
    }
}
