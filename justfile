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
