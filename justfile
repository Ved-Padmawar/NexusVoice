# NexusVoice task runner. Install: `cargo install just`. Usage: `just <recipe>`.
#
# Local runs are CPU-only so no Vulkan SDK is needed; CI adds `--features gpu`.
# The CUDA backend is a separate download the app fetches at runtime.

default:
    @just --list

# Run the dev build: its own data dir and a "Dev" badge, beside an installed copy
dev:
    cargo run

# Run an optimized build against your real NexusVoice data
run:
    cargo run --release

# Compile without running
build:
    cargo build

# Optimized build
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Lint and test, as CI does
check: lint test

# Clippy (pedantic, warnings are errors) and a format check
lint:
    cargo clippy --all-targets -- -W clippy::pedantic -D warnings
    cargo fmt -- --check

# Apply rustfmt
fmt:
    cargo fmt

# Audit dependencies for known vulnerabilities
# RUSTSEC-2023-0071 ignored: affects sqlx-mysql (unused); we are SQLite-only, no upstream fix exists
audit:
    cargo audit --ignore RUSTSEC-2023-0071
