# bmo build recipes

_default:
    just --list

# Run all tests
test:
    cargo test

# Check formatting and run clippy
check:
    cargo fmt --check
    cargo clippy -- -D warnings

# Release build
build:
    cargo build --release

# Remove build artifacts
clean:
    cargo clean

# Install binary to Cargo bin path
install:
    cargo install --path .

# Run cargo fmt
fmt:
    cargo fmt