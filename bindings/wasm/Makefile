# CodeLoom WASM - Makefile
# Convenience commands for building and testing

.PHONY: all build build-web build-node build-bundler clean test demo install help

all: build

# Build all targets
build:
	@echo "Building all WASM targets..."
	./build.sh

# Build individual targets
build-web:
	@echo "Building for web..."
	wasm-pack build --target web --out-dir pkg-web

build-node:
	@echo "Building for Node.js..."
	wasm-pack build --target nodejs --out-dir pkg-node

build-bundler:
	@echo "Building for bundlers..."
	wasm-pack build --target bundler --out-dir pkg

# Build with optimizations
build-release:
	@echo "Building optimized release..."
	wasm-pack build --release --target web --out-dir pkg-web
	wasm-pack build --release --target nodejs --out-dir pkg-node
	wasm-pack build --release --target bundler --out-dir pkg

# Run tests
test:
	@echo "Running Rust tests..."
	cargo test

test-wasm:
	@echo "Running WASM tests..."
	wasm-pack test --headless --firefox

test-node: build-node
	@echo "Running Node.js example..."
	node demo/node-example.js

# Start demo server
demo: build-web
	@echo "Starting demo server on http://localhost:8080"
	@echo "Press Ctrl+C to stop"
	cd demo && python3 -m http.server 8080

# Install dependencies
install:
	@echo "Installing wasm-pack..."
	cargo install wasm-pack

# Format code
fmt:
	cargo fmt

# Lint code
lint:
	cargo clippy -- -D warnings

# Check formatting
check-fmt:
	cargo fmt -- --check

# Security audit
audit:
	cargo audit

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	rm -rf pkg pkg-web pkg-node target
	cargo clean

# Package for npm
package: build-release
	@echo "Packaging for npm..."
	npm pack

# Show help
help:
	@echo "CodeLoom WASM - Available commands:"
	@echo ""
	@echo "Building:"
	@echo "  make build           - Build all targets"
	@echo "  make build-web       - Build web target only"
	@echo "  make build-node      - Build Node.js target only"
	@echo "  make build-bundler   - Build bundler target only"
	@echo "  make build-release   - Build optimized release"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Run Rust unit tests"
	@echo "  make test-wasm      - Run WASM tests in browser"
	@echo "  make test-node      - Run Node.js example"
	@echo ""
	@echo "Development:"
	@echo "  make demo           - Start demo server"
	@echo "  make fmt            - Format code"
	@echo "  make lint           - Lint code"
	@echo "  make check-fmt      - Check formatting"
	@echo "  make audit          - Security audit"
	@echo ""
	@echo "Other:"
	@echo "  make install        - Install dependencies"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make package        - Package for npm"
	@echo "  make help           - Show this help"
