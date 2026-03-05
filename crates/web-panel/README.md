# Web Panel - Hexapod Control Interface

Interactive web-based control panel for the hexapod robot. Provides a visual interface for movement control, gait selection, and body pose adjustment.

## Features

### 🎮 Movement Control
- **Dual Joysticks** - Touch/mouse-based joystick controls
  - Left joystick: Forward/backward and strafe left/right
  - Right joystick: Rotation left/right
- Real-time movement with smooth responsiveness

### 🚶 Gait Selection
- Quick-switch between 6 different gaits:
  - Tripod (fast, stable)
  - Wave (slow, stable)
  - Ripple (medium speed)
  - Bipod (fast, less stable)
  - Quad (stable)
  - Hop (experimental)

### 🎭 Body Pose Control
- Sliders for precise body orientation:
  - **Roll**: -30° to +30° (tilt left/right)
  - **Pitch**: -30° to +30° (tilt forward/backward)
  - **Yaw**: -40° to +40° (rotate body)

### 📊 Real-Time Status Display
- Battery voltage and current monitoring
- Current gait display
- Connection status indicator

### 🛑 Emergency Stop
- Large, prominent emergency stop button
- Resets all controls to neutral

### 🔊 Text-to-Speech
- Type text to be spoken by the robot
- Voice selection (English/Slovak)
- Real-time feedback on speech status

## Usage

### Starting the Panel

The panel automatically starts when you run the hexapod application:

```bash
cargo run --features real
```

Default ports:
- **Web Panel**: http://0.0.0.0:8080
- **API**: http://0.0.0.0:3000

### Configuration

Edit `src/config.rs`:
```rust
pub const WEB_PANEL_ENABLE: bool = true;
pub const WEB_PANEL_PORT: u16 = 8080;
```

### Accessing the Panel

From the same device:
```
http://localhost:8080
```

From another device on the network:
```
http://<hexapod-ip>:8080
```

Example:
```
http://192.168.1.100:8080
http://hexapod.local:8080
```

### Public Domain (Cloudflared)

If you expose the services via separate tunnels, use subdomains:
- `https://hexapod.<domain>` → Web panel (port 8080)
- `https://hexapi.<domain>` → API (port 3000)
- `https://hexai.<domain>` → AI (port 3001)

The web panel auto-detects `hexapod.*` and targets `hexapi.*` / `hexai.*` for requests.
When opened on `http://hexapod.local:8080`, it uses local ports `3000` (API) and `3001` (AI).

## Interface Guide

### Movement Controls

**Forward/Strafe Joystick (Left)**
- Push up: Move forward
- Push down: Move backward
- Push left: Strafe left
- Push right: Strafe right
- Diagonal: Combined movement

**Rotation Joystick (Right)**
- Push left: Rotate counterclockwise
- Push right: Rotate clockwise
- Push up/down: Rotate (same as left/right)

### Status Cards

**Battery Card**
- Shows real-time voltage (e.g., "7.4V")
- Updates every second

**Current Card**
- Shows current draw (e.g., "1.5A")
- Helps monitor power consumption

**Gait Card**
- Shows active gait (e.g., "tri")
- Updates when gait changes

### Emergency Stop

Click the red "🛑 EMERGENCY STOP" button to:
- Stop all movement immediately
- Reset body pose to neutral
- Clear all joystick inputs
- Send immediate UBEC shutdown (SHUTDOWN 0)

### Text-to-Speech

The TTS section allows you to:
1. Type any text in the input field (max 500 characters)
2. Select a voice from the dropdown:
   - **English (Ryan)**: `en_US-ryan-medium`
   - **Slovak (Lili)**: `sk_SK-lili-medium`
3. Click "🔊 Speak" or press Enter to have the robot speak
4. Status messages show if speech is being processed

**Note**: In dummy mode, TTS will only print to console. In real mode, it requires a TTS server (configured in `src/config.rs`).

## Technical Details

### Architecture
```
Web Panel (Port 8080)
    ↓ HTTP Requests
API Server (Port 3000)
    ↓ Shared Controllers
Hexapod Hardware
```

The panel communicates with the API server, which shares controllers with the main application.

### Network Requirements
- Panel and API must be on the same network
- Browser must support modern JavaScript (ES6+)
- Touch events supported for mobile devices

### Browser Compatibility
- ✅ Chrome/Chromium (recommended)
- ✅ Firefox
- ✅ Safari (iOS/macOS)
- ✅ Edge
- ✅ Mobile browsers

## Mobile Usage

The panel is fully responsive and touch-optimized:
- Joysticks work with touch gestures
- Sliders respond to touch
- Optimized layout for small screens
- Portrait and landscape modes supported

## Customization

### Styling

The panel uses embedded CSS. To customize:
1. Edit `crates/web-panel/src/lib.rs`
2. Find the `<style>` section
3. Modify colors, sizes, layouts
4. Rebuild: `cargo build`

### Adding Features

To add new controls:
1. Add HTML elements in the `INDEX_HTML` constant
2. Add JavaScript handlers
3. Add corresponding API endpoints in `src/api/routes.rs`

## Troubleshooting

### "Disconnected" Status
- Check if API server is running (port 3000)
- Verify network connectivity
- Check browser console for errors

### Joysticks Not Responding
- Try clicking/touching directly on the stick
- Refresh the page
- Check browser console for JavaScript errors

### Controls Lag
- Reduce update frequency in JavaScript
- Check network latency
- Ensure hexapod isn't overloaded

### Can't Access from Another Device
- Check firewall settings
- Verify devices are on same network
- Use IP address instead of localhost
- Check if ports 3000 and 8080 are open

## Development

### Hot Reload

The HTML is embedded in Rust. To update:
1. Edit `crates/web-panel/src/lib.rs`
2. Rebuild: `cargo build`
3. Restart application
4. Refresh browser

### API Integration

The panel calls these API endpoints:
- `GET /api/health` - Connection check
- `GET /api/status` - Status updates
- `POST /api/move` - Movement control
- `POST /api/estop` - Emergency stop (SHUTDOWN 0)
- `POST /api/stop` - Graceful stop (SHUTDOWN 30 + poweroff)
- `POST /api/gait` - Gait selection
- `POST /api/pose` - Body pose
- `POST /api/tts` - Text-to-speech

### Adding Static Assets

Currently all assets are embedded. To add external files:
1. Use `tower-http` file serving
2. Create `static/` directory
3. Serve from `Router::nest_service()`

## Security Notes

⚠️ **Warning**: This panel has no authentication!
- Anyone on the network can control the hexapod
- Use on trusted networks only
- Consider adding authentication for production use

## Future Enhancements

Planned features:
- [ ] WebSocket for real-time updates
- [ ] Camera feed integration
- [ ] Touch-optimized gamepad mode
- [ ] Preset movement sequences
- [ ] Battery history graph
- [ ] Authentication/login system
- [ ] Multi-language support
- [ ] Dark/light theme toggle
