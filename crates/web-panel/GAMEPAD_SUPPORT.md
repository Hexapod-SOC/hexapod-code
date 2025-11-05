# Gamepad Support Documentation

## Overview
The web panel now includes full gamepad support using the HTML5 Gamepad API. You can control the hexapod using Xbox or PlayStation controllers.

## Features

### Controller Layout Toggle
- **Xbox Layout**: Default mapping for Xbox controllers (Xbox One, Xbox Series X/S)
- **PlayStation Layout**: Mapping for PlayStation controllers (DualShock 4, DualSense)

### Controls

#### Analog Sticks
- **Left Stick**: 
  - Forward/Backward movement (Y-axis)
  - Left/Right strafing (X-axis)
- **Right Stick**: 
  - Rotation left/right (X-axis)
- **Deadzone**: 0.15 (15%) to prevent stick drift

#### Buttons

##### Xbox Layout:
- **A Button**: Emergency Stop
- **B Button**: Center Pose (reset all body pose values to 0)
- **D-Pad Up**: Increase pitch
- **D-Pad Down**: Decrease pitch
- **D-Pad Left**: Decrease roll
- **D-Pad Right**: Increase roll

##### PlayStation Layout:
- **X Button (Cross)**: Emergency Stop
- **O Button (Circle)**: Center Pose (reset all body pose values to 0)
- **D-Pad Up**: Increase pitch
- **D-Pad Down**: Decrease pitch
- **D-Pad Left**: Decrease roll
- **D-Pad Right**: Increase roll

### Connection Status
The gamepad section displays:
- Connection indicator (red dot = disconnected, green dot = connected)
- Controller name/model when connected
- Control mapping guide

## Usage

1. **Connect your gamepad** to your computer via USB or Bluetooth
2. **Open the web panel** at `http://localhost:8080`
3. **Press any button** on the gamepad to activate it
4. **Select your controller layout** (Xbox or PlayStation) using the toggle buttons
5. **Control the hexapod** using the analog sticks and buttons

## Technical Details

### Gamepad API Implementation
- Uses `navigator.getGamepads()` for polling
- Auto-detects gamepad connection/disconnection events
- Runs at ~60 FPS using `requestAnimationFrame`
- Button press detection with state tracking to prevent repeats

### Browser Compatibility
- Chrome/Edge: Full support
- Firefox: Full support
- Safari: Limited support (may require user gesture)

### Axis Mapping (Standard Gamepad)
```
axes[0] = Left Stick X
axes[1] = Left Stick Y
axes[2] = Right Stick X
axes[3] = Right Stick Y (not used)
```

### Button Mapping (Standard Gamepad)
```
buttons[0]  = A/Cross
buttons[1]  = B/Circle
buttons[2]  = X/Square
buttons[3]  = Y/Triangle
buttons[4]  = LB/L1
buttons[5]  = RB/R1
buttons[6]  = LT/L2
buttons[7]  = RT/R2
buttons[8]  = Select/Share
buttons[9]  = Start/Options
buttons[10] = L3 (Left stick button)
buttons[11] = R3 (Right stick button)
buttons[12] = D-Pad Up
buttons[13] = D-Pad Down
buttons[14] = D-Pad Left
buttons[15] = D-Pad Right
```

## Testing

To test gamepad support:
1. Connect a gamepad
2. Open browser console (F12)
3. Watch for connection messages
4. Test each control and verify commands are sent to the API
5. Check emergency stop functionality

## Troubleshooting

### Gamepad not detected
- Try pressing any button after connecting
- Check if gamepad is recognized by your OS
- Try disconnecting and reconnecting
- Refresh the page

### Wrong button mapping
- Toggle between Xbox and PlayStation layouts
- Some third-party controllers may use non-standard mappings
- Check browser console for gamepad ID

### Drift or sensitivity issues
- Adjust the deadzone value in `app.js` (currently 0.15)
- Calibrate your controller in your OS settings

## Future Enhancements
- Custom button mapping
- Adjustable deadzone from UI
- Trigger button support for gait switching
- Vibration feedback
- Multiple controller support
