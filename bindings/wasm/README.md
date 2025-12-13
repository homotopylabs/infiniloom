# Infiniloom WASM

WebAssembly bindings for [Infiniloom](https://github.com/homotopylabs/infiniloom) - Transform repositories into LLM-friendly context.

## Installation

```bash
npm install @infiniloom/wasm
```

## Usage

### Web (ES Modules)

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Infiniloom Demo</title>
</head>
<body>
    <script type="module">
        import init, { count_tokens_all, generate_context, OutputFormat, CompressionLevel } from './pkg-web/infiniloom_wasm.js';

        async function main() {
            // Initialize WASM module
            await init();

            // Count tokens
            const text = "function hello() { console.log('Hello!'); }";
            const tokens = count_tokens_all(text);
            console.log('Tokens:', {
                claude: tokens.claude,
                gpt4o: tokens.gpt4o,
                gpt4: tokens.gpt4,
                gemini: tokens.gemini,
                llama: tokens.llama
            });

            // Generate context
            const files = [
                ['index.js', 'console.log("Hello");'],
                ['main.rs', 'fn main() { println!("Hello"); }']
            ];

            const context = generate_context(
                files,
                OutputFormat.Claude,
                CompressionLevel.Balanced
            );
            console.log(context);
        }

        main();
    </script>
</body>
</html>
```

### Node.js

```javascript
const { count_tokens_all, generate_context, OutputFormat, CompressionLevel } = require('@infiniloom/wasm/pkg-node');

// Count tokens
const text = "function hello() { console.log('Hello!'); }";
const tokens = count_tokens_all(text);
console.log('Claude tokens:', tokens.claude);
console.log('GPT-4o tokens:', tokens.gpt4o);

// Generate context
const files = [
    ['index.js', 'console.log("Hello");'],
    ['main.rs', 'fn main() { println!("Hello"); }']
];

const context = generate_context(
    files,
    OutputFormat.Claude,
    CompressionLevel.Balanced
);
console.log(context);
```

### Webpack/Bundlers

```javascript
import init, { count_tokens_all, generate_context, OutputFormat, CompressionLevel } from '@infiniloom/wasm';

async function run() {
    await init();

    const tokens = count_tokens_all("your code here");
    console.log(tokens);
}

run();
```

## API

### Token Counting

#### `count_tokens(text: string, model: string): number`

Count tokens for a specific model.

```javascript
const count = count_tokens("Hello, world!", "claude");
```

Supported models: `claude`, `gpt4o`, `gpt4`, `gemini`, `llama`

#### `count_tokens_all(text: string): TokenCounts`

Count tokens for all models at once.

```javascript
const tokens = count_tokens_all("Hello, world!");
console.log(tokens.claude, tokens.gpt4o);
```

### File Processing

#### `detect_language(filename: string): string | null`

Detect programming language from filename.

```javascript
const lang = detect_language("main.rs"); // Returns "rust"
```

#### `process_file(filename: string, content: string): FileInfo`

Process a file and get comprehensive information.

```javascript
const info = process_file("app.py", "print('hello')");
console.log(info.language, info.tokens, info.size_bytes);
```

### Compression

#### `compress(content: string, level: CompressionLevel, language?: string): string`

Compress code with specified level.

```javascript
const compressed = compress(
    sourceCode,
    CompressionLevel.Balanced,
    "javascript"
);
```

Compression levels:
- `CompressionLevel.None` - No compression
- `CompressionLevel.Minimal` - Remove trailing whitespace, excessive blank lines (10-20% reduction)
- `CompressionLevel.Balanced` - Remove comments, normalize whitespace (30-40% reduction)
- `CompressionLevel.Aggressive` - Signatures only, remove docstrings (50-60% reduction)

### Context Generation

#### `generate_context(files: [string, string][], format: OutputFormat, compression: CompressionLevel): string`

Generate LLM-ready context from files.

```javascript
const files = [
    ['src/main.rs', 'fn main() {}'],
    ['src/lib.rs', 'pub mod utils;']
];

const context = generate_context(
    files,
    OutputFormat.Claude,  // Claude (XML), GPT (Markdown), Gemini (YAML), Plain
    CompressionLevel.Balanced
);
```

Output formats:
- `OutputFormat.Claude` - XML format optimized for Claude
- `OutputFormat.GPT` - Markdown format optimized for GPT models
- `OutputFormat.Gemini` - YAML-like format for Gemini
- `OutputFormat.Plain` - Plain text format

### Statistics

#### `calculate_stats(files: [string, string][]): RepoStats`

Calculate repository statistics.

```javascript
const files = [
    ['file1.js', content1],
    ['file2.py', content2]
];

const stats = calculate_stats(files);
console.log({
    files: stats.total_files,
    bytes: stats.total_bytes,
    lines: stats.total_lines,
    tokens: {
        claude: stats.tokens_claude,
        gpt4o: stats.tokens_gpt4o
    }
});
```

## Performance

Infiniloom WASM is designed for performance:

- **Fast token counting**: Optimized algorithms for quick estimation
- **Efficient compression**: Rule-based compression with minimal overhead
- **Small bundle size**: ~150KB gzipped
- **No dependencies**: Pure WASM with no external dependencies

## Benchmarks

| Operation | Time (1KB file) | Time (100KB file) |
|-----------|----------------|-------------------|
| Token counting | <1ms | ~5ms |
| Language detection | <0.1ms | <0.1ms |
| Minimal compression | ~1ms | ~50ms |
| Balanced compression | ~2ms | ~100ms |
| Context generation | ~5ms | ~200ms |

## Browser Support

- Chrome/Edge 87+
- Firefox 78+
- Safari 15+
- Node.js 16+

## Building from Source

```bash
# Install wasm-pack
cargo install wasm-pack

# Build all targets
./build.sh

# Or build specific targets
npm run build:web      # For web
npm run build:node     # For Node.js
npm run build:bundler  # For bundlers
```

## Examples

See the `demo/` directory for complete examples:

- `demo/index.html` - Interactive web demo
- `demo/node-example.js` - Node.js example
- `demo/webpack-example/` - Webpack integration

## License

MIT License - see LICENSE file for details

## Contributing

Contributions welcome! See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## Links

- [Main Repository](https://github.com/homotopylabs/infiniloom)
- [Documentation](https://infiniloom.dev/docs)
- [Issues](https://github.com/homotopylabs/infiniloom/issues)
