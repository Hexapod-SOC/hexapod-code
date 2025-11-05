# Servo Calibration Workflow

## Quick Start

1. **Build the tool** (on Raspberry Pi or cross-compile):
   ```bash
   cargo build --example servo_center --features real
   ```

2. **Run the tool**:
   ```bash
   cargo run --example servo_center --features real
   ```

3. **Follow the prompts**:
   - Enter PCA9685 address (0x40 or 0x41)
   - Enter servo pin number (0-15)

4. **Calibrate each servo**:
   - Use `-`, `--`, `---` to move left
   - Use `+`, `++`, `+++` to move right
   - Find the center position where the servo is at 90°
   - Record the displayed angle in your spreadsheet

## Spreadsheet Template

Create a Google Sheet with these columns:

| Board | Pin | Leg Part | Expected PWM | Actual PWM | Offset | Notes |
|-------|-----|----------|--------------|------------|--------|-------|
| 0x40  | 0   | LF Coxa  | 369          | 358        | -11    | Slightly left |
| 0x40  | 1   | LF Femur | 369          | 373        | +4     | Slightly right |
| 0x40  | 2   | LF Tibia | 369          | 367        | -2     | Good |
| ...   | ... | ...      | ...          | ...        | ...    | ... |

**Important:** Record **PWM values**, not angles! PWM values are more precise and avoid rounding errors.

## Pin Mapping Reference

Left PCA (0x40):
- Pins 0-2: Left Front (Coxa, Femur, Tibia)
- Pins 3-5: Left Middle (Coxa, Femur, Tibia)
- Pins 6-8: Left Back (Coxa, Femur, Tibia)

Right PCA (0x41):
- Pins 0-2: Right Front (Coxa, Femur, Tibia)
- Pins 3-5: Right Middle (Coxa, Femur, Tibia)
- Pins 6-8: Right Back (Coxa, Femur, Tibia)

## Tips

- Start with large movements (`-` or `+`) to get close
- Use fine movements (`--` or `++`) to dial it in
- Use micro movements (`---` or `+++`) for precision
- Look at the servo horn position to verify center
- Some servos may have ±10-15 PWM units offset from center (369)
- **Always record PWM values**, not angles
- PWM 369 ≈ 90° center position (1.5ms pulse width)
- Record all offsets for future calibration code
