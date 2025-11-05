# Servo Patch Installation Guide

## Quick Install on Raspberry Pi

### 1. Build the Binary

On your development machine:
```bash
cd /media/diskD/MyProjects/HEXAPOD/hexapod-code/crates/devices
cross build --target aarch64-unknown-linux-gnu --example servo_patch --features real --release
```

The binary will be at:
```
../../target/aarch64-unknown-linux-gnu/release/examples/servo_patch
```

### 2. Copy to Raspberry Pi

```bash
scp ../../target/aarch64-unknown-linux-gnu/release/examples/servo_patch pi@hexapod:/tmp/
```

### 3. Install on Raspberry Pi

SSH into the Pi:
```bash
ssh pi@hexapod
```

Install the binary:
```bash
sudo mv /tmp/servo_patch /usr/local/bin/
sudo chmod +x /usr/local/bin/servo_patch
```

Test it:
```bash
sudo /usr/local/bin/servo_patch
```

### 4. Set up Systemd Service (Optional)

Copy the service file to the Pi:
```bash
# On dev machine:
scp examples/servo-patch.service pi@hexapod:/tmp/

# On Pi:
sudo mv /tmp/servo-patch.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable servo-patch.service
```

Edit configuration if needed:
```bash
sudo systemctl edit servo-patch.service
```

Add environment variables:
```ini
[Service]
Environment="SERVO_PATCH_ANGLE=85"
Environment="SERVO_PATCH_DELAY_MS=150"
```

Start the service:
```bash
sudo systemctl start servo-patch.service
```

Check status:
```bash
sudo systemctl status servo-patch.service
journalctl -u servo-patch.service
```

### 5. Reboot Test

Reboot the Pi to ensure servos are set on boot:
```bash
sudo reboot
```

After reboot, check the service ran:
```bash
sudo systemctl status servo-patch.service
```

## Manual Configuration

If you don't want to use systemd, you can run it manually on each boot or add it to `/etc/rc.local`:

```bash
# Add before "exit 0" in /etc/rc.local
/usr/local/bin/servo_patch &
```

## Troubleshooting

### I2C Permissions
If you get permission errors, ensure I2C is enabled and your user is in the `i2c` group:
```bash
sudo usermod -a -G i2c $USER
sudo chmod 666 /dev/i2c-1
```

### Check I2C Devices
```bash
sudo i2cdetect -y 1
```

You should see devices at addresses 0x40 and 0x41.

### View Logs
```bash
journalctl -u servo-patch.service -f
```

### Test Different Angles
```bash
sudo SERVO_PATCH_ANGLE=80 /usr/local/bin/servo_patch
sudo SERVO_PATCH_ANGLE=100 /usr/local/bin/servo_patch
```

## Uninstall

Disable and remove the service:
```bash
sudo systemctl stop servo-patch.service
sudo systemctl disable servo-patch.service
sudo rm /etc/systemd/system/servo-patch.service
sudo systemctl daemon-reload
```

Remove the binary:
```bash
sudo rm /usr/local/bin/servo_patch
```
