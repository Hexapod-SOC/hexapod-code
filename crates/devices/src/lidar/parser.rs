//! Packet parser and frame assembler

use super::packet::{LidarPacket, PACKET_SIZE, POINTS_PER_PACKET};
use super::point::{Point, PointCloud};
use super::filter::NearRangeFilter;

/// Packet parsing state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Header,
    VerLen,
    Data(usize), // bytes collected
}

/// Parser for LiDAR data packets
pub struct PacketParser {
    state: ParserState,
    buffer: Vec<u8>,
    frame_buffer: Vec<Point>,
    last_angle: f32,
    speed: u16,
    timestamp: u16,
    error_count: u64,
    frame_ready: bool,
    latest_frame: Option<PointCloud>,
    filter: NearRangeFilter,
}

const POINT_FREQUENCY: f32 = 4500.0;

impl PacketParser {
    /// Create a new packet parser
    pub fn new() -> Self {
        Self {
            state: ParserState::Header,
            buffer: Vec::with_capacity(PACKET_SIZE),
            frame_buffer: Vec::with_capacity(500),
            last_angle: 0.0,
            speed: 0,
            timestamp: 0,
            error_count: 0,
            frame_ready: false,
            latest_frame: None,
            filter: NearRangeFilter::new(),
        }
    }

    /// Process incoming bytes
    pub fn process_bytes(&mut self, data: &[u8]) {
        for &byte in data {
            if self.process_byte(byte) {
                // Packet successfully parsed
                if let Some(packet) = LidarPacket::from_bytes(&self.buffer) {
                    if packet.validate() {
                        self.process_packet(packet);
                    } else {
                        self.error_count += 1;
                    }
                }
                self.buffer.clear();
                self.state = ParserState::Header;
            }
        }
    }

    /// Process a single byte through the state machine
    fn process_byte(&mut self, byte: u8) -> bool {
        match self.state {
            ParserState::Header => {
                if byte == super::packet::PKG_HEADER {
                    self.buffer.clear();
                    self.buffer.push(byte);
                    self.state = ParserState::VerLen;
                }
                false
            }
            ParserState::VerLen => {
                if byte == super::packet::PKG_VER_LEN {
                    self.buffer.push(byte);
                    self.state = ParserState::Data(2);
                    false
                } else {
                    self.state = ParserState::Header;
                    false
                }
            }
            ParserState::Data(count) => {
                self.buffer.push(byte);
                if self.buffer.len() >= PACKET_SIZE {
                    self.state = ParserState::Header;
                    true // Packet complete
                } else {
                    self.state = ParserState::Data(count + 1);
                    false
                }
            }
        }
    }

    /// Process a complete validated packet
    fn process_packet(&mut self, packet: LidarPacket) {
        self.speed = packet.speed;
        self.timestamp = packet.timestamp;

        // Calculate angle step between points
        let step = packet.angle_step();
        let start_angle = (packet.start_angle as f32) / 100.0;

        // Extract points from packet
        for i in 0..POINTS_PER_PACKET {
            let mut angle = start_angle + (i as f32) * step;
            if angle >= 360.0 {
                angle -= 360.0;
            }

            let point = Point::new(
                angle,
                packet.points[i].distance,
                packet.points[i].intensity,
            );

            self.frame_buffer.push(point);

            // Check for frame completion (rotation wrap-around)
            if angle < 20.0 && self.last_angle > 340.0 {
                self.try_assemble_frame();
            }

            self.last_angle = angle;
        }
    }

    /// Try to assemble a complete frame from buffered points
    fn try_assemble_frame(&mut self) {
        let count = self.frame_buffer.len();
        
        if count == 0 {
            return;
        }

        // Validate we have reasonable amount of data
        let speed_hz = (self.speed as f32) / 360.0;
        if (count as f32) * speed_hz > POINT_FREQUENCY * 1.4 {
            // Too much data, likely accumulated errors
            self.frame_buffer.clear();
            return;
        }

        // Apply filtering
        let filtered_points = self.filter.filter(&self.frame_buffer, self.speed);

        if !filtered_points.is_empty() {
            // Sort by angle
            let mut sorted_points = filtered_points;
            sorted_points.sort_by(|a, b| a.angle.partial_cmp(&b.angle).unwrap());

            let cloud = PointCloud::new(sorted_points, self.speed, self.timestamp);
            self.latest_frame = Some(cloud);
            self.frame_ready = true;
        }

        self.frame_buffer.clear();
    }

    /// Check if a complete frame is ready
    pub fn is_frame_ready(&self) -> bool {
        self.frame_ready
    }

    /// Get the latest point cloud and reset the ready flag
    pub fn get_point_cloud(&mut self) -> Option<PointCloud> {
        if self.frame_ready {
            self.frame_ready = false;
            self.latest_frame.clone()
        } else {
            None
        }
    }

    /// Get current rotation speed in Hz
    pub fn get_speed(&self) -> f64 {
        (self.speed as f64) / 360.0
    }

    /// Get the error count
    pub fn get_error_count(&self) -> u64 {
        self.error_count
    }

    /// Get the timestamp
    pub fn get_timestamp(&self) -> u16 {
        self.timestamp
    }
}

impl Default for PacketParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = PacketParser::new();
        assert_eq!(parser.state, ParserState::Header);
        assert!(!parser.is_frame_ready());
        assert_eq!(parser.get_error_count(), 0);
    }

    #[test]
    fn test_state_transitions() {
        let mut parser = PacketParser::new();
        
        // Should accept header
        assert!(!parser.process_byte(0x54));
        assert_eq!(parser.state, ParserState::VerLen);
        
        // Should accept version/length
        assert!(!parser.process_byte(0x2C));
        matches!(parser.state, ParserState::Data(_));
    }
}
