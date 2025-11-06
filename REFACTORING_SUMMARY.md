# Hexapod Control Refactoring Summary

## Overview
Refactored the hexapod control architecture to centralize all movement calculations in `hexapod.rs` with a unified `update()` function. Control interfaces (web API, future Bluetooth, radio, etc.) now only modify control variables instead of performing calculations.

## Key Changes

### 1. **New Control Structure** (`src/hexapod.rs`)

#### `HexapodControl` struct
- Central control state that any interface can modify
- Contains: velocity, rotation, body_pose, enabled flag
- Shared via `Arc<Mutex<HexapodControl>>`

#### `Hexapod::update(dt)` method
- Single source of truth for movement updates
- Called periodically in main loop (~20Hz)
- Reads control state and applies calculations
- Updates: battery monitoring, gait phase, IK calculations, servo positions

#### Benefits
- **Separation of concerns**: Control input vs movement execution
- **Easy extensibility**: Add new control interfaces without touching movement logic
- **Thread-safe**: All control interfaces share the same Arc<Mutex> references
- **Testable**: Movement logic isolated from input sources

### 2. **Simplified API** (`src/api/`)

#### `AppState` (`state.rs`)
- Now only contains shared references (no ownership)
- References: `HexapodControl`, `GaitController`, `PicoUbecController`
- No movement calculations in API layer

#### Routes (`routes.rs`)
- `/api/move` - Sets velocity/rotation in control state
- `/api/stop` - Zeros velocity/rotation
- `/api/pose` - Sets body pose
- `/api/gait` - Changes gait template
- `/api/status` - Reads status (no calculations)
- `/api/battery` - Reads battery info

#### Server (`server.rs`)
- **Removed**: Movement update loop (was doing `hexapod.update()` in API)
- Now just serves HTTP requests
- Non-blocking, runs in background tokio task

### 3. **Updated Main Loop** (`src/main.rs`)

#### Centralized Update Loop
```rust
loop {
    interval.tick().await;
    hexapod.update(0.05).await; // 50ms = 20Hz
    
    // Battery monitoring
    // Safety checks
}
```

- Runs at 20Hz (50ms intervals)
- Calls `hexapod.update()` which reads control state
- Handles battery monitoring and critical shutdowns
- API/gamepad just modify control state in background

### 4. **Updated Demos** (`src/demos.rs`)

- All demo functions now use `set_velocity()` and `set_body_pose()`
- Demos include their own update loops
- No direct servo/gait manipulation
- More realistic of how external controllers will work

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                      Control Inputs                      │
│  (Web API, Bluetooth, Radio, Gamepad, Demos, etc.)     │
└────────────────────┬────────────────────────────────────┘
                     │ Modify control variables
                     ↓
         ┌──────────────────────────┐
         │   HexapodControl State   │
         │  Arc<Mutex<...>>         │
         │  - velocity              │
         │  - rotation              │
         │  - body_pose             │
         │  - enabled               │
         └──────────┬───────────────┘
                    │ Read by
                    ↓
         ┌──────────────────────────┐
         │   hexapod.update(dt)     │  ← Called in main loop (20Hz)
         │                          │
         │  1. Read control state   │
         │  2. Update gait phase    │
         │  3. Calculate IK         │
         │  4. Apply to servos      │
         │  5. Update battery       │
         └──────────────────────────┘
```

## Future Extensions

### Easy to Add
1. **Bluetooth Control**: Create module that modifies `HexapodControl`
2. **Radio Control**: Same pattern - just modify control variables
3. **Gamepad**: Already has infrastructure via web panel
4. **ROS Integration**: Publish control state, subscribe to commands
5. **Autonomous Navigation**: AI reads sensors, sets control variables

### Example: Adding Bluetooth
```rust
// In future bluetooth.rs module
pub async fn bluetooth_control_loop(control: Arc<Mutex<HexapodControl>>) {
    loop {
        let cmd = bluetooth_receive_command().await;
        let mut ctrl = control.lock().await;
        ctrl.velocity = cmd.velocity;
        ctrl.rotation = cmd.rotation;
        // Done! hexapod.update() will handle the rest
    }
}
```

## Migration Notes

### Breaking Changes
- `Hexapod` API changed - now provides `get_control()`, `get_gait_controller()`, etc.
- Demos now use `set_velocity()` / `set_body_pose()` instead of direct access
- API server no longer runs movement loop

### Backwards Compatibility
- Config files unchanged
- Gait system unchanged  
- IK system unchanged
- Servo interface unchanged
- Web panel unchanged (uses same API endpoints)

## Testing

Run with dummy features to test without hardware:
```bash
cargo check --features dummy
cargo run --features dummy
```

## Performance

- Main loop: 20Hz (50ms intervals)
- API: Non-blocking, async handlers
- Control state updates: Instant (Arc<Mutex> overhead minimal)
- No performance regression vs previous architecture

## Summary

The refactoring achieves:
✅ Clean separation of control input vs execution
✅ Easy to add new control interfaces  
✅ Thread-safe shared state
✅ Centralized update loop
✅ Simplified API layer
✅ Better testability
✅ Maintains all existing functionality

Next steps for multi-interface control:
1. Implement Bluetooth module
2. Add radio control module  
3. Implement control priority system (e.g., emergency stop overrides)
4. Add control input validation/limiting
