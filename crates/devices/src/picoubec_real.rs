use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::time::Instant;

/// Battery and power status information
#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    pub voltage: f32,
    pub current: f32,
    pub last_update: Option<Instant>,
}

impl Default for BatteryStatus {
    fn default() -> Self {
        Self {
            voltage: 0.0,
            current: 0.0,
            last_update: None,
        }
    }
}

/// Power system state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerState {
    Normal,
    LowBatteryWarning { timeout_seconds: u32 },
    Critical,
    ShuttingDown { remaining_seconds: u32 },
}

pub struct PicoUbecController {
    reader: Option<BufReader<File>>,
    writer: Option<File>,
    battery_status: BatteryStatus,
    power_state: PowerState,
    connection_failed: bool,
}

impl PicoUbecController {
    pub fn new(port_path: &str) -> Self {
        // Try to open the serial port with raw file I/O
        let (reader, writer, failed) = match Self::open_serial_port(port_path) {
            Ok((r, w)) => {
                println!("UBEC controller connected on {}", port_path);
                (Some(r), Some(w), false)
            }
            Err(e) => {
                eprintln!("Warning: Failed to open UBEC serial port {}: {}", port_path, e);
                eprintln!("Continuing without battery monitoring (device will auto-disconnect if needed)");
                (None, None, true)
            }
        };

        Self {
            reader,
            writer,
            battery_status: BatteryStatus::default(),
            power_state: PowerState::Normal,
            connection_failed: failed,
        }
    }

    /// Open and configure a serial port using raw file I/O
    fn open_serial_port(port_path: &str) -> std::io::Result<(BufReader<File>, File)> {
        use std::os::unix::io::AsRawFd;

        // Open for reading and writing
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(port_path)?;

        let fd = file.as_raw_fd();

        // Configure serial port using termios
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            
            if libc::tcgetattr(fd, &mut termios) != 0 {
                return Err(std::io::Error::last_os_error());
            }

            // Set baud rate to 115200
            libc::cfsetispeed(&mut termios, libc::B115200);
            libc::cfsetospeed(&mut termios, libc::B115200);

            // 8N1 mode
            termios.c_cflag &= !libc::PARENB; // No parity
            termios.c_cflag &= !libc::CSTOPB; // 1 stop bit
            termios.c_cflag &= !libc::CSIZE;
            termios.c_cflag |= libc::CS8; // 8 bits

            // Enable receiver, ignore modem control lines
            termios.c_cflag |= libc::CREAD | libc::CLOCAL;

            // Raw mode
            termios.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHOE | libc::ISIG);
            termios.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
            termios.c_oflag &= !libc::OPOST;

            // Non-blocking read with timeout
            termios.c_cc[libc::VMIN] = 0;
            termios.c_cc[libc::VTIME] = 1; // 0.1 second timeout

            if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0 {
                return Err(std::io::Error::last_os_error());
            }

            // Flush any existing data
            libc::tcflush(fd, libc::TCIOFLUSH);
        }

        // Clone file descriptor for writer
        let writer = file.try_clone()?;
        let reader = BufReader::new(file);

        Ok((reader, writer))
    }

    /// Update battery status by reading from UART
    /// Returns true if new data was received, false if no data or error
    pub fn update(&mut self) -> bool {
        // Skip if connection failed at initialization
        if self.connection_failed {
            return false;
        }

        let reader = match self.reader.as_mut() {
            Some(r) => r,
            None => return false,
        };

        // Try to read a line, but don't block if no data is available
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => false, // No data available
            Ok(_) => {
                // Parse the message
                self.parse_message(&line);
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available (non-blocking mode)
                false
            }
            Err(e) => {
                // Log error but continue operation
                eprintln!("UART read error (continuing without battery data): {}", e);
                false
            }
        }
    }

    /// Parse a UART message according to the protocol specification
    fn parse_message(&mut self, msg: &str) {
        let msg = msg.trim_end_matches("\r\n").trim_end_matches('\n');
        let parts: Vec<&str> = msg.split(':').collect();

        match parts.as_slice() {
            // Startup
            ["START"] => {
                println!("[UBEC] Device initialized");
            }

            // Data telemetry
            ["DATA", "VOLTAGE", v] => {
                if let Ok(voltage) = v.parse::<f32>() {
                    self.battery_status.voltage = voltage;
                    self.battery_status.last_update = Some(Instant::now());
                }
            }
            ["DATA", "CURRENT", c] => {
                if let Ok(current) = c.parse::<f32>() {
                    self.battery_status.current = current;
                    self.battery_status.last_update = Some(Instant::now());
                }
            }

            // Warnings
            ["WARNING", "LOW_BATTERY_START", v, t] => {
                if let (Ok(voltage), Ok(timeout)) = (v.parse::<f32>(), t.parse::<u32>()) {
                    println!(
                        "[UBEC WARNING] Low battery! {:.3}V - Disconnecting in {}s",
                        voltage, timeout
                    );
                    self.power_state = PowerState::LowBatteryWarning {
                        timeout_seconds: timeout,
                    };
                }
            }
            ["WARNING", "LOW_BATTERY_COUNTDOWN", v, r] => {
                if let (Ok(voltage), Ok(remaining)) = (v.parse::<f32>(), r.parse::<u32>()) {
                    println!(
                        "[UBEC WARNING] Countdown: {}s remaining (Voltage: {:.3}V)",
                        remaining, voltage
                    );
                    self.power_state = PowerState::LowBatteryWarning {
                        timeout_seconds: remaining,
                    };
                }
            }

            // Critical events
            ["CRITICAL", "VOLTAGE_LOW", v, t] => {
                if let (Ok(voltage), Ok(threshold)) = (v.parse::<f32>(), t.parse::<f32>()) {
                    println!(
                        "[UBEC CRITICAL] Voltage {:.3}V below threshold {:.3}V - RELAY OFF",
                        voltage, threshold
                    );
                    self.power_state = PowerState::Critical;
                }
            }
            ["CRITICAL", "LOW_VOLTAGE_TIMEOUT", v] => {
                if let Ok(voltage) = v.parse::<f32>() {
                    println!(
                        "[UBEC CRITICAL] Low voltage timeout! Final voltage: {:.3}V - RELAY OFF",
                        voltage
                    );
                    self.power_state = PowerState::Critical;
                }
            }

            // Info
            ["INFO", "VOLTAGE_RECOVERED", v] => {
                if let Ok(voltage) = v.parse::<f32>() {
                    println!("[UBEC INFO] Battery voltage recovered: {:.3}V", voltage);
                    self.power_state = PowerState::Normal;
                }
            }

            // Errors
            ["ERROR", "ADC_TIMEOUT", c] => {
                if let Ok(count) = c.parse::<u32>() {
                    println!(
                        "[UBEC ERROR] ADC timeout after {} failed reads - RELAY OFF",
                        count
                    );
                    self.power_state = PowerState::Critical;
                }
            }

            // Commands
            ["CMD", "SHUTDOWN_SCHEDULED", d] => {
                if let Ok(delay) = d.parse::<u32>() {
                    println!("[UBEC CMD] Shutdown scheduled in {} seconds", delay);
                    self.power_state = PowerState::ShuttingDown {
                        remaining_seconds: delay,
                    };
                }
            }
            ["CMD", "SHUTDOWN_COUNTDOWN", r] => {
                if let Ok(remaining) = r.parse::<u32>() {
                    println!("[UBEC CMD] Shutdown in {} seconds", remaining);
                    self.power_state = PowerState::ShuttingDown {
                        remaining_seconds: remaining,
                    };
                }
            }
            ["CMD", "SHUTDOWN_EXECUTED"] => {
                println!("[UBEC CMD] Shutdown executed - RELAY OFF");
                self.power_state = PowerState::Critical;
            }

            // Unknown message
            _ => {
                if !msg.is_empty() {
                    eprintln!("[UBEC] Unknown message: {}", msg);
                }
            }
        }
    }

    /// Get current battery status
    pub fn get_battery_status(&self) -> BatteryStatus {
        self.battery_status
    }

    /// Get current power state
    pub fn get_power_state(&self) -> PowerState {
        self.power_state
    }

    /// Check if battery voltage is critically low
    pub fn is_critical(&self) -> bool {
        matches!(self.power_state, PowerState::Critical)
    }

    /// Send shutdown command to UBEC
    pub fn send_shutdown(&mut self, delay_seconds: u32) {
        if self.connection_failed {
            eprintln!("Cannot send shutdown: UART not connected");
            return;
        }

        if let Some(writer) = self.writer.as_mut() {
            let cmd = format!("SHUTDOWN:{}\n", delay_seconds);
            if let Err(e) = writer.write_all(cmd.as_bytes()) {
                eprintln!("Failed to send shutdown command: {}", e);
            } else {
                println!("Shutdown command sent: {} seconds delay", delay_seconds);
            }
        }
    }

    /// Check if UART connection is available
    pub fn is_connected(&self) -> bool {
        !self.connection_failed && self.reader.is_some()
    }
}
