# LD19 LiDAR Driver

A modern, safe, and idiomatic Rust implementation of the LDROBOT LD19 LiDAR driver.

## Features

- **Full Protocol Support**: Implements the complete LD19 data protocol with CRC validation
- **Async Data Processing**: Non-blocking serial communication with background processing
- **Smart Filtering**: Near-range filtering algorithm removes unreliable measurements
- **Point Cloud Assembly**: Automatic assembly of 360° point cloud data
- **Feature Flags**: Supports both real hardware and dummy mode for testing
- **Thread Safe**: Uses Arc and Mutex for safe concurrent access
- **Zero Copy Parsing**: Efficient byte-by-byte state machine parser

## Architecture

The driver is split into several focused modules:

- **`mod.rs`**: Main driver interface and thread management
- **`point.rs`**: Point and PointCloud data structures
- **`packet.rs`**: Low-level packet format and CRC validation
- **`parser.rs`**: State machine for packet parsing and frame assembly
- **`filter.rs`**: Near-range filtering algorithm
- **`serial.rs`**: Serial port abstraction with dummy/real implementations

## Usage

### Basic Example

```rust
use devices::lidar::LidarDriver;
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // Create and start the driver
    let mut driver = LidarDriver::new("/dev/ttyUSB0")?;
    driver.start()?;

    // Read frames
    loop {
        if driver.is_frame_ready() {
            if let Some(cloud) = driver.get_point_cloud() {
                println!("Got {} points at {:.2} Hz", 
                    cloud.points.len(), 
                    cloud.frequency());
                
                // Process point cloud...
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}
```

### Obstacle Detection

```rust
// Find closest obstacle in front (±15 degrees)
if let Some(cloud) = driver.get_point_cloud() {
    let closest = cloud.closest_in_direction(0.0, 15.0);
    if let Some(point) = closest {
        println!("Obstacle at {} mm", point.distance);
    }
}
```

### Point Cloud Analysis

```rust
// Get all valid points
let valid_points: Vec<_> = cloud.valid_points().collect();

// Get points in specific angular range
let front_points = cloud.points_in_range(0.0, 90.0);

// Calculate statistics
let avg_distance: f32 = cloud.valid_points()
    .map(|p| p.distance as f32)
    .sum::<f32>() / cloud.valid_count() as f32;
```

## Protocol Details

### Packet Format

Each LiDAR packet is 47 bytes:

```
[Header] [Ver/Len] [Speed] [Start Angle] [12 Points] [End Angle] [Timestamp] [CRC]
  1B        1B       2B         2B           36B          2B          2B       1B
```

- **Header**: `0x54` (constant)
- **Ver/Len**: `0x2C` (constant)
- **Speed**: Rotation speed in degrees per second
- **Start/End Angle**: In hundredths of degrees (0-35999)
- **Points**: 12 measurements, each with distance (2B) + intensity (1B)
- **Timestamp**: Milliseconds
- **CRC**: CRC-8 checksum

### Serial Settings

- **Baud Rate**: 230400
- **Data Bits**: 8
- **Stop Bits**: 1
- **Parity**: None
- **Flow Control**: None

## Filtering Algorithm

The near-range filter removes unreliable close-range measurements using:

1. **Distance Threshold**: Points < 5000mm are candidates for filtering
2. **Point Grouping**: Nearby points (angle and distance) are grouped
3. **Intensity Validation**: Low-intensity groups are filtered out
   - Groups < 3 points need intensity ≥ 220
   - Groups ≥ 3 points need average intensity ≥ 15
4. **Wraparound Handling**: Groups at 0°/360° boundary are merged

This algorithm significantly improves data quality in cluttered environments.

## Performance

- **Scan Rate**: ~10 Hz (configurable by LiDAR)
- **Point Rate**: ~4500 points/second
- **Data Rate**: ~230400 baud
- **CPU Usage**: Minimal (background thread handles I/O)
- **Memory**: ~100KB for buffers

## Hardware Setup

### Wiring

Connect the LD19 LiDAR to your Raspberry Pi:

```
LiDAR   ->  Raspberry Pi
VCC     ->  5V (Pin 2 or 4)
GND     ->  GND (Pin 6)
TX      ->  RX (GPIO 15, Pin 10)
RX      ->  TX (GPIO 14, Pin 8) [optional, for commands]
```

### Enable Serial Port

On Raspberry Pi, enable the serial port in `/boot/config.txt`:

```
enable_uart=1
```

And disable serial console in `/boot/cmdline.txt` (remove `console=serial0,115200`).

### Permissions

Add your user to the `dialout` group:

```bash
sudo usermod -a -G dialout $USER
```

## Testing

Run the included example:

```bash
# With dummy features (no hardware)
cargo run --example lidar_test --features dummy

# With real hardware
cargo run --example lidar_test --features real
```

## Comparison with C++ SDK

This Rust implementation improves on the original C++ SDK in several ways:

| Feature | C++ SDK | Rust Implementation |
|---------|---------|---------------------|
| Memory Safety | Manual management | Automatic with ownership |
| Thread Safety | Mutex + raw pointers | Arc + Mutex types |
| Error Handling | Return codes | Result<T, E> |
| State Machine | Static variables | Clean enum-based FSM |
| Filtering | Mutable iteration | Functional iteration |
| API Design | Callbacks | Direct data access |
| Testing | Limited | Comprehensive unit tests |

## Troubleshooting

### No Data Received

- Check serial port name (usually `/dev/ttyUSB0` or `/dev/ttyAMA0`)
- Verify baud rate is 230400
- Ensure user has permission to access serial port
- Check LiDAR power supply (needs stable 5V)

### High Error Count

- Check for loose connections
- Verify LiDAR is spinning freely
- Ensure adequate power supply
- Check for electromagnetic interference

### Inconsistent Readings

- Near-range filtering is working as intended
- Low-intensity points are filtered out
- Reflective surfaces may cause issues

## Future Enhancements

- [ ] Command interface for LiDAR control (start/stop motor)
- [ ] Point cloud serialization (ROS messages, etc.)
- [ ] Real-time visualization
- [ ] Multi-LiDAR support
- [ ] Advanced filtering options
- [ ] Performance profiling tools

## License

This implementation is inspired by the LDROBOT C++ SDK (MIT License).

## References

- [LDROBOT Official SDK](https://github.com/ldrobotSensorTeam/ldlidar_stl_sdk)
- [LD19 Product Page](https://www.ldrobot.com/product/en/126)
