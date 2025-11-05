# Devices Examples

## Servo Patch (`servo_patch.rs`)

A boot-time utility that sets all servos to a safe position on system startup.

### Purpose

Prevents MG996R servos from overheating at null angle (0°) by immediately setting them to a safe position (default: 90°) when the system boots. This tool is designed to run as a systemd service or startup script.

### Usage

Run the example with the `real` feature enabled:

```bash
cargo run --example servo_patch --features real
```

Or cross-compile for Raspberry Pi:

```bash
cross build --target aarch64-unknown-linux-gnu --example servo_patch --features real --release
```

### Environment Variables

Configure the tool using environment variables:

- `SERVO_PATCH_ANGLE` - Target angle in degrees (0-180), default: `90.0`
- `SERVO_PATCH_PWM` - Direct PWM value (246-492), overrides angle, default: calculated from angle
- `SERVO_PATCH_LEFT_ADDR` - Left PCA9685 I2C address, default: `0x40`
- `SERVO_PATCH_RIGHT_ADDR` - Right PCA9685 I2C address, default: `0x41`
- `SERVO_PATCH_ENABLE_LEFT` - Enable left board (`true`/`1`), default: `true`
- `SERVO_PATCH_ENABLE_RIGHT` - Enable right board (`true`/`1`), default: `true`
- `SERVO_PATCH_DELAY_MS` - Delay between boards in milliseconds, default: `100`

### Examples

Set to 90° (default):
```bash
./servo_patch
```

Set to specific angle:
```bash
SERVO_PATCH_ANGLE=80 ./servo_patch
```

Set to specific PWM value:
```bash
SERVO_PATCH_PWM=369 ./servo_patch
```

Only configure left board:
```bash
SERVO_PATCH_ENABLE_RIGHT=false ./servo_patch
```

Custom addresses:
```bash
SERVO_PATCH_LEFT_ADDR=0x42 SERVO_PATCH_RIGHT_ADDR=0x43 ./servo_patch
```

### Systemd Service Setup

Create `/etc/systemd/system/servo-patch.service`:

```ini
[Unit]
Description=Servo Patch - Initialize servos on boot
After=local-fs.target
Before=hexapod.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/servo_patch
Environment="SERVO_PATCH_ANGLE=90"
Environment="SERVO_PATCH_DELAY_MS=100"
RemainAfterExit=yes
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable servo-patch.service
sudo systemctl start servo-patch.service
```

Check status:
```bash
sudo systemctl status servo-patch.service
```

---

## Servo Centering Tool (`servo_center.rs`)

A CLI tool for manually centering and calibrating servos connected to PCA9685 boards.

### Purpose

This tool helps you find the precise center position (90°) for each servo by allowing fine-grained adjustments. You can then record the actual angle values in a spreadsheet for servo calibration.

### Usage

Run the example with the `real` feature enabled:

```bash
cargo run --example servo_center --features real
```

Or if cross-compiling for the Raspberry Pi:

```bash
cross build --target aarch64-unknown-linux-gnu --example servo_center --features real
```

### Interactive Commands

When the tool starts, it will ask for:
1. **PCA9685 I2C address** (0x40 or 0x41) - defaults to 0x40
2. **Pin number** (0-15) - the servo pin you want to calibrate

Then you can use these commands:

#### Movement Commands
- `---` - Move left by 1 PWM unit (micro adjustment)
- `--` - Move left by 5 PWM units (fine adjustment)
- `-` - Move left by 20 PWM units (large adjustment)
- `+++` - Move right by 1 PWM unit (micro adjustment)
- `++` - Move right by 5 PWM units (fine adjustment)
- `+` - Move right by 20 PWM units (large adjustment)

#### Control Commands
- `c` - Center servo at PWM 369 (~90°)
- `s [pwm]` - Set specific PWM value (205-533 extended), e.g., `s 350`
- `p [pin]` - Switch to different pin (0-15), e.g., `p 5`
- `b [addr]` - Switch PCA board address, e.g., `b 0x41`
- `h` - Show help
- `q` - Quit and display final position summary

**Note:** This tool works directly with PWM values (not angles) to avoid conversion rounding errors.

**PWM Ranges:**
- **Standard:** 246-492 (0° to 180°) - Safe for most servos
- **Extended:** 205-533 (~-30° to ~210°) - For testing mechanical limits
- **Center:** 369 (~90°)

⚠️ **Warning:** Extended range may damage servos if they hit mechanical stops. Use with caution and monitor servo behavior!

### Example Workflow

1. Start the tool and select your PCA board and pin
2. Use large adjustments (`-` or `+`) to get close to center
3. Use fine adjustments (`--` or `++`) to dial it in
4. Use micro adjustments (`---` or `+++`) for precision
5. When the servo is perfectly centered, note the **PWM value** displayed
6. Type `q` to quit and see the final summary
7. **Record the PWM value** in your calibration spreadsheet (not the angle!)

### Display

The tool displays:
```
📍 Current Position:
   Board: 0x40 | Pin: 3 | PWM: 358 | ~86.9° [✓]

Command: 
```

When in extended range:
```
📍 Current Position:
   Board: 0x40 | Pin: 3 | PWM: 220 | ~-19.5° [⚠️ EXTENDED]

Command:
```

### Notes

- The servo will move immediately after each command
- **Works with raw PWM values** to avoid rounding errors from angle conversions
- PWM values are clamped to 246-492 (MG996R servos at 60Hz)
- Angles shown are approximate, for reference only
- **Always record the PWM value**, not the angle
- Use this to find the "true center" for each servo, as manufacturing tolerances vary
