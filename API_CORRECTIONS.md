# API Corrections Applied

## Summary of Changes

The JavaScript client has been updated to correctly communicate with the Rust API in `src/api/routes.rs`.

## API Endpoint Corrections

### Base URL
- **Before**: `http://localhost:3000`
- **After**: `http://localhost:3000/api`

All API routes are prefixed with `/api`.

### Movement Command (`POST /api/move`)

**API Expects:**
```json
{
  "forward": 0.0,   // -100.0 to 100.0 mm/s
  "strafe": 0.0,    // -100.0 to 100.0 mm/s
  "rotation": 0.0   // -1.0 to 1.0 rad/s
}
```

**JavaScript Changes:**
- Joystick X/Y values (-1 to 1) are now multiplied by 100 to convert to mm/s
- Parameter names changed from `{x, y, rotation}` to `{forward, strafe, rotation}`
- `forward` = Y-axis (forward/backward)
- `strafe` = X-axis (left/right)

### Gait Selection (`POST /api/gait`)

**API Expects:**
```json
{
  "gait_name": "tri"  // or "wave", "ripple", "bi", "quad", "hop"
}
```

**JavaScript Changes:**
- Parameter changed from `gait` to `gait_name`
- Default gait changed from "tripod" to "tri"
- HTML button `data-gait` attributes updated to match API names

**Available Gaits:**
- `tri` - Tripod gait
- `wave` - Wave gait
- `ripple` - Ripple gait
- `bi` - Bipod gait
- `quad` - Quad gait
- `hop` - Hop gait

### Body Pose (`POST /api/pose`)

**API Expects:**
```json
{
  "roll": 0.0,   // degrees
  "pitch": 0.0,  // degrees
  "yaw": 0.0     // degrees
}
```

**JavaScript Changes:**
- Only sends `roll`, `pitch`, `yaw` (removed `x`, `y`, `z` position parameters)
- The API currently only supports body rotation, not translation
- All pose-related functions updated to only send rotation values

### Emergency Stop (`POST /api/stop`)

**API Expects:**
```json
{}
```

**JavaScript Changes:**
- Added empty JSON body to POST request
- Added proper Content-Type header

### Status Updates (`GET /api/status` and `GET /api/battery`)

**API Returns (status):**
```json
{
  "battery": { ... },
  "gait_phase": 0.0,
  "gait_name": "tri"
}
```

**API Returns (battery):**
```json
{
  "voltage": 7.40,
  "current": 1.50,
  "power_state": "Normal",
  "has_data": true
}
```

**JavaScript Changes:**
- Changed `status.gait` to `status.gait_name`
- Changed `status.state` to `battery.power_state`
- Correctly displays battery voltage and current

## Gamepad Controls Updated

All gamepad control functions now use the correct API format:
- Movement: `{forward, strafe, rotation}` with proper scaling
- Pose adjustments: Only `{roll, pitch, yaw}`
- Emergency stop: Proper JSON body

## Files Modified

1. `crates/web-panel/static/app.js` - All API calls corrected
2. `crates/web-panel/static/index.html` - Gait button data attributes updated
3. `crates/web-panel/src/lib.rs` - Removed unused import warning

## Testing Checklist

- [x] API base URL includes `/api` prefix
- [x] Movement commands use correct parameter names
- [x] Gait selection uses "tri" instead of "tripod"
- [x] Pose control only sends roll/pitch/yaw
- [x] Emergency stop has proper request body
- [x] Status display shows correct fields
- [x] Gamepad controls use correct API format
- [x] Favicon added to prevent 404 error

## Next Steps

To test the corrected API integration:

```bash
cargo make pcrun
```

Then open `http://localhost:8080` in your browser and verify:
1. Connection status shows "Connected"
2. Battery voltage and current display correctly
3. Gait name shows current gait (e.g., "tri")
4. Movement joysticks send commands
5. Gait buttons switch gaits
6. Pose sliders adjust body rotation
7. Gamepad controls work (if gamepad connected)
