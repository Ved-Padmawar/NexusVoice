# NexusVoice — CI-ready task runner
# Install: cargo install just
# Usage: just <target>

default:
    @just --list

# Run all checks (lint + test)
check: lint test

# Compile without producing an artifact
build:
    cd src-tauri && cargo build

# Build release artifact
build-release:
    cd src-tauri && cargo build --release

# Run all tests
test:
    cd src-tauri && cargo test

# Lint: clippy -D warnings + fmt check
lint:
    cd src-tauri && cargo clippy -- -D warnings
    cd src-tauri && cargo fmt -- --check

# Apply rustfmt
fmt:
    cd src-tauri && cargo fmt

# Audit dependencies for known vulnerabilities
# RUSTSEC-2023-0071 ignored: affects sqlx-mysql (unused); we are SQLite-only, no upstream fix exists
audit:
    cd src-tauri && cargo audit --ignore RUSTSEC-2023-0071
