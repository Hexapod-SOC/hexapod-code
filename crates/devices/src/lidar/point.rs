//! LiDAR point data structures

use std::f32::consts::PI;

/// LiDAR product type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidarType {
    LD06,
    LD19,
    Unknown,
}

/// A single LiDAR measurement point
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Angle in degrees (0.0 to 359.99)
    pub angle: f32,
    /// Distance in millimeters
    pub distance: u16,
    /// Signal intensity (0-255)
    pub intensity: u8,
    /// Cartesian X coordinate (calculated)
    pub x: f32,
    /// Cartesian Y coordinate (calculated)
    pub y: f32,
}

impl Point {
    /// Create a new point with polar coordinates
    pub fn new(angle: f32, distance: u16, intensity: u8) -> Self {
        let (x, y) = Self::polar_to_cartesian(angle, distance);
        Self {
            angle,
            distance,
            intensity,
            x,
            y,
        }
    }

    /// Create a new point from raw angle value (in hundredths of degrees)
    pub fn from_raw(angle_raw: u16, distance: u16, intensity: u8) -> Self {
        let angle = (angle_raw as f32) / 100.0;
        Self::new(angle, distance, intensity)
    }

    /// Convert polar coordinates to Cartesian
    fn polar_to_cartesian(angle: f32, distance: u16) -> (f32, f32) {
        let angle_rad = angle * PI / 180.0;
        let dist_mm = distance as f32;
        let x = dist_mm * angle_rad.cos();
        let y = dist_mm * angle_rad.sin();
        (x, y)
    }

    /// Check if this is a valid measurement (non-zero distance)
    pub fn is_valid(&self) -> bool {
        self.distance > 0
    }

    /// Convert angle to radians
    pub fn angle_radians(&self) -> f32 {
        self.angle * PI / 180.0
    }
}

impl Default for Point {
    fn default() -> Self {
        Self {
            angle: 0.0,
            distance: 0,
            intensity: 0,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// A complete 360-degree scan of LiDAR points
#[derive(Debug, Clone)]
pub struct PointCloud {
    /// All points in the scan
    pub points: Vec<Point>,
    /// Rotation speed in degrees per second
    pub speed: u16,
    /// Timestamp in milliseconds
    pub timestamp: u16,
}

impl PointCloud {
    /// Create a new point cloud
    pub fn new(points: Vec<Point>, speed: u16, timestamp: u16) -> Self {
        Self {
            points,
            speed,
            timestamp,
        }
    }

    /// Get rotation frequency in Hz
    pub fn frequency(&self) -> f64 {
        (self.speed as f64) / 360.0
    }

    /// Get only valid points (distance > 0)
    pub fn valid_points(&self) -> impl Iterator<Item = &Point> {
        self.points.iter().filter(|p| p.is_valid())
    }

    /// Get the number of valid points
    pub fn valid_count(&self) -> usize {
        self.valid_points().count()
    }

    /// Get points within a specific angle range
    pub fn points_in_range(&self, start_angle: f32, end_angle: f32) -> Vec<&Point> {
        self.points
            .iter()
            .filter(|p| p.angle >= start_angle && p.angle <= end_angle)
            .collect()
    }

    /// Find the closest point in a given direction (angle in degrees)
    pub fn closest_in_direction(&self, angle: f32, tolerance: f32) -> Option<&Point> {
        self.points
            .iter()
            .filter(|p| {
                let diff = (p.angle - angle).abs();
                (diff <= tolerance || (360.0 - diff) <= tolerance) && p.is_valid()
            })
            .min_by_key(|p| p.distance)
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            speed: 0,
            timestamp: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let point = Point::new(45.0, 1000, 128);
        assert_eq!(point.angle, 45.0);
        assert_eq!(point.distance, 1000);
        assert_eq!(point.intensity, 128);
        assert!(point.is_valid());
    }

    #[test]
    fn test_point_from_raw() {
        let point = Point::from_raw(4500, 1000, 128);
        assert_eq!(point.angle, 45.0);
    }

    #[test]
    fn test_polar_to_cartesian() {
        let point = Point::new(0.0, 1000, 128);
        assert!((point.x - 1000.0).abs() < 1.0);
        assert!(point.y.abs() < 1.0);
    }

    #[test]
    fn test_point_cloud() {
        let points = vec![
            Point::new(0.0, 1000, 128),
            Point::new(90.0, 2000, 128),
            Point::new(180.0, 0, 128), // Invalid
        ];
        let cloud = PointCloud::new(points, 3600, 1000);
        assert_eq!(cloud.frequency(), 10.0);
        assert_eq!(cloud.valid_count(), 2);
    }
}
