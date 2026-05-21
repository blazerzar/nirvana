# Nirvana build system
# Install: cargo install just
# Usage: just <recipe>

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Default: list available recipes
default:
    @just --list

# Build debug
build:
    cargo build

# Build release
build-release:
    cargo build --release

# Run debug in development environment
run *args="":
    cargo run -- {{ args }}

# Run release
run-release *args="":
    cargo run --release -- {{ args }}

# Run all tests
test:
    cargo test --workspace

# Check compilation without building
check:
    cargo check --workspace

# Lint with clippy
lint:
    cargo clippy --workspace -- -D warnings

# Format check
fmt:
    cargo fmt --check

# Clean build artifacts
clean:
    cargo clean

# ── GUI (crates/nirvana-gui) ─────────────────────────────────────────────────

# Install GUI JS dependencies (run once after clone or after package.json changes)
gui-install:
    cd crates/nirvana-gui && bun install

# Start Vite dev server only (frontend, no Rust recompile)
gui-dev:
    cd crates/nirvana-gui && bun run dev

# Start full Tauri app in dev mode (Vite + Rust backend, hot-reload)
gui-tauri-dev:
    cd crates/nirvana-gui && bun run tauri dev

# Build the full Tauri desktop app (release bundle)
gui-build:
    cd crates/nirvana-gui && bun run tauri build

# Type-check the Vue/TS frontend
gui-typecheck:
    cd crates/nirvana-gui && bun run typecheck
