---
name: cargo
description: Build, run, test, check, format, and lint the Rust workspace. Use for any cargo operations on the rust-ui project.
---

# Cargo Skill

Common cargo commands for the rust-ui workspace. All commands run in the project root.

## Build

```bash
cargo build
```

Builds all crates in the workspace.

## Run

```bash
cargo run
```

Runs the default binary (demo). To run a specific binary:

```bash
cargo run --package <name>
```

Available packages: `demo`, `frontend`, `nurbs`, `time-series`

## Test

Run all tests:
```bash
cargo test
```

Run a specific test by name:
```bash
cargo test <test_name>
```

Example from AGENTS.md:
```bash
cargo test can_load_rectangle_rendering_shader
```

## Check

Quick syntax and type checking without building:
```bash
cargo check
```

## Format

Format all code:
```bash
cargo fmt
```

## Lint

Run clippy lints:
```bash
cargo clippy
```

Fix auto-fixable lints:
```bash
cargo clippy --fix --allow-dirty
```

## Common Options

- `--release` - Build with optimizations
- `--package <name>` - Target specific crate
- `--lib` - Build only library targets
- `--bins` - Build only binaries

## Workspace Crates

- `rust-ui` - Core UI library
- `demo` - Hello world demo
- `time-series` - Real-time sequential data explorer
- `frontend` - Frontend for demiurge
- `modes` - Input library
- `nurbs` - Bezier surface rasterizer
