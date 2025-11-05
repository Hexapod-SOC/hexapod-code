# API Restructure - Summary

## What Changed

Moved HTTP API from separate `crates/web/` to integrated `src/api/` module.

## File Movements

```
crates/web/src/                →  src/api/
  ├── lib.rs                   →  mod.rs
  ├── state.rs                 →  state.rs
  ├── api.rs                   →  routes.rs
  └── server.rs                →  server.rs
```

## New Structure

```
src/
  ├── main.rs           # Main application
  ├── config.rs         # Configuration
  ├── hexapod.rs        # Hexapod controller
  ├── demos.rs          # Demo routines
  └── api/              # HTTP API (NEW LOCATION)
      ├── mod.rs        # Module exports
      ├── state.rs      # Shared state
      ├── routes.rs     # HTTP handlers
      ├── server.rs     # Axum server
      └── README.md     # API documentation

crates/
  ├── audio/            # TTS module
  ├── devices/          # Hardware interface
  └── movement/         # Kinematics
  (web/ removed)
```

## Changes Made

### 1. Created `src/api/` module structure
- Moved all web code from `crates/web/src/` to `src/api/`
- Updated imports to use module-relative paths
- Changed `crate::` references to `super::`

### 2. Updated `Cargo.toml`
- Removed `web` from dependencies
- Removed `crates/web` from workspace members
- Added API dependencies directly to main package:
  - axum
  - serde/serde_json
  - tower/tower-http
  - tracing/tracing-subscriber

### 3. Updated `src/main.rs`
- Added `pub mod api;`
- Changed `web::` to `api::`
- API now accessed as local module

## Benefits

### ✅ Simpler Structure
- One less crate to manage
- No workspace path dependencies
- Faster compilation (single unit)

### ✅ Better Integration
- API can directly access `config.rs`
- Can share types from `hexapod.rs` without re-export
- Natural module hierarchy

### ✅ Easier Development
- Make changes without crate boundaries
- Better IDE support
- Simpler imports

### ✅ Still Modular
- Clear separation in `src/api/`
- Can easily extract later if needed
- Clean module interface

## Future Plans

When Bluetooth or other APIs are added:

### Option 1: Keep in src/
```
src/
  ├── api/
  │   ├── http/         # HTTP transport
  │   ├── bluetooth/    # BLE transport
  │   └── common/       # Shared types
```

### Option 2: Extract to crate
```
crates/
  ├── api/              # Generic API layer
  ├── http-api/         # HTTP implementation
  └── bluetooth-api/    # BLE implementation
```

Choice depends on whether API becomes reusable across projects.

## Testing

API still works exactly the same:

```bash
# Start hexapod with API
cargo run --features real

# Test endpoint
curl http://localhost:3000/api/status
```

## Migration Notes

If you had external code importing `web` crate:
- Code inside `src/main.rs`: Change `web::` to `api::`
- External projects: Would need to import hexapod-code as library (not common use case)

## Documentation

- API docs: `src/api/README.md`
- Architecture: This file
- Original design notes: `WEB_API_CHANGES.md`
