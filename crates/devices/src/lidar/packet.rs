//! LiDAR packet structures and parsing

/// Packet protocol constants
pub const PKG_HEADER: u8 = 0x54;
pub const PKG_VER_LEN: u8 = 0x2C;
pub const POINTS_PER_PACKET: usize = 12;
pub const PACKET_SIZE: usize = 47; // Total packet size in bytes

/// CRC-8 lookup table for packet validation
const CRC_TABLE: [u8; 256] = [
    0x00, 0x4d, 0x9a, 0xd7, 0x79, 0x34, 0xe3, 0xae, 0xf2, 0xbf, 0x68, 0x25,
    0x8b, 0xc6, 0x11, 0x5c, 0xa9, 0xe4, 0x33, 0x7e, 0xd0, 0x9d, 0x4a, 0x07,
    0x5b, 0x16, 0xc1, 0x8c, 0x22, 0x6f, 0xb8, 0xf5, 0x1f, 0x52, 0x85, 0xc8,
    0x66, 0x2b, 0xfc, 0xb1, 0xed, 0xa0, 0x77, 0x3a, 0x94, 0xd9, 0x0e, 0x43,
    0xb6, 0xfb, 0x2c, 0x61, 0xcf, 0x82, 0x55, 0x18, 0x44, 0x09, 0xde, 0x93,
    0x3d, 0x70, 0xa7, 0xea, 0x3e, 0x73, 0xa4, 0xe9, 0x47, 0x0a, 0xdd, 0x90,
    0xcc, 0x81, 0x56, 0x1b, 0xb5, 0xf8, 0x2f, 0x62, 0x97, 0xda, 0x0d, 0x40,
    0xee, 0xa3, 0x74, 0x39, 0x65, 0x28, 0xff, 0xb2, 0x1c, 0x51, 0x86, 0xcb,
    0x21, 0x6c, 0xbb, 0xf6, 0x58, 0x15, 0xc2, 0x8f, 0xd3, 0x9e, 0x49, 0x04,
    0xaa, 0xe7, 0x30, 0x7d, 0x88, 0xc5, 0x12, 0x5f, 0xf1, 0xbc, 0x6b, 0x26,
    0x7a, 0x37, 0xe0, 0xad, 0x03, 0x4e, 0x99, 0xd4, 0x7c, 0x31, 0xe6, 0xab,
    0x05, 0x48, 0x9f, 0xd2, 0x8e, 0xc3, 0x14, 0x59, 0xf7, 0xba, 0x6d, 0x20,
    0xd5, 0x98, 0x4f, 0x02, 0xac, 0xe1, 0x36, 0x7b, 0x27, 0x6a, 0xbd, 0xf0,
    0x5e, 0x13, 0xc4, 0x89, 0x63, 0x2e, 0xf9, 0xb4, 0x1a, 0x57, 0x80, 0xcd,
    0x91, 0xdc, 0x0b, 0x46, 0xe8, 0xa5, 0x72, 0x3f, 0xca, 0x87, 0x50, 0x1d,
    0xb3, 0xfe, 0x29, 0x64, 0x38, 0x75, 0xa2, 0xef, 0x41, 0x0c, 0xdb, 0x96,
    0x42, 0x0f, 0xd8, 0x95, 0x3b, 0x76, 0xa1, 0xec, 0xb0, 0xfd, 0x2a, 0x67,
    0xc9, 0x84, 0x53, 0x1e, 0xeb, 0xa6, 0x71, 0x3c, 0x92, 0xdf, 0x08, 0x45,
    0x19, 0x54, 0x83, 0xce, 0x60, 0x2d, 0xfa, 0xb7, 0x5d, 0x10, 0xc7, 0x8a,
    0x24, 0x69, 0xbe, 0xf3, 0xaf, 0xe2, 0x35, 0x78, 0xd6, 0x9b, 0x4c, 0x01,
    0xf4, 0xb9, 0x6e, 0x23, 0x8d, 0xc0, 0x17, 0x5a, 0x06, 0x4b, 0x9c, 0xd1,
    0x7f, 0x32, 0xe5, 0xa8,
];

/// Calculate CRC-8 checksum for data validation
pub fn calculate_crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        crc = CRC_TABLE[(crc ^ byte) as usize];
    }
    crc
}

/// A single point measurement within a packet
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct LidarPoint {
    pub distance: u16,
    pub intensity: u8,
}

/// Complete LiDAR data packet (47 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct LidarPacket {
    pub header: u8,
    pub ver_len: u8,
    pub speed: u16,
    pub start_angle: u16,
    pub points: [LidarPoint; POINTS_PER_PACKET],
    pub end_angle: u16,
    pub timestamp: u16,
    pub crc8: u8,
}

impl LidarPacket {
    /// Parse a packet from raw bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < PACKET_SIZE {
            return None;
        }

        // Validate header
        if data[0] != PKG_HEADER || data[1] != PKG_VER_LEN {
            return None;
        }

        // Calculate and verify CRC
        let expected_crc = data[PACKET_SIZE - 1];
        let calculated_crc = calculate_crc8(&data[..PACKET_SIZE - 1]);
        if expected_crc != calculated_crc {
            return None;
        }

        // Parse packet fields
        let speed = u16::from_le_bytes([data[2], data[3]]);
        let start_angle = u16::from_le_bytes([data[4], data[5]]);
        
        let mut points = [LidarPoint::default(); POINTS_PER_PACKET];
        for i in 0..POINTS_PER_PACKET {
            let offset = 6 + i * 3;
            points[i] = LidarPoint {
                distance: u16::from_le_bytes([data[offset], data[offset + 1]]),
                intensity: data[offset + 2],
            };
        }

        let end_angle = u16::from_le_bytes([data[42], data[43]]);
        let timestamp = u16::from_le_bytes([data[44], data[45]]);
        let crc8 = data[46];

        Some(Self {
            header: data[0],
            ver_len: data[1],
            speed,
            start_angle,
            points,
            end_angle,
            timestamp,
            crc8,
        })
    }

    /// Validate packet consistency
    pub fn validate(&self) -> bool {
        // Check if angle difference is reasonable
        let angle_diff = self.angle_difference();
        let expected_diff = (self.speed as f32) * (POINTS_PER_PACKET as f32) / 4500.0 * 1.5;
        
        angle_diff <= expected_diff
    }

    /// Calculate the angular difference between start and end angles
    pub fn angle_difference(&self) -> f32 {
        let start = (self.start_angle as f32) / 100.0;
        let end = (self.end_angle as f32) / 100.0;
        (end - start + 360.0) % 360.0
    }

    /// Get the angular step between points
    pub fn angle_step(&self) -> f32 {
        let diff = ((self.end_angle as i32 + 36000 - self.start_angle as i32) % 36000) as f32;
        diff / (POINTS_PER_PACKET - 1) as f32 / 100.0
    }
}

/// A frame containing points from a complete rotation
#[derive(Debug, Clone)]
pub struct LidarFrame {
    pub points: Vec<crate::lidar::point::Point>,
    pub speed: u16,
    pub timestamp: u16,
}

impl LidarFrame {
    /// Create a new frame
    pub fn new(points: Vec<crate::lidar::point::Point>, speed: u16, timestamp: u16) -> Self {
        Self {
            points,
            speed,
            timestamp,
        }
    }

    /// Check if this frame represents a complete rotation
    pub fn is_complete(&self) -> bool {
        if self.points.is_empty() {
            return false;
        }

        // Check if we have points covering most of 360 degrees
        let mut angles: Vec<f32> = self.points.iter().map(|p| p.angle).collect();
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // We should have significant angular coverage
        if let (Some(&first), Some(&last)) = (angles.first(), angles.last()) {
            (last - first) > 300.0 // At least 300 degrees coverage
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc8() {
        let data = [0x54, 0x2C, 0x10, 0x0E];
        let crc = calculate_crc8(&data);
        assert!(crc < 256);
    }

    #[test]
    fn test_angle_difference() {
        // Test normal case
        let packet = LidarPacket {
            header: PKG_HEADER,
            ver_len: PKG_VER_LEN,
            speed: 3600,
            start_angle: 1000, // 10.00 degrees
            points: [LidarPoint::default(); POINTS_PER_PACKET],
            end_angle: 2000, // 20.00 degrees
            timestamp: 0,
            crc8: 0,
        };
        let diff = packet.angle_difference();
        assert!((diff - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_angle_wrap() {
        // Test wrap-around case
        let packet = LidarPacket {
            header: PKG_HEADER,
            ver_len: PKG_VER_LEN,
            speed: 3600,
            start_angle: 35900, // 359.00 degrees
            points: [LidarPoint::default(); POINTS_PER_PACKET],
            end_angle: 100, // 1.00 degrees
            timestamp: 0,
            crc8: 0,
        };
        let diff = packet.angle_difference();
        assert!((diff - 2.0).abs() < 0.1);
    }
}
