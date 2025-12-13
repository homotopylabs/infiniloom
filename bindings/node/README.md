# @infiniloom/node

Node.js bindings for Infiniloom - Repository context engine for LLMs.

## Features

- **Pack repositories** into optimized LLM context with configurable compression
- **Scan repositories** for statistics and metadata
- **Count tokens** for different LLM models
- **Security scanning** to detect secrets and sensitive data
- **Fast native performance** powered by Rust and NAPI-RS
- **Cross-platform** support (macOS, Linux, Windows)

## Installation

```bash
npm install @infiniloom/node
```

## Quick Start

### Simple Packing

```javascript
const { pack } = require('@infiniloom/node');

// Pack a repository with default settings
const context = pack('./my-repo');
console.log(context);
```

### With Options

```javascript
const { pack } = require('@infiniloom/node');

const context = pack('./my-repo', {
  format: 'xml',           // Output format: 'xml', 'markdown', 'json', 'yaml', 'toon'
  model: 'claude',         // Target model: 'claude', 'gpt-4o', 'gpt-4', 'gemini', 'llama'
  compression: 'balanced', // Compression: 'none', 'minimal', 'balanced', 'aggressive', 'extreme'
  mapBudget: 2000,        // Token budget for repository map
  maxSymbols: 50,         // Maximum symbols to include in map
  skipSecurity: false     // Skip security scanning
});
```

### Repository Scanning

```javascript
const { scan } = require('@infiniloom/node');

const stats = scan('./my-repo', 'claude');
console.log(`Repository: ${stats.name}`);
console.log(`Total files: ${stats.totalFiles}`);
console.log(`Total lines: ${stats.totalLines}`);
console.log(`Total tokens: ${stats.totalTokens}`);
console.log(`Primary language: ${stats.primaryLanguage}`);
console.log(`Languages:`, stats.languages);
```

### Token Counting

```javascript
const { countTokens } = require('@infiniloom/node');

const count = countTokens('Hello, world!', 'claude');
console.log(`Tokens: ${count}`);
```

### Advanced Usage with Infiniloom Class

```javascript
const { Infiniloom } = require('@infiniloom/node');

// Create an Infiniloom instance
const loom = new Infiniloom('./my-repo', 'claude');

// Get statistics
const stats = loom.getStats();
console.log(stats);

// Generate repository map
const map = loom.generateMap(2000, 50);
console.log(map);

// Pack with options
const context = loom.pack({
  format: 'xml',
  compression: 'balanced'
});
console.log(context);

// Security scan
const findings = loom.securityScan();
if (findings.length > 0) {
  console.warn('Security issues found:');
  findings.forEach(finding => console.warn(finding));
}
```

## API Reference

### Functions

#### `pack(path: string, options?: PackOptions): string`

Pack a repository into optimized LLM context.

**Parameters:**
- `path` - Path to repository root
- `options` - Optional packing options

**Returns:** Formatted repository context as a string

#### `scan(path: string, model?: string): ScanStats`

Scan a repository and return statistics.

**Parameters:**
- `path` - Path to repository root
- `model` - Optional target model (default: "claude")

**Returns:** Repository statistics

#### `countTokens(text: string, model?: string): number`

Count tokens in text for a specific model.

**Parameters:**
- `text` - Text to tokenize
- `model` - Optional model name (default: "claude")

**Returns:** Token count

### Types

#### `PackOptions`

```typescript
interface PackOptions {
  format?: string;        // "xml", "markdown", "json", "yaml", "toon"
  model?: string;         // "claude", "gpt-4o", "gpt-4", "gemini", or "llama"
  compression?: string;   // "none", "minimal", "balanced", "aggressive", "extreme", "semantic"
  mapBudget?: number;     // Token budget for repository map
  maxSymbols?: number;    // Maximum number of symbols in map
  skipSecurity?: boolean; // Skip security scanning
}
```

#### `ScanStats`

```typescript
interface ScanStats {
  name: string;
  totalFiles: number;
  totalLines: number;
  totalTokens: number;
  primaryLanguage?: string;
  languages: LanguageStat[];
  securityFindings: number;
}
```

#### `LanguageStat`

```typescript
interface LanguageStat {
  language: string;
  files: number;
  lines: number;
  percentage: number;
}
```

### Infiniloom Class

#### `new Infiniloom(path: string, model?: string)`

Create a new Infiniloom instance.

#### `getStats(): ScanStats`

Get repository statistics.

#### `generateMap(budget?: number, maxSymbols?: number): string`

Generate a repository map.

#### `pack(options?: PackOptions): string`

Pack repository with specific options.

#### `securityScan(): string[]`

Check for security issues and return findings.

## Supported Models

- **Claude** - Anthropic's Claude models
- **GPT-4o** - OpenAI's GPT-4o
- **GPT-4** - OpenAI's GPT-4
- **Gemini** - Google's Gemini
- **Llama** - Meta's Llama

## Compression Levels

- **none** - No compression (0% reduction)
- **minimal** - Remove empty lines, trim whitespace (~15% reduction)
- **balanced** - Remove comments, normalize whitespace (~35% reduction)
- **aggressive** - Remove docstrings, keep signatures only (~60% reduction)
- **extreme** - Key symbols only (~80% reduction)
- **semantic** - AI-powered semantic compression (~90% reduction)

## Output Formats

- **xml** - XML format optimized for Claude
- **markdown** - Markdown format for GPT models
- **json** - JSON format for programmatic access
- **yaml** - YAML format optimized for Gemini
- **toon** - TOON format (~40% smaller than JSON)

## Security Scanning

Infiniloom automatically scans for sensitive data including:

- API keys
- Access tokens
- Private keys
- Passwords
- Database connection strings
- AWS credentials
- GitHub tokens

Critical security issues will prevent packing unless `skipSecurity: true` is set.

## Building from Source

```bash
# Install dependencies
npm install

# Build native addon
npm run build

# Build for release
npm run build --release
```

## Requirements

- Node.js >= 16
- Rust >= 1.75 (for building from source)

## License

MIT

## Links

- [GitHub Repository](https://github.com/homotopylabs/infiniloom)
- [Documentation](https://infiniloom.dev/docs)
- [npm Package](https://www.npmjs.com/package/@infiniloom/node)
