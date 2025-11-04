# Hexapod Robot Control System

A Rust-based control system for a hexapod robot with 18 servos (3 per leg), featuring inverse kinematics, tripod gait walking, and text-to-speech capabilities.

## Features

- **Inverse Kinematics**: Calculate joint angles for desired leg positions using [`SimpleIK`](src/movement/ik.rs)
- **[WIP] Tripod Gait Walking**: Efficient 6-legged walking pattern implementation
- **Servo Control**: Dual PCA9685 PWM controllers for precise servo management via [`ServoController`](src/devices/servos.rs)
- **Text-to-Speech**: Piper TTS integration with caching and multi-language support (English/Slovak)
- **TTS Cache**: Tmp TTS Cache for repeated sayings
- **Audio Playback**: WAV and ~~MP3~~ file playback capabilities
- **Async Runtime**: Built on Tokio for efficient concurrent operations
- **[WIP] Config.toml**: Move from hardcoded config.rs to Config.toml

## Todo
- [ ]  make the tts + audio run on separate thread bcose rn its blocking the main thread

## Project Structure

```
├── src/
│   ├── main.rs              # Main application entry point
│   ├── macros.rs            # TTS convenience macros
│   ├── config.rs            # Hardcoded confugration in the future move to Config.toml
├── crates/
│   ├── devices/             # Controller ServoPCA / BME680 / LIDAR / ...
│   └── movement/            # Inverse Kinematics, gaits, etc...
│   └── audio/               # Audio control and tts / sst
```

## Installation

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cross-compilation tool
# Only required for cross compiling to PI
# Local dummy run on amd64 platform doesnt use it 
cargo install --force cross

# Install cargo-make for task runner
cargo install --no-default-features --force cargo-make

# For remote code running on the hexapod
# [ONLY USE WHEN HEXAPOD WITH YOU]
cargo install --git https://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@github.com/Adam-Grman/cargo-hexapod-devkit.git
```

### Building

**For PC (dummy mode aka fake sensors):**
```bash
cargo make pcbuild
```

**For Raspberry Pi (cross-compile with real sensors):**
```bash
cargo make pibuild
```

**Release builds:**
```bash
cargo make pcbuildrelease  # PC dummy sensors
cargo make pibuildrelease  # Pi real sensors
```

## Configuration

### Servo Pin Mapping

Configure servo pins in [`src/main.rs`](src/main.rs):

```rust
const SERVO_PINS: ServoPins = ServoPins {
    left_front: (0, 1, 2),    // Coxa, Femur, Tibia
    left_middle: (4, 5, 6),
    left_back: (8, 9, 10),
    right_front: (0, 1, 2),
    right_middle: (4, 5, 6),
    right_back: (8, 9, 10),
};
```

### Inverse Kinematics Parameters

Adjust leg segment lengths in [`cargo/movement/ik.rs`](cargo/movement/ik.rs):

```rust
// FIXME
```

### TTS Configuration

Set environment variables or use defaults:

```bash
# FIXME
```

## Usage

### Running the Demo

**On PC (fake sensors):**
```bash
cargo make pcrun
```

**Remote deployment and run:**
** [ONLY USE WHEN HEXAPOD WITH YOU] **
```bash
cargo make pirunremote
```

### Basic Movement Control

```rust
use movement::Movement;
use glam::Vec3;

// Initialize controllers
let ik = SimpleIK::new();
let servos = ServoController::new(SERVO_PINS);
let mut movement = Movement::new(servos, ik);

// Move a leg to a position (X, Y, Z in mm)
movement.move_leg_to_position(
    Leg::LeftFront, 
    Vec3::new(30.0, 50.0, -80.0)
);
```

## Walking Gait

The tripod gait divides legs into two groups that alternate:
- **Tripod 1**: RightFront, LeftMiddle, RightBack
- **Tripod 2**: LeftFront, RightMiddle, LeftBack

## API Reference

### Movement Module

- [`Movement::move_leg_to_position(leg, position)`](src/movement/movement.rs) - Move a leg to 3D position
- [`SimpleIK::calculate_leg_angles(leg, pos)`](src/movement/ik.rs) - Calculate joint angles for position

### Servo Control

- [`ServoController::set_servo_angle(leg, part, angle)`](src/devices/servos.rs) - Set individual servo angle
- [`ServoController::set_leg_angles(leg, angles)`](src/devices/servos.rs) - Set all three servos for a leg
- [`ServoController::set_all_legs_to_angles(coxa, femur, tibia)`](src/devices/servos.rs) - Set all legs to same angles

### Audio System

- [`TtsEngine::say(text, voice)`](src/audio/tts.rs) - Generate speech with caching
- [`play_wav(file_path)`](src/audio/play.rs) - Play WAV file using aplay
- [`play_mp3(file_path)`](src/audio/play.rs) - Play MP3 file (converts to WAV)
- [`spawn_voice_server(voice_model)`](src/audio/piper_server.rs) - Start Piper TTS server
- [`ensure_voice_server_running(url, model)`](src/audio/piper_server.rs) - Start server if not running

## Development

### Task Runner Commands

See [Makefile.toml](Makefile.toml) for all available commands:

```bash
cargo make pibuild          # Build for Pi
cargo make pcbuild          # Build for PC
cargo make pcrun            # Run on PC
cargo make pibuildrelease   # Release build for Pi
cargo make pirunremote      # Deploy and run remotely
```

### Cross-Compilation

The project uses [cross](https://github.com/cross-rs/cross) for ARM64 cross-compilation. Configuration in [Cross.toml](Cross.toml).

## Troubleshooting

### Servo Issues
- Verify I2C is enabled: `sudo i2cdetect -y 1`
- Check PCA9685 addresses appear at 0x40 and 0x41
- Ensure servos have adequate power supply (5V, 5A+ recommended)

### TTS Issues
- Check server is running: [`is_server_running("http://127.0.0.1:5000")`](src/audio/piper_server.rs)
- View server logs: `tail -f /tmp/piper_tts.log`
- Verify voice models exist in `PIPER_VOICES_PATH`

### Audio Playback
- Ensure `aplay` is installed: `sudo apt-get install alsa-utils`
- For MP3 support, install ffmpeg: `sudo apt-get install ffmpeg`
- Run with sudo if permission issues occur

## License

[Add your license here]

## Authors
- Adam
- Michael

## Acknowledgments

- [Piper TTS](https://github.com/rhasspy/piper) for text-to-speech
- [pwm-pca9685](https://crates.io/crates/pwm-pca9685) for servo driver
- [glam](https://crates.io/crates/glam) for 3D math
