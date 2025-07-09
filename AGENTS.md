# AGENTS.md - Miro PDF Viewer Development Guide

## Build/Test Commands
- `cargo build` - Build the project
- `cargo run` - Run the application
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run a specific test
- `cargo check` - Check code without building, use this before each build since it is a lot faster

## Code Style Guidelines
- Use `snake_case` for variables, functions, and modules
- Use `PascalCase` for types, structs, enums, and traits
- Prefer explicit types over `auto`/inference when clarity improves
- Use `anyhow::Result` for error handling with context
- Import organization: std first, external crates, then local modules
- Use `#[derive(Debug)]` on all structs and enums
- Prefer `match` over `if let` for complex pattern matching
- Use `const` for compile-time constants (e.g., `const MOVE_STEP: f32 = 40.0`)
- Use `static` with `LazyLock` for global state (e.g., `CONFIG`)
- Implement `Default` trait where appropriate
- Use `EnumString` derive for string-to-enum conversion
- Prefer `PathBuf` over `&str` for file paths
- Use `tokio::sync` primitives for async communication
- Structure modules with `mod.rs` files for organization
- Use the `debug!`, `info!` and `error!` from `tracing` for printing and logging
