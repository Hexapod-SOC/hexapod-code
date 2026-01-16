use crate::types::{LaserScan, LidarPoint};
use thiserror::Error;

const PKG_HEADER: u8 = 0x54;
const PKG_VER_LEN: u8 = 0x2C;
const POINTS_PER_PACK: usize = 12;
const PKG_SIZE_BYTES: usize = 47;

/// Error conditions emitted by the LD19 parser.
#[derive(Debug, Error)]
pub enum DriverError {
    #[error("frame too short")]
    FrameTooShort,
    #[error("checksum mismatch")]
    ChecksumMismatch,
    #[error("unexpected header")]
    UnexpectedHeader,
}

pub type DriverResult<T> = Result<T, DriverError>;

#[derive(Debug)]
struct ParsedPacket {
    points: Vec<LidarPoint>,
    rpm: f32,
    timestamp: u16,
}

/// Stateful parser for LD19 packets. Data can be fed incrementally via `ingest_bytes`.
pub struct Ld19Parser {
    buffer: Vec<u8>,
    partial_scan: Vec<LidarPoint>,
    last_angle: f32,
    last_rpm: f32,
    last_timestamp: u16,
}

impl Default for Ld19Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Ld19Parser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
            partial_scan: Vec::new(),
            last_angle: 0.0,
            last_rpm: 0.0,
            last_timestamp: 0,
        }
    }

    /// Feed raw bytes from the serial device; returns zero or more completed scans.
    pub fn ingest_bytes(&mut self, data: &[u8]) -> DriverResult<Vec<LaserScan>> {
        self.buffer.extend_from_slice(data);
        let mut frames = Vec::new();

        loop {
            let start = match self.find_header() {
                Some(idx) => idx,
                None => {
                    self.buffer.clear();
                    break;
                }
            };

            if self.buffer.len() < start + PKG_SIZE_BYTES {
                if start > 0 {
                    self.buffer.drain(..start);
                }
                break;
            }

            let packet = self.buffer[start..start + PKG_SIZE_BYTES].to_vec();
            self.buffer.drain(..start + PKG_SIZE_BYTES);

            if !self.validate_crc(&packet) {
                continue;
            }

            let parsed = self.parse_packet(&packet)?;
            frames.extend(self.accumulate_frame(parsed));
        }

        Ok(frames)
    }

    fn accumulate_frame(&mut self, packet: ParsedPacket) -> Vec<LaserScan> {
        let mut completed = Vec::new();
        for point in packet.points.into_iter() {
            if point.angle_deg < 20.0 && self.last_angle > 340.0 && !self.partial_scan.is_empty() {
                // Finish the current revolution; start the next with the current point.
                let mut scan_points = Vec::new();
                std::mem::swap(&mut scan_points, &mut self.partial_scan);
                let rpm = if packet.rpm > 0.0 {
                    packet.rpm
                } else {
                    self.last_rpm
                };
                let timestamp_ns = (packet.timestamp as u64) * 1_000_000;
                completed.push(LaserScan {
                    timestamp_ns,
                    rpm,
                    start_angle_deg: scan_points.first().map(|p| p.angle_deg).unwrap_or(0.0),
                    end_angle_deg: self.last_angle,
                    points: scan_points,
                });
            }
            self.last_angle = point.angle_deg;
            self.partial_scan.push(point);
        }
        self.last_rpm = packet.rpm;
        self.last_timestamp = packet.timestamp;
        completed
    }

    /// Convenience to build a scan from already-demarcated packet bytes.
    pub fn parse_frame(&mut self, frame: &[u8]) -> DriverResult<LaserScan> {
        let packet = self.parse_packet(frame)?;
        let mut scans = self.accumulate_frame(packet);
        if let Some(scan) = scans.pop() {
            Ok(scan)
        } else {
            Ok(LaserScan {
                timestamp_ns: (self.last_timestamp as u64) * 1_000_000,
                rpm: self.last_rpm,
                start_angle_deg: self
                    .partial_scan
                    .first()
                    .map(|p| p.angle_deg)
                    .unwrap_or(0.0),
                end_angle_deg: self.last_angle,
                points: self.partial_scan.clone(),
            })
        }
    }

    fn find_header(&self) -> Option<usize> {
        self.buffer
            .windows(2)
            .position(|w| w[0] == PKG_HEADER && w[1] == PKG_VER_LEN)
    }

    fn validate_crc(&self, packet: &[u8]) -> bool {
        if packet.len() < PKG_SIZE_BYTES {
            return false;
        }
        let expected = packet[PKG_SIZE_BYTES - 1];
        let actual = calc_crc8(&packet[..PKG_SIZE_BYTES - 1]);
        expected == actual
    }

    fn parse_packet(&self, packet: &[u8]) -> DriverResult<ParsedPacket> {
        if packet.len() < PKG_SIZE_BYTES {
            return Err(DriverError::FrameTooShort);
        }
        if packet[0] != PKG_HEADER || packet[1] != PKG_VER_LEN {
            return Err(DriverError::UnexpectedHeader);
        }

        let speed = u16::from_le_bytes([packet[2], packet[3]]) as f32;
        let rpm = speed / 6.0;
        let start_angle_raw = u16::from_le_bytes([packet[4], packet[5]]);
        let end_angle_raw = u16::from_le_bytes([packet[42], packet[43]]);
        let timestamp = u16::from_le_bytes([packet[44], packet[45]]);

        let start_angle_deg = start_angle_raw as f32 / 100.0;
        let diff_raw = (end_angle_raw as i32 + 36000 - start_angle_raw as i32) % 36000;
        let diff = diff_raw as f32;
        let step = diff / (POINTS_PER_PACK as f32 - 1.0) / 100.0;

        let mut points = Vec::with_capacity(POINTS_PER_PACK);
        for i in 0..POINTS_PER_PACK {
            let offset = 6 + i * 3;
            let distance_mm = u16::from_le_bytes([packet[offset], packet[offset + 1]]);
            let intensity = packet[offset + 2] as u16;
            let mut angle = start_angle_deg + step * i as f32;
            if angle >= 360.0 {
                angle -= 360.0;
            }
            points.push(LidarPoint {
                angle_deg: angle,
                distance_m: distance_mm as f32 / 1000.0,
                intensity,
            });
        }

        Ok(ParsedPacket {
            points,
            rpm,
            timestamp,
        })
    }
}

fn calc_crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &value in data {
        let idx = (crc ^ value) as usize;
        crc = CRC_TABLE[idx];
    }
    crc
}

const CRC_TABLE: [u8; 256] = [
    0x00, 0x4D, 0x9A, 0xD7, 0x79, 0x34, 0xE3, 0xAE, 0xF2, 0xBF, 0x68, 0x25, 0x8B, 0xC6, 0x11, 0x5C,
    0xA9, 0xE4, 0x33, 0x7E, 0xD0, 0x9D, 0x4A, 0x07, 0x5B, 0x16, 0xC1, 0x8C, 0x22, 0x6F, 0xB8, 0xF5,
    0x1F, 0x52, 0x85, 0xC8, 0x66, 0x2B, 0xFC, 0xB1, 0xED, 0xA0, 0x77, 0x3A, 0x94, 0xD9, 0x0E, 0x43,
    0xB6, 0xFB, 0x2C, 0x61, 0xCF, 0x82, 0x55, 0x18, 0x44, 0x09, 0xDE, 0x93, 0x3D, 0x70, 0xA7, 0xEA,
    0x3E, 0x73, 0xA4, 0xE9, 0x47, 0x0A, 0xDD, 0x90, 0xCC, 0x81, 0x56, 0x1B, 0xB5, 0xF8, 0x2F, 0x62,
    0x97, 0xDA, 0x0D, 0x40, 0xEE, 0xA3, 0x74, 0x39, 0x65, 0x28, 0xFF, 0xB2, 0x1C, 0x51, 0x86, 0xCB,
    0x21, 0x6C, 0xBB, 0xF6, 0x58, 0x15, 0xC2, 0x8F, 0xD3, 0x9E, 0x49, 0x04, 0xAA, 0xE7, 0x30, 0x7D,
    0x88, 0xC5, 0x12, 0x5F, 0xF1, 0xBC, 0x6B, 0x26, 0x7A, 0x37, 0xE0, 0xAD, 0x03, 0x4E, 0x99, 0xD4,
    0x7C, 0x31, 0xE6, 0xAB, 0x05, 0x48, 0x9F, 0xD2, 0x8E, 0xC3, 0x14, 0x59, 0xF7, 0xBA, 0x6D, 0x20,
    0xD5, 0x98, 0x4F, 0x02, 0xAC, 0xE1, 0x36, 0x7B, 0x27, 0x6A, 0xBD, 0xF0, 0x5E, 0x13, 0xC4, 0x89,
    0x63, 0x2E, 0xF9, 0xB4, 0x1A, 0x57, 0x80, 0xCD, 0x91, 0xDC, 0x0B, 0x46, 0xE8, 0xA5, 0x72, 0x3F,
    0xCA, 0x87, 0x50, 0x1D, 0xB3, 0xFE, 0x29, 0x64, 0x38, 0x75, 0xA2, 0xEF, 0x41, 0x0C, 0xDB, 0x96,
    0x42, 0x0F, 0xD8, 0x95, 0x3B, 0x76, 0xA1, 0xEC, 0xB0, 0xFD, 0x2A, 0x67, 0xC9, 0x84, 0x53, 0x1E,
    0xEB, 0xA6, 0x71, 0x3C, 0x92, 0xDF, 0x08, 0x45, 0x19, 0x54, 0x83, 0xCE, 0x60, 0x2D, 0xFA, 0xB7,
    0x5D, 0x10, 0xC7, 0x8A, 0x24, 0x69, 0xBE, 0xF3, 0xAF, 0xE2, 0x35, 0x78, 0xD6, 0x9B, 0x4C, 0x01,
    0xF4, 0xB9, 0x6E, 0x23, 0x8D, 0xC0, 0x17, 0x5A, 0x06, 0x4B, 0x9C, 0xD1, 0x7F, 0x32, 0xE5, 0xA8,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn build_packet(start_angle_deg: f32, step_deg: f32, distance_mm: u16) -> Vec<u8> {
        let mut pkt = vec![0u8; PKG_SIZE_BYTES];
        pkt[0] = PKG_HEADER;
        pkt[1] = PKG_VER_LEN;
        let speed: u16 = 3600;
        pkt[2..4].copy_from_slice(&speed.to_le_bytes());

        let start_raw = (start_angle_deg * 100.0) as u16;
        let end_raw =
            ((start_angle_deg + step_deg * (POINTS_PER_PACK as f32 - 1.0)) * 100.0) as u16;
        pkt[4..6].copy_from_slice(&start_raw.to_le_bytes());

        for i in 0..POINTS_PER_PACK {
            let offset = 6 + i * 3;
            pkt[offset..offset + 2].copy_from_slice(&distance_mm.to_le_bytes());
            pkt[offset + 2] = 100;
        }

        pkt[42..44].copy_from_slice(&end_raw.to_le_bytes());
        let timestamp: u16 = 1234;
        pkt[44..46].copy_from_slice(&timestamp.to_le_bytes());
        let crc = calc_crc8(&pkt[..PKG_SIZE_BYTES - 1]);
        pkt[PKG_SIZE_BYTES - 1] = crc;
        pkt
    }

    #[test]
    fn parses_single_packet() {
        let pkt = build_packet(10.0, 1.0, 1000);
        let mut parser = Ld19Parser::new();
        let scan = parser.parse_frame(&pkt).expect("frame");
        assert_eq!(scan.points.len(), POINTS_PER_PACK);
        assert!((scan.rpm - 600.0).abs() < 1e-3);
        assert!((scan.points[0].angle_deg - 10.0).abs() < 1e-3);
        assert!((scan.points.last().unwrap().angle_deg - 21.0).abs() < 1e-3);
    }

    #[test]
    fn detects_wrap_and_emits_frame() {
        let pkt1 = build_packet(350.0, 0.5, 800);
        let pkt2 = build_packet(0.0, 0.5, 800);

        let mut parser = Ld19Parser::new();
        let frames0 = parser.ingest_bytes(&pkt1).unwrap();
        assert!(frames0.is_empty());

        let frames1 = parser.ingest_bytes(&pkt2).unwrap();
        assert_eq!(frames1.len(), 1);
        assert_eq!(frames1[0].points.len(), POINTS_PER_PACK);
    }
}
