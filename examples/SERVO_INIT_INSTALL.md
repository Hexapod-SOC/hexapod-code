# Servo Init Installation Guide

This is the hexapod-aware version of servo_patch that uses the project's config.

## Differences from servo_patch

- **Uses hexapod config**: Reads `SERVO_PINS` and `SERVO_OFFSETS` from `src/config.rs`
- **Feature-aware**: Works with both `--features real` and `--features dummy`
- **Better integration**: Part of the main examples, easier to maintain
- **Environment variables**: Uses `SERVO_INIT_*` prefix instead of `SERVO_PATCH_*`

## Quick Install on Raspberry Pi

### 1. Build the Binary

On your development machine:
```bash
cd /media/diskD/MyProjects/HEXAPOD/hexapod-code
cross build --target aarch64-unknown-linux-gnu --example servo_init --features real --release
```

The binary will be at:
```
target/aarch64-unknown-linux-gnu/release/examples/servo_init
```

### 2. Copy to Raspberry Pi

```bash
scp target/aarch64-unknown-linux-gnu/release/examples/servo_init pi@hexapod:/tmp/
```

### 3. Install on Raspberry Pi

SSH into the Pi:
```bash
ssh pi@hexapod
```

Install the binary:
```bash
sudo mv /tmp/servo_init /usr/local/bin/
sudo chmod +x /usr/local/bin/servo_init
```

Test it:
```bash
sudo /usr/local/bin/servo_init
```

### 4. Set up Systemd Service

Create service file `/etc/systemd/system/servo-init.service`:
```ini
[Unit]
Description=Hexapod Servo Initialization
After=network.target
Before=hexapod.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/servo_init
RemainAfterExit=yes

# Configuration (optional - defaults shown)
Environment="SERVO_INIT_ANGLE=90"
Environment="SERVO_INIT_LEFT_ADDR=0x40"
Environment="SERVO_INIT_RIGHT_ADDR=0x41"
Environment="SERVO_INIT_ENABLE_LEFT=true"
Environment="SERVO_INIT_ENABLE_RIGHT=true"
Environment="SERVO_INIT_DELAY_MS=100"

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable servo-init.service
sudo systemctl start servo-init.service
```

Check status:
```bash
sudo systemctl status servo-init.service
journalctl -u servo-init.service
```

### 5. Reboot Test

```bash
sudo reboot
```

After reboot:
```bash
sudo systemctl status servo-init.service
```

## Configuration

### Environment Variables

All configuration is done via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVO_INIT_ANGLE` | `90.0` | Default servo angle (0-180°) |
| `SERVO_INIT_PWM` | calculated | Direct PWM value (overrides angle) |
| `SERVO_INIT_LEFT_ADDR` | `0x40` | I2C address of left PCA9685 board |
| `SERVO_INIT_RIGHT_ADDR` | `0x41` | I2C address of right PCA9685 board |
| `SERVO_INIT_ENABLE_LEFT` | `true` | Enable left board initialization |
| `SERVO_INIT_ENABLE_RIGHT` | `true` | Enable right board initialization |
| `SERVO_INIT_DELAY_MS` | `100` | Delay between board initializations |

### Edit Service Configuration

```bash
sudo systemctl edit servo-init.service
```

Add overrides:
```ini
[Service]
Environment="SERVO_INIT_ANGLE=85"
Environment="SERVO_INIT_DELAY_MS=150"
```

Apply changes:
```bash
sudo systemctl daemon-reload
sudo systemctl restart servo-init.service
```

## Testing Locally (Dummy Mode)

You can test the tool locally without hardware:

```bash
cd /media/diskD/MyProjects/HEXAPOD/hexapod-code
cargo run --example servo_init --features dummy
```

This will show the configuration without accessing any hardware.

## Manual Testing on Pi

Test different angles:
```bash
sudo SERVO_INIT_ANGLE=80 /usr/local/bin/servo_init
sudo SERVO_INIT_ANGLE=100 /usr/local/bin/servo_init
```

Test only one board:
```bash
sudo SERVO_INIT_ENABLE_RIGHT=false /usr/local/bin/servo_init
```

## Troubleshooting

### Check I2C Devices
```bash
sudo i2cdetect -y 1
```

Should show devices at 0x40 and 0x41.

### View Detailed Logs
```bash
journalctl -u servo-init.service -f
```

### I2C Permissions
```bash
sudo usermod -a -G i2c $USER
sudo chmod 666 /dev/i2c-1
```

### Compare with servo_patch

This tool provides the same functionality as `servo_patch` but:
- Reads from hexapod config (servo pins, offsets)
- Uses consistent environment variable naming
- Better integration with main project
- Supports dummy mode for testing

## Uninstall

```bash
sudo systemctl stop servo-init.service
sudo systemctl disable servo-init.service
sudo rm /etc/systemd/system/servo-init.service
sudo rm /usr/local/bin/servo_init
sudo systemctl daemon-reload
```

## Integration with Hexapod Service

If you have a main hexapod service, you can ensure servo_init runs first:

In `/etc/systemd/system/hexapod.service`, add:
```ini
[Unit]
After=servo-init.service
Requires=servo-init.service
```

This ensures servos are in safe position before the main hexapod code runs.
