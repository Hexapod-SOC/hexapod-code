# Leg Calibration Feature

This document describes the leg position calibration feature added to the hexapod control system.

## Overview

The leg calibration feature allows you to adjust the default position of each leg through the web interface, test the positions in real-time, and then copy the calibrated values as Rust code to update your configuration constants.

## How It Works

### Web Interface

The calibration section is located in the web control panel and includes:

- **6 leg position controls** (Left Front, Left Middle, Left Back, Right Front, Right Middle, Right Back)
- Each leg has **3 sliders** for X, Y, Z coordinates:
  - **X**: Forward/backward position (-100 to 100 mm)
  - **Y**: Left/right (strafe) position (-150 to 150 mm)
  - **Z**: Height position (-150 to 0 mm, where 0 is body level)

### Control Buttons

1. **Load Current Values**: Fetches the current leg positions from the robot
2. **Apply & Print to Console**: Applies the new positions to the robot AND prints Rust code to the terminal
3. **Reset to Default**: Resets all sliders to the default hardcoded values

## Usage Workflow

1. **Start the hexapod** with `cargo make run` or `cargo make pirunremote`
2. **Open the web interface** at `http://localhost:3000` (or robot's IP)
3. **Navigate to the "Leg Position Calibration" section**
4. **Adjust the sliders** for each leg to find the optimal stance
5. **Test the positions** - the robot updates in real-time as you move sliders
6. **Click "Apply & Print to Console"** when satisfied
7. **Check the terminal output** - you'll see Rust code like:

```rust
=== Calibrated Leg Stance (copy to constants) ===
LegStances {
    left_front: Vec3::new(0.0, -45.0, -70.0),
    left_middle: Vec3::new(0.0, -55.0, -50.0),
    left_back: Vec3::new(0.0, -45.0, -70.0),
    right_front: Vec3::new(0.0, 45.0, -70.0),
    right_middle: Vec3::new(0.0, 55.0, -50.0),
    right_back: Vec3::new(0.0, 45.0, -70.0),
}
==================================================
```

8. **Copy the printed code** and update your configuration

## Where to Update the Code

The default leg stance is defined in `crates/movement/src/gait.rs`:

```rust
impl Default for LegStances {
    fn default() -> Self {
        LegStances {
            left_front: Vec3::new(0.0, -45.0, -70.0),
            left_middle: Vec3::new(0.0, -55.0, -50.0),
            left_back: Vec3::new(0.0, -45.0, -70.0),
            right_front: Vec3::new(0.0, 45.0, -70.0),
            right_middle: Vec3::new(0.0, 55.0, -50.0),
            right_back: Vec3::new(0.0, 45.0, -70.0),
        }
    }
}
```

Simply replace these values with your calibrated ones.

## API Endpoints

### GET `/api/leg_stance`

Returns the current default leg stance configuration.

**Response:**
```json
{
  "success": true,
  "message": "Current leg stance",
  "current_stance": {
    "left_front": [0.0, -45.0, -70.0],
    "left_middle": [0.0, -55.0, -50.0],
    "left_back": [0.0, -45.0, -70.0],
    "right_front": [0.0, 45.0, -70.0],
    "right_middle": [0.0, 55.0, -50.0],
    "right_back": [0.0, 45.0, -70.0]
  }
}
```

### POST `/api/leg_stance`

Updates the default leg stance and prints Rust code to console.

**Request:**
```json
{
  "left_front": [0.0, -45.0, -70.0],
  "left_middle": [0.0, -55.0, -50.0],
  "left_back": [0.0, -45.0, -70.0],
  "right_front": [0.0, 45.0, -70.0],
  "right_middle": [0.0, 55.0, -50.0],
  "right_back": [0.0, 45.0, -70.0]
}
```

**Response:**
```json
{
  "success": true,
  "message": "Leg stance updated and printed to console",
  "current_stance": { ... }
}
```

## Code Changes Made

### Backend
- `crates/movement/src/gait.rs`: Added `to_array()` method to `LegStances`
- `crates/movement/src/controller.rs`: Added `set_default_stance()` and `get_default_stance()` methods
- `src/api/routes.rs`: Added `get_leg_stance()` and `set_leg_stance()` endpoints
- `src/api/server.rs`: Added routes for leg calibration endpoints

### Frontend
- `crates/web-panel/static/index.html`: Added calibration UI section with sliders
- `crates/web-panel/static/app.js`: Added calibration JavaScript functions
- `crates/web-panel/static/style.css`: Added styling for calibration controls

## Benefits

1. **No code recompilation needed for testing** - adjust positions on the fly
2. **Visual feedback** - see the robot move as you adjust sliders
3. **Easy to copy** - Rust code is automatically formatted for you
4. **Persistent** - changes remain active until robot restart
5. **Safe** - values are constrained by slider ranges

## Notes

- Changes made through the web interface are **runtime only** and will reset on restart
- To make changes permanent, you must copy the printed code into the source and recompile
- The coordinate system: X=forward/back, Y=left/right, Z=up/down
- Negative Z values mean below the body center
