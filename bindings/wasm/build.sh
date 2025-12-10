#!/bin/bash
set -e

echo "🔨 Building CodeLoom WASM..."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check for required tools
if ! command -v wasm-pack &> /dev/null; then
    echo -e "${RED}❌ wasm-pack not found. Installing...${NC}"
    cargo install wasm-pack
fi

# Build for web (default)
echo -e "${BLUE}📦 Building for web target...${NC}"
wasm-pack build --target web --out-dir pkg-web

# Build for Node.js
echo -e "${BLUE}📦 Building for Node.js target...${NC}"
wasm-pack build --target nodejs --out-dir pkg-node

# Build for bundler (webpack, etc)
echo -e "${BLUE}📦 Building for bundler target...${NC}"
wasm-pack build --target bundler --out-dir pkg

echo -e "${GREEN}✅ Build complete!${NC}"
echo ""
echo "Output directories:"
echo "  - pkg/         (for webpack/bundlers)"
echo "  - pkg-web/     (for direct web use)"
echo "  - pkg-node/    (for Node.js)"
echo ""
echo "To test the demo:"
echo "  cd demo"
echo "  python3 -m http.server 8080"
echo "  # Open http://localhost:8080"
