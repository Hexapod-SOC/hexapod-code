# Hexapod Web API

HTTP REST API for controlling and monitoring the hexapod robot.

## Running the Server

Start the API server:

```bash
# Default port (3000)
cargo run --features real -- --api

# Custom port
cargo run --features real -- --api --port 8080

# Dummy mode for testing
cargo run --features dummy -- --api
```

## API Endpoints

### Health Check

```
GET /api/health
```

Response:
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### Get Status

Get complete hexapod status including battery and gait info.

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

### Move Hexapod

Control hexapod movement.

```
POST /api/move
Content-Type: application/json

{
  "forward": 50.0,    // -100 to 100 mm/s
  "strafe": 0.0,      // -100 to 100 mm/s  
  "rotation": 0.0     // -1 to 1 rad/s
}
```

Response:
```json
{
  "success": true,
  "message": "Moving: forward=50.0, strafe=0.0, rotation=0.00"
}
```

### Stop Movement

```
POST /api/stop
Content-Type: application/json

{}
```

Response:
```json
{
  "success": true,
  "message": "Hexapod stopped"
}
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

Change the walking gait pattern.

```
POST /api/gait
Content-Type: application/json

{
  "gait_name": "wave"
}
```

Valid gait names: `tri`, `wave`, `ripple`, `bi`, `quad`, `hop`

Response:
```json
{
  "success": true,
  "message": "Gait changed to wave",
  "current_gait": "wave"
}
```

### Set Body Pose

Control body orientation without walking.

```
POST /api/pose
Content-Type: application/json

{
  "roll": 15.0,   // degrees
  "pitch": 0.0,   // degrees
  "yaw": 0.0      // degrees
}
```

Response:
```json
{
  "success": true,
  "message": "Body pose set: roll=15.0°, pitch=0.0°, yaw=0.0°"
}
```

## Example Usage

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

# Tilt body
curl -X POST http://localhost:3000/api/pose \
  -H "Content-Type: application/json" \
  -d '{"roll": 15.0, "pitch": 0.0, "yaw": 0.0}'
```

### JavaScript

```javascript
// Get status
const status = await fetch('http://hexapod.local:3000/api/status')
  .then(r => r.json());

// Move forward
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

### Python

```python
import requests

# Get battery status
response = requests.get('http://hexapod.local:3000/api/battery')
battery = response.json()
print(f"Voltage: {battery['voltage']}V")

# Move hexapod
requests.post('http://hexapod.local:3000/api/move', json={
    'forward': 50.0,
    'strafe': 0.0,
    'rotation': 0.0
})
```

## CORS

CORS is enabled for all origins in development. For production, configure appropriate CORS settings in `server.rs`.

## Future Sensors

When additional sensors are added to the `devices` crate, new endpoints will be added:

- `GET /api/sensors/gyro` - IMU/gyroscope data
- `GET /api/sensors/lidar` - LIDAR distance measurements
- `GET /api/sensors/environment` - BME680 temperature/humidity/pressure
