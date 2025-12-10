# CodeLoom WASM Demo

Interactive demonstrations of CodeLoom WASM capabilities.

## Contents

- **index.html** - Interactive web demo with live token counting and context generation
- **node-example.js** - Comprehensive Node.js example showing all features

## Running the Web Demo

### Option 1: Python HTTP Server (Recommended)

```bash
# Make sure you've built the WASM module first
cd ..
./build.sh

# Start server
cd demo
python3 -m http.server 8080

# Open in browser
# http://localhost:8080
```

### Option 2: Node.js http-server

```bash
npm install -g http-server
cd demo
http-server -p 8080
```

### Option 3: PHP Built-in Server

```bash
cd demo
php -S localhost:8080
```

## Web Demo Features

The interactive web demo (`index.html`) includes:

1. **Live Token Counting**
   - Real-time token counts for 5 LLM models
   - Updates as you type (debounced)
   - Shows Claude, GPT-4o, GPT-4, Gemini, and Llama counts

2. **Code Examples**
   - Pre-built examples for Rust, Python, JavaScript, TypeScript
   - Quick-load buttons for rapid testing

3. **Language Detection**
   - Automatic language detection from filename
   - Supports 20+ programming languages

4. **Context Generation**
   - Generate context in 4 formats:
     - Claude (XML) - Optimized for Anthropic Claude
     - GPT (Markdown) - Optimized for OpenAI models
     - Gemini (YAML) - Optimized for Google Gemini
     - Plain Text - Universal format

5. **Compression Levels**
   - None - Original code
   - Minimal - Remove trailing whitespace, excessive blank lines (10-20%)
   - Balanced - Remove comments, normalize whitespace (30-40%)
   - Aggressive - Signatures only, remove docstrings (50-60%)

6. **Copy to Clipboard**
   - One-click copy of generated context
   - Perfect for pasting into LLM interfaces

## Running the Node.js Example

```bash
# Build for Node.js target
cd ..
wasm-pack build --target nodejs --out-dir pkg-node

# Run example
cd demo
node node-example.js
```

### Node.js Example Output

The Node.js example demonstrates:

1. **Token Counting** - Count tokens for all models
2. **Language Detection** - Detect languages from filenames
3. **Claude XML Context** - Generate Claude-optimized XML
4. **GPT Markdown Context** - Generate GPT-optimized Markdown
5. **Repository Statistics** - Calculate repo-wide stats
6. **Compression Comparison** - Compare all compression levels

Example output:
```
🧬 CodeLoom WASM - Node.js Example

=== Token Counting ===
Token counts for all models:
  Claude:   47
  GPT-4o:   45
  GPT-4:    47
  Gemini:   47
  Llama:    54

=== Language Detection ===
  main.rs      -> rust
  app.py       -> python
  index.js     -> javascript
  test.go      -> go
  Main.java    -> java

=== Claude XML Context ===
<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <context>
    <file path="src/main.rs" language="rust">
...
```

## Browser Compatibility

The web demo works in:

- Chrome 87+
- Firefox 78+
- Safari 15+
- Edge 87+

Requirements:
- ES Modules support
- WebAssembly support
- Modern JavaScript (async/await)

## Performance Notes

### Token Counting
- ~1ms for small files (<1KB)
- ~5ms for medium files (10KB)
- ~50ms for large files (100KB)

### Context Generation
- ~5ms for single file
- ~50ms for 10 files
- ~200ms for 100 files

### Memory Usage
- WASM module: ~150KB (gzipped)
- Runtime memory: ~2-5MB typical
- Scales linearly with content size

## Customization

### Modify Examples

Edit the `examples` object in `index.html`:

```javascript
const examples = {
    rust: `your rust code here`,
    python: `your python code here`,
    // Add more languages...
};
```

### Change Styling

Modify the `<style>` section in `index.html`. The demo uses:
- CSS Grid for responsive layout
- Gradient backgrounds
- Custom stat cards
- Syntax highlighting ready (add a library if needed)

### Add Features

The WASM module exposes these functions in JavaScript:

```javascript
// Token counting
count_tokens(text, model)
count_tokens_all(text)

// File processing
detect_language(filename)
process_file(filename, content)

// Compression
compress(content, level, language)

// Context generation
generate_context(files, format, compression)

// Statistics
calculate_stats(files)

// Info
version()
```

## Troubleshooting

### "Failed to load WASM module"

1. Ensure you built the web target:
   ```bash
   wasm-pack build --target web --out-dir pkg-web
   ```

2. Serve from an HTTP server (not `file://`):
   ```bash
   python3 -m http.server 8080
   ```

3. Check browser console for specific errors

### CORS Errors

If loading from a different origin, ensure CORS headers are set:

```javascript
// Server-side
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST
```

### Performance Issues

For large files:

1. Use Web Workers:
   ```javascript
   const worker = new Worker('codeloom-worker.js');
   worker.postMessage({ code, action: 'count_tokens' });
   ```

2. Debounce updates:
   ```javascript
   const debounced = debounce(updateTokens, 500);
   input.addEventListener('input', debounced);
   ```

3. Process in chunks:
   ```javascript
   const chunks = splitIntoChunks(largeFile, 10000);
   for (const chunk of chunks) {
       await processChunk(chunk);
   }
   ```

## Advanced Usage

### Integrating with Monaco Editor

```html
<script src="https://cdn.jsdelivr.net/npm/monaco-editor@latest/min/vs/loader.js"></script>
<script>
require.config({ paths: { vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@latest/min/vs' }});
require(['vs/editor/editor.main'], function() {
    const editor = monaco.editor.create(document.getElementById('editor'), {
        value: 'function hello() {}',
        language: 'javascript'
    });

    editor.onDidChangeModelContent(() => {
        const code = editor.getValue();
        const tokens = count_tokens_all(code);
        updateDisplay(tokens);
    });
});
</script>
```

### Web Worker Integration

Create `codeloom-worker.js`:

```javascript
importScripts('../pkg-web/codeloom_wasm.js');

let wasm;

self.onmessage = async (e) => {
    if (!wasm) {
        wasm = await wasm_bindgen('../pkg-web/codeloom_wasm_bg.wasm');
    }

    const { action, data } = e.data;

    switch (action) {
        case 'count_tokens':
            const tokens = wasm.count_tokens_all(data.code);
            self.postMessage({ tokens });
            break;

        case 'generate_context':
            const context = wasm.generate_context(
                data.files,
                data.format,
                data.compression
            );
            self.postMessage({ context });
            break;
    }
};
```

Use in main thread:

```javascript
const worker = new Worker('codeloom-worker.js');

worker.postMessage({
    action: 'count_tokens',
    data: { code: editor.getValue() }
});

worker.onmessage = (e) => {
    console.log('Tokens:', e.data.tokens);
};
```

## Contributing

Want to improve the demos?

1. Fork the repository
2. Make your changes
3. Test thoroughly in multiple browsers
4. Submit a pull request

## License

MIT License - see [LICENSE](../../../LICENSE) for details
