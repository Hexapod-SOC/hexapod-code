# AI Agent Instructions

This document provides guidance for AI coding assistants working on the hexapod-code project.

## Project Build & Run Commands

### Using Cargo Make (Recommended)

The project uses `cargo-make` for streamlined build and run workflows:

```bash
# Run locally (automatically uses dummy features)
cargo make run

# Run on PC with dummy devices
cargo make pcrun

# Build for Raspberry Pi (cross-compilation)
cargo make pibuild

# Run remotely on Raspberry Pi
cargo make pirunremote
```

### Using Standard Cargo

If not using `cargo-make`, you must specify feature flags:

```bash
# For development/testing with dummy devices
cargo run --features dummy

# For real hardware (Raspberry Pi)
cargo run --features real
```

### NixOS Users

On NixOS, use `nix-shell` to access cross-compilation tools:

```bash
# Build for Raspberry Pi
nix-shell -p cargo-cross --run "cargo make pibuild"

# Run remotely on Raspberry Pi
nix-shell -p cargo-cross --run "cargo make pirunremote"
```

## Code Editing Guidelines

### Direct File Modifications

**ALWAYS modify files directly using the appropriate editing tools.**

- ❌ **DO NOT** print entire file contents to chat
- ❌ **DO NOT** show large code blocks unless explicitly requested
- ✅ **DO** use file editing tools to make targeted changes
- ✅ **DO** show only the relevant diff or changed section when explaining modifications

### Best Practices

1. **Use precise edits**: Make surgical changes to specific sections rather than rewriting entire files
2. **Read before editing**: Always read the current file content before making changes
3. **Verify changes**: Check that edits compile and don't introduce errors
4. **Explain concisely**: Briefly describe what was changed and why
5. **Batch related changes**: Group related modifications together when possible

## Project Structure

- `src/` - Main application code
  - `api/` - Web API server implementation
  - `config.rs` - Configuration management
  - `demos.rs` - Demo movement sequences
  - `hexapod.rs` - Core hexapod controller
  - `main.rs` - Application entry point
- `crates/` - Workspace crates
  - `audio/` - Text-to-speech and audio playback
  - `devices/` - Hardware abstraction (servo, PicoUBEC)
  - `movement/` - Inverse kinematics and gait patterns
  - `web-panel/` - Web control interface
- `examples/` - Example programs and utilities
- `target/` - Build artifacts (gitignored)

## Feature Flags

- `dummy` - Use simulated hardware (for development on any platform)
- `real` - Use real hardware interfaces (requires Raspberry Pi with actual servos)

The features are mutually exclusive. The project defaults to `dummy` for safety.

## Cross-Compilation

The project targets `aarch64-unknown-linux-gnu` for Raspberry Pi deployment. The cross-compilation setup is configured in:
- `Cross.toml` - Cross-compilation configuration
- `Cargo.toml` - Build dependencies and features
- `Makefile.toml` - Cargo-make task definitions

## Testing

```bash
# Run tests with dummy features
cargo test --features dummy

# Run specific example
cargo run --example servo_center --features dummy
```

## Documentation References

See these files for more detailed information:
- `README.md` - Project overview and setup

## Common Tasks

### Adding a new dependency
Edit `Cargo.toml` in the appropriate crate directory.

### Modifying API endpoints
Edit files in `src/api/` directory.

### Adjusting movement patterns
Edit files in `crates/movement/src/`.

### Updating web interface
Edit files in `crates/web-panel/static/`.

---

**Remember**: This is a robotics project with real hardware. Always test with `--features dummy` before deploying to actual hardware.
