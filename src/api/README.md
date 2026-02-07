# Hexapod HTTP API

HTTP REST API for controlling and monitoring the hexapod robot. This API is built into the main application as a module in `src/api/`.

## Architecture

```
src/
  api/
    mod.rs          # Module exports
    state.rs        # Shared state with Arc<Mutex<>> controllers
    routes.rs       # HTTP endpoint handlers
    server.rs       # Axum server setup
```

The API shares controllers directly with the main hexapod application using `Arc<Mutex<>>` for thread-safe access. This means:
- No duplicate I2C device connections
- Real-time state synchronization
- Safe concurrent access from web and main loop

## Running the Server

Enable in `src/config.rs`:
```rust
pub const WEB_ENABLE: bool = true;
pub const API_PORT: u16 = 3000;
```

Then run:
```bash
cargo run --features real
```

The API server will start in the background while the main application continues running.

## API Endpoints

### Health Check
```
GET /api/health
```
Response:
```json
{
  "status": "healthy",
  "version": "0.0.0"
}
```

### Get Complete Status
```
GET /api/status
```
Response:
```json
{
  "battery": {
    "voltage": 7.4,
    "current": 1.5,
    "power_state": "Normal",
    "has_data": true
  },
  "gait_phase": 0.35,
  "gait_name": "tri"
}
```

### Get Battery Status
```
GET /api/battery
```
Response:
```json
{
  "voltage": 7.4,
  "current": 1.5,
  "power_state": "Normal",
  "has_data": true
}
```

### Get Leg Kinematics
```
GET /api/legs
```
Response:
```json
{
  "gait_phase": 0.35,
  "gait_name": "tri",
  "velocity": [50.0, 0.0, 0.0],
  "rotation": 0.0,
  "body_pose": { "roll": 0.0, "pitch": 0.0, "yaw": 0.0, "x": 0.0, "y": 0.0, "z": 0.0 },
  "legs": {
    "left_front": {
      "position": [10.0, -45.0, -70.0],
      "angles_deg": [90.0, 120.0, 80.0],
      "angles_tweaked_deg": [90.0, 120.0, 80.0],
      "angles_rad": [0.0, -0.6, 1.4]
    }
  }
}
```

### Move Hexapod
```
POST /api/move
Content-Type: application/json

{
  "forward": 50.0,    // -100 to 100 mm/s
  "strafe": 0.0,      // -100 to 100 mm/s  
  "rotation": 0.0     // -1 to 1 rad/s
}
```

### Stop Movement
```
POST /api/stop
Content-Type: application/json

{}
```

### Get Current Gait
```
GET /api/gait
```
Response:
```json
{
  "success": true,
  "message": "Current gait",
  "current_gait": "tri"
}
```

### Set Gait
```
POST /api/gait
Content-Type: application/json

{
  "gait_name": "wave"
}
```
Valid gaits: `tri`, `wave`, `ripple`, `bi`, `quad`, `hop`

### Set Body Pose
```
POST /api/pose
Content-Type: application/json

{
  "roll": 15.0,   // degrees
  "pitch": 0.0,   // degrees
  "yaw": 0.0      // degrees
}
```

## Usage Examples

### cURL
```bash
# Get status
curl http://localhost:3000/api/status

# Move forward
curl -X POST http://localhost:3000/api/move \
  -H "Content-Type: application/json" \
  -d '{"forward": 50.0, "strafe": 0.0, "rotation": 0.0}'

# Change gait
curl -X POST http://localhost:3000/api/gait \
  -H "Content-Type: application/json" \
  -d '{"gait_name": "wave"}'
```

### JavaScript (Fetch API)
```javascript
// Get status
const status = await fetch('http://hexapod.local:3000/api/status')
  .then(r => r.json());

// Move hexapod
await fetch('http://hexapod.local:3000/api/move', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    forward: 50.0,
    strafe: 0.0,
    rotation: 0.0
  })
});
```

### Python (requests)
```python
import requests

# Get battery
response = requests.get('http://hexapod.local:3000/api/battery')
battery = response.json()
print(f"Voltage: {battery['voltage']}V")

# Move
requests.post('http://hexapod.local:3000/api/move', json={
    'forward': 50.0,
    'strafe': 0.0,
    'rotation': 0.0
})
```

## CORS

CORS is enabled for all origins. For production, configure in `src/api/server.rs`:
```rust
.layer(CorsLayer::new()
    .allow_origin("http://your-frontend.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::GET, Method::POST])
)
```

## Future Extensions

### Planned Features
- WebSocket support for real-time updates
- Video streaming endpoint
- Sensor data endpoints (when added to devices crate):
  - `GET /api/sensors/gyro`
  - `GET /api/sensors/lidar`
  - `GET /api/sensors/environment`

### Adding New Endpoints

1. Add handler function in `src/api/routes.rs`
2. Add route in `src/api/server.rs`
3. Test with curl or frontend

Example:
```rust
// In routes.rs
pub async fn get_temperature(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TempResponse>, StatusCode> {
    // Your implementation
}

// In server.rs
.route("/api/sensors/temp", get(routes::get_temperature))
```

## Integration with Main Application

The API runs in a background tokio task and shares the same controller instances as the main application. This means:

- Changes made via API are immediately reflected in main loop
- Battery monitoring continues while API is active
- Emergency shutdown procedures work regardless of API state
- No race conditions due to mutex protection

## Why Built-In Module?

The API is tightly coupled to this specific hexapod implementation:
- Uses hexapod-specific types and controllers
- Not intended as a reusable library
- Benefits from direct access to `src/` code
- Simpler than separate crate for this use case

For a generic robot API, see the planned `crates/api/` design in future versions.
