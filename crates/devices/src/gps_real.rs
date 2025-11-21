use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::time::Instant;

/// GPS position data
#[derive(Debug, Clone, Copy)]
pub struct GpsPosition {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    pub speed_kmh: f32,
    pub heading: Option<f32>,
    pub satellites: u8,
    pub fix_quality: FixQuality,
    pub last_update: Option<Instant>,
}

impl Default for GpsPosition {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            altitude: 0.0,
            speed_kmh: 0.0,
            heading: None,
            satellites: 0,
            fix_quality: FixQuality::NoFix,
            last_update: None,
        }
    }
}

/// GPS fix quality indicator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FixQuality {
    NoFix,
    GpsFix,
    DifferentialFix,
    PpsFix,
    RtkFixed,
    RtkFloat,
    Estimated,
    Manual,
    Simulation,
}

impl From<u8> for FixQuality {
    fn from(val: u8) -> Self {
        match val {
            0 => FixQuality::NoFix,
            1 => FixQuality::GpsFix,
            2 => FixQuality::DifferentialFix,
            3 => FixQuality::PpsFix,
            4 => FixQuality::RtkFixed,
            5 => FixQuality::RtkFloat,
            6 => FixQuality::Estimated,
            7 => FixQuality::Manual,
            8 => FixQuality::Simulation,
            _ => FixQuality::NoFix,
        }
    }
}

pub struct GpsController {
    reader: Option<BufReader<File>>,
    position: GpsPosition,
    connection_failed: bool,
}

impl GpsController {
    pub fn new(port_path: &str) -> Self {
        // Try to open the serial port with raw file I/O
        let (reader, failed) = match Self::open_serial_port(port_path) {
            Ok(r) => {
                println!("GPS controller connected on {}", port_path);
                (Some(r), false)
            }
            Err(e) => {
                eprintln!("Warning: Failed to open GPS serial port {}: {}", port_path, e);
                eprintln!("Continuing without GPS (location data unavailable)");
                (None, true)
            }
        };

        Self {
            reader,
            position: GpsPosition::default(),
            connection_failed: failed,
        }
    }

    /// Open and configure a serial port using raw file I/O
    fn open_serial_port(port_path: &str) -> std::io::Result<BufReader<File>> {
        use std::os::unix::io::AsRawFd;

        // Open for reading
        let file = OpenOptions::new()
            .read(true)
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

        let reader = BufReader::new(file);
        Ok(reader)
    }

    /// Update GPS position by reading from UART
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
                // Parse the NMEA sentence
                self.parse_nmea(&line);
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available (non-blocking mode)
                false
            }
            Err(e) => {
                // Log error but continue operation
                eprintln!("GPS UART read error (continuing without GPS data): {}", e);
                false
            }
        }
    }

    /// Parse an NMEA sentence manually
    fn parse_nmea(&mut self, sentence: &str) {
        let sentence = sentence.trim();
        
        // Verify checksum
        if !Self::verify_checksum(sentence) {
            return;
        }

        // Split sentence
        let parts: Vec<&str> = sentence.split(',').collect();
        if parts.is_empty() {
            return;
        }

        let sentence_type = parts[0];

        match sentence_type {
            // GGA - Global Positioning System Fix Data
            s if s.ends_with("GGA") => {
                self.parse_gga(&parts);
            }
            // RMC - Recommended Minimum Navigation Information
            s if s.ends_with("RMC") => {
                self.parse_rmc(&parts);
            }
            // VTG - Track made good and Ground speed
            s if s.ends_with("VTG") => {
                self.parse_vtg(&parts);
            }
            _ => {
                // Ignore other sentence types for now
            }
        }
    }

    /// Verify NMEA checksum
    fn verify_checksum(sentence: &str) -> bool {
        if !sentence.starts_with('$') {
            return false;
        }

        // Find checksum delimiter
        if let Some(asterisk_pos) = sentence.rfind('*') {
            let data = &sentence[1..asterisk_pos];
            let checksum_str = &sentence[asterisk_pos + 1..];

            // Calculate checksum
            let mut checksum: u8 = 0;
            for byte in data.bytes() {
                checksum ^= byte;
            }

            // Compare with provided checksum
            if let Ok(expected) = u8::from_str_radix(checksum_str, 16) {
                return checksum == expected;
            }
        }

        false
    }

    /// Parse GGA sentence (position and fix data)
    fn parse_gga(&mut self, parts: &[&str]) {
        // $GNGGA,203030.00,4836.03803,N,01805.13318,E,1,32,0.5,208.29,M,42.98,M,,*77
        // 0: $GNGGA
        // 1: Time (hhmmss.ss)
        // 2: Latitude (ddmm.mmmmm)
        // 3: N/S
        // 4: Longitude (dddmm.mmmmm)
        // 5: E/W
        // 6: Fix quality
        // 7: Number of satellites
        // 8: HDOP
        // 9: Altitude
        // 10: M (altitude unit)
        // 11: Geoid separation
        // 12: M (geoid unit)
        
        if parts.len() < 10 {
            return;
        }

        // Parse latitude
        if let Ok(lat) = Self::parse_coordinate(parts[2], parts.get(3).copied()) {
            self.position.latitude = lat;
        }

        // Parse longitude
        if let Ok(lon) = Self::parse_coordinate(parts[4], parts.get(5).copied()) {
            self.position.longitude = lon;
        }

        // Fix quality
        if let Ok(quality) = parts[6].parse::<u8>() {
            self.position.fix_quality = FixQuality::from(quality);
        }

        // Number of satellites
        if let Ok(sats) = parts[7].parse::<u8>() {
            self.position.satellites = sats;
        }

        // Altitude
        if let Ok(alt) = parts[9].parse::<f32>() {
            self.position.altitude = alt;
        }

        self.position.last_update = Some(Instant::now());
    }

    /// Parse RMC sentence (speed and course)
    fn parse_rmc(&mut self, parts: &[&str]) {
        // $GNRMC,203030.00,A,4836.03803,N,01805.13318,E,0.18,,211125,,,A,V*2E
        // 0: $GNRMC
        // 1: Time
        // 2: Status (A=active, V=void)
        // 3: Latitude
        // 4: N/S
        // 5: Longitude
        // 6: E/W
        // 7: Speed over ground (knots)
        // 8: Track angle (degrees)
        // 9: Date
        
        if parts.len() < 9 {
            return;
        }

        // Status check
        if parts[2] != "A" {
            return; // Invalid fix
        }

        // Parse latitude
        if let Ok(lat) = Self::parse_coordinate(parts[3], parts.get(4).copied()) {
            self.position.latitude = lat;
        }

        // Parse longitude
        if let Ok(lon) = Self::parse_coordinate(parts[5], parts.get(6).copied()) {
            self.position.longitude = lon;
        }

        // Speed (convert knots to km/h)
        if let Ok(speed_knots) = parts[7].parse::<f32>() {
            self.position.speed_kmh = speed_knots * 1.852; // 1 knot = 1.852 km/h
        }

        // Track angle
        if !parts[8].is_empty() {
            if let Ok(heading) = parts[8].parse::<f32>() {
                self.position.heading = Some(heading);
            }
        }

        self.position.last_update = Some(Instant::now());
    }

    /// Parse VTG sentence (velocity and track)
    fn parse_vtg(&mut self, parts: &[&str]) {
        // $GNVTG,,,,,0.18,N,0.33,K,A*2D
        // 0: $GNVTG
        // 1: Track made good (degrees true)
        // 2: T
        // 3: Track made good (degrees magnetic)
        // 4: M
        // 5: Speed (knots)
        // 6: N
        // 7: Speed (km/h)
        // 8: K
        // 9: Mode
        
        if parts.len() < 9 {
            return;
        }

        // Speed in km/h
        if let Ok(speed) = parts[7].parse::<f32>() {
            self.position.speed_kmh = speed;
        }

        self.position.last_update = Some(Instant::now());
    }

    /// Parse NMEA coordinate format (ddmm.mmmmm) to decimal degrees
    fn parse_coordinate(coord_str: &str, direction: Option<&str>) -> Result<f64, &'static str> {
        if coord_str.is_empty() {
            return Err("Empty coordinate");
        }

        // Find decimal point
        let dot_pos = coord_str.find('.').ok_or("No decimal point")?;
        
        // Degrees are before the last 2 digits before decimal
        let degrees_end = if dot_pos >= 2 { dot_pos - 2 } else { 0 };
        
        let degrees_str = &coord_str[..degrees_end];
        let minutes_str = &coord_str[degrees_end..];

        let degrees = degrees_str.parse::<f64>().map_err(|_| "Invalid degrees")?;
        let minutes = minutes_str.parse::<f64>().map_err(|_| "Invalid minutes")?;

        let mut decimal_degrees = degrees + (minutes / 60.0);

        // Apply direction (N/S, E/W)
        if let Some(dir) = direction {
            if dir == "S" || dir == "W" {
                decimal_degrees = -decimal_degrees;
            }
        }

        Ok(decimal_degrees)
    }

    /// Get current GPS position
    pub fn get_position(&self) -> GpsPosition {
        self.position
    }

    /// Check if GPS has a valid fix
    pub fn has_fix(&self) -> bool {
        self.position.fix_quality != FixQuality::NoFix
            && self.position.satellites > 0
    }

    /// Check if UART connection is available
    pub fn is_connected(&self) -> bool {
        !self.connection_failed && self.reader.is_some()
    }
}
