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
    port_path: String,
    battery_status: BatteryStatus,
    power_state: PowerState,
    connection_failed: bool,
    raw_log: bool,
    unknown_log: bool,
}

impl PicoUbecController {
    pub fn new(port_path: &str) -> Self {
        let raw_log = std::env::var("UBEC_RAW_LOG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let unknown_log = std::env::var("UBEC_UNKNOWN_LOG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        // Try to open the serial port with raw file I/O
        let (reader, writer, failed) = match Self::open_serial_port(port_path) {
            Ok((r, w)) => {
                println!("UBEC controller connected on {}", port_path);
                (Some(r), Some(w), false)
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to open UBEC serial port {}: {}",
                    port_path, e
                );
                eprintln!(
                    "Continuing without battery monitoring (device will auto-disconnect if needed)"
                );
                (None, None, true)
            }
        };

        Self {
            reader,
            writer,
            port_path: port_path.to_string(),
            battery_status: BatteryStatus::default(),
            power_state: PowerState::Normal,
            connection_failed: failed,
            raw_log,
            unknown_log,
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

    /// Update battery status by reading from UART.
    /// Drains all currently available lines so cached values stay fresh.
    /// Returns true if at least one new line was received.
    pub fn update(&mut self) -> bool {
        // Skip if connection failed at initialization
        if self.connection_failed {
            return false;
        }

        // Collect all pending lines first (while holding the reader borrow),
        // then release the borrow before calling parse_message (which needs &mut self).
        let lines: Vec<String> = {
            let reader = match self.reader.as_mut() {
                Some(r) => r,
                None => return false,
            };

            let mut collected = Vec::new();
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => collected.push(line),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        eprintln!("UART read error (continuing without battery data): {}", e);
                        break;
                    }
                }
            }
            collected
        }; // reader borrow ends here

        let got_data = !lines.is_empty();
        for line in lines {
            if self.raw_log {
                eprintln!("[UBEC RAW] {:?}", line.trim_end());
            }
            self.parse_message(&line);
        }
        got_data
    }

    /// Parse a UART message according to the protocol specification
    fn parse_message(&mut self, msg: &str) {
        // Use .trim() so CR+LF, bare CR, bare LF and trailing spaces are all stripped.
        // The previous trim_end_matches chain left '\r' inside value tokens (e.g. "7.40\r")
        // which caused f32::parse to silently fail and the voltage to never update.
        let msg = msg.trim();
        let parts: Vec<&str> = msg.split(':').collect();

        match parts.as_slice() {
            // Startup
            ["START"] => {
                println!("[UBEC] Device initialized");
            }

            // Data telemetry
            ["DATA", "VOLTAGE", v] => {
                if let Ok(voltage) = v.parse::<f32>() {
                    if (voltage - self.battery_status.voltage).abs() > 0.001 {
                        eprintln!("[UBEC] Voltage updated: {:.3}V → {:.3}V", self.battery_status.voltage, voltage);
                    }
                    self.battery_status.voltage = voltage;
                    self.battery_status.last_update = Some(Instant::now());
                } else {
                    eprintln!("[UBEC] Failed to parse voltage token: {:?}", v);
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
                if self.unknown_log && !msg.is_empty() {
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

    /// Send a command over UART, with fallback direct write if stored fd fails
    fn send_command(&mut self, cmd: &str, label: &str) {
        if self.connection_failed {
            eprintln!("Cannot send {}: UART not connected", label);
            return;
        }

        // First attempt: use stored writer
        if let Some(writer) = self.writer.as_mut() {
            if writer.write_all(cmd.as_bytes()).is_ok() {
                let _ = writer.flush();
                println!("{} sent", label);
                return;
            }
        }

        // Stored fd is dead — open a fresh write-only fd (skips termios, device keeps config from startup)
        eprintln!("[UBEC] Stored fd failed, direct write to {}...", self.port_path);
        match OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(&self.port_path)
        {
            Ok(mut f) => {
                match f.write_all(cmd.as_bytes()) {
                    Ok(_) => {
                        let _ = f.flush();
                        // Replace dead writer with new one
                        self.writer = Some(f);
                        println!("{} sent (via new fd)", label);
                    }
                    Err(e) => {
                        eprintln!("[UBEC] Direct write also failed: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[UBEC] Cannot open {} for write: {}", self.port_path, e);
            }
        }
    }

    /// Send shutdown command to UBEC
    pub fn send_shutdown(&mut self, delay_seconds: u32) {
        let cmd = format!("SHUTDOWN:{}\n", delay_seconds);
        self.send_command(&cmd, &format!("Shutdown ({}s delay)", delay_seconds));
    }

    /// Check if UART connection is available
    pub fn is_connected(&self) -> bool {
        !self.connection_failed && self.reader.is_some()
    }

    /// Enable servos via UART command
    pub fn enable_servos(&mut self) {
        self.send_command("ENABLE_SERVOS\n", "Enable servos");
    }

    /// Disable servos via UART command
    pub fn disable_servos(&mut self) {
        self.send_command("DISABLE_SERVOS\n", "Disable servos");
    }
}
