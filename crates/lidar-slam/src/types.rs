//! Core types for 2D SLAM
//!
//! Defines the fundamental data structures for scan matching and mapping.

use nalgebra::{Matrix2, Matrix3, Vector2};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// 2D point in the world coordinate frame
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn distance_to(&self, other: &Point2D) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn distance_squared_to(&self, other: &Point2D) -> f32 {
        (self.x - other.x).powi(2) + (self.y - other.y).powi(2)
    }

    /// Create from polar coordinates (angle in radians, distance in mm)
    pub fn from_polar(angle_rad: f32, distance_mm: f32) -> Self {
        Self {
            x: distance_mm * angle_rad.cos(),
            y: distance_mm * angle_rad.sin(),
        }
    }

    pub fn to_vec(&self) -> Vector2<f32> {
        Vector2::new(self.x, self.y)
    }
}

impl From<Vector2<f32>> for Point2D {
    fn from(v: Vector2<f32>) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl std::ops::Add for Point2D {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl std::ops::Sub for Point2D {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl std::ops::Mul<f32> for Point2D {
    type Output = Self;
    fn mul(self, scalar: f32) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

/// 2D robot pose (position + orientation)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pose2D {
    /// X position in millimeters
    pub x: f32,
    /// Y position in millimeters
    pub y: f32,
    /// Orientation in radians (-π to π)
    pub theta: f32,
}

impl Pose2D {
    pub fn new(x: f32, y: f32, theta: f32) -> Self {
        Self {
            x,
            y,
            theta: Self::normalize_angle(theta),
        }
    }

    pub fn origin() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            theta: 0.0,
        }
    }

    /// Normalize angle to [-π, π]
    fn normalize_angle(angle: f32) -> f32 {
        let mut a = angle;
        while a > PI {
            a -= 2.0 * PI;
        }
        while a < -PI {
            a += 2.0 * PI;
        }
        a
    }

    /// Get position as Point2D
    pub fn position(&self) -> Point2D {
        Point2D::new(self.x, self.y)
    }

    /// Compose this pose with another (apply transformation)
    pub fn compose(&self, other: &Pose2D) -> Pose2D {
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        Pose2D::new(
            self.x + cos_t * other.x - sin_t * other.y,
            self.y + sin_t * other.x + cos_t * other.y,
            self.theta + other.theta,
        )
    }

    /// Get the inverse of this pose
    pub fn inverse(&self) -> Pose2D {
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        Pose2D::new(
            -(cos_t * self.x + sin_t * self.y),
            -(cos_t * self.y - sin_t * self.x),
            -self.theta,
        )
    }

    /// Transform a point from local to world coordinates
    pub fn transform_point(&self, point: &Point2D) -> Point2D {
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        Point2D::new(
            self.x + cos_t * point.x - sin_t * point.y,
            self.y + sin_t * point.x + cos_t * point.y,
        )
    }

    /// Transform a point from world to local coordinates
    pub fn inverse_transform_point(&self, point: &Point2D) -> Point2D {
        let dx = point.x - self.x;
        let dy = point.y - self.y;
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        Point2D::new(cos_t * dx + sin_t * dy, -sin_t * dx + cos_t * dy)
    }

    /// Convert to 3x3 transformation matrix
    pub fn to_matrix(&self) -> Matrix3<f32> {
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        Matrix3::new(
            cos_t, -sin_t, self.x, sin_t, cos_t, self.y, 0.0, 0.0, 1.0,
        )
    }
}

impl Default for Pose2D {
    fn default() -> Self {
        Self::origin()
    }
}

/// 2D rigid transformation (rotation + translation)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    /// Rotation matrix
    pub rotation: Matrix2<f32>,
    /// Translation vector (mm)
    pub translation: Vector2<f32>,
}

impl Transform2D {
    pub fn new(rotation: Matrix2<f32>, translation: Vector2<f32>) -> Self {
        Self {
            rotation,
            translation,
        }
    }

    pub fn identity() -> Self {
        Self {
            rotation: Matrix2::identity(),
            translation: Vector2::zeros(),
        }
    }

    pub fn from_pose(pose: &Pose2D) -> Self {
        let cos_t = pose.theta.cos();
        let sin_t = pose.theta.sin();
        Self {
            rotation: Matrix2::new(cos_t, -sin_t, sin_t, cos_t),
            translation: Vector2::new(pose.x, pose.y),
        }
    }

    pub fn to_pose(&self) -> Pose2D {
        let theta = self.rotation[(1, 0)].atan2(self.rotation[(0, 0)]);
        Pose2D::new(self.translation.x, self.translation.y, theta)
    }

    /// Apply transformation to a point
    pub fn transform_point(&self, point: &Point2D) -> Point2D {
        let v = self.rotation * point.to_vec() + self.translation;
        Point2D::from(v)
    }

    /// Compose two transformations
    pub fn compose(&self, other: &Transform2D) -> Transform2D {
        Transform2D {
            rotation: self.rotation * other.rotation,
            translation: self.rotation * other.translation + self.translation,
        }
    }

    /// Get inverse transformation
    pub fn inverse(&self) -> Transform2D {
        let inv_rot = self.rotation.transpose();
        Transform2D {
            rotation: inv_rot,
            translation: -(inv_rot * self.translation),
        }
    }
}

/// A 2D laser scan (point cloud from one rotation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scan2D {
    /// Points in local (sensor) frame, in millimeters
    pub points: Vec<Point2D>,
    /// Timestamp in milliseconds
    pub timestamp: u64,
    /// Gyro reading at scan time (radians). 0.0 when disconnected.
    pub gyro_yaw: f32,
}

impl Scan2D {
    pub fn new(points: Vec<Point2D>, timestamp: u64) -> Self {
        Self {
            points,
            timestamp,
            gyro_yaw: 0.0, // Forced to 0 since gyro is disconnected
        }
    }

    pub fn with_gyro(mut self, gyro_yaw: f32) -> Self {
        self.gyro_yaw = gyro_yaw;
        self
    }

    /// Create from LiDAR PointCloud
    pub fn from_point_cloud(cloud: &devices::lidar::PointCloud) -> Self {
        let points: Vec<Point2D> = cloud
            .valid_points()
            .map(|p| Point2D::new(p.x, p.y))
            .collect();

        Self {
            points,
            timestamp: cloud.timestamp as u64,
            gyro_yaw: 0.0,
        }
    }

    /// Transform all points to world coordinates using the given pose
    pub fn transform(&self, pose: &Pose2D) -> Vec<Point2D> {
        self.points.iter().map(|p| pose.transform_point(p)).collect()
    }

    /// Get centroid of the scan
    pub fn centroid(&self) -> Point2D {
        if self.points.is_empty() {
            return Point2D::zero();
        }
        let sum = self.points.iter().fold(Point2D::zero(), |acc, p| acc + *p);
        sum * (1.0 / self.points.len() as f32)
    }

    /// Downsample the scan to reduce computation
    pub fn downsample(&self, factor: usize) -> Self {
        let points = self
            .points
            .iter()
            .step_by(factor)
            .copied()
            .collect();
        Self {
            points,
            timestamp: self.timestamp,
            gyro_yaw: self.gyro_yaw,
        }
    }

    /// Filter points by maximum distance from origin
    pub fn filter_by_distance(&self, max_distance: f32) -> Self {
        let points = self
            .points
            .iter()
            .filter(|p| (p.x.powi(2) + p.y.powi(2)).sqrt() <= max_distance)
            .copied()
            .collect();
        Self {
            points,
            timestamp: self.timestamp,
            gyro_yaw: self.gyro_yaw,
        }
    }
}

/// 3D point for future camera integration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Project to 2D (XY plane)
    pub fn to_2d(&self) -> Point2D {
        Point2D::new(self.x, self.y)
    }

    /// Create from 2D point with Z coordinate (for 2D lidar at given height)
    pub fn from_2d_with_height(point: &Point2D, height: f32) -> Self {
        Self {
            x: point.x,
            y: point.y,
            z: height,
        }
    }
}

/// Gyro data for orientation estimation
/// Currently forced to 0 since gyro is disconnected
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct GyroData {
    /// Roll angle (rotation around X axis) in radians
    pub roll: f32,
    /// Pitch angle (rotation around Y axis) in radians
    pub pitch: f32,
    /// Yaw angle (rotation around Z axis) in radians
    pub yaw: f32,
    /// Timestamp in milliseconds
    pub timestamp: u64,
}

impl GyroData {
    /// Create zero gyro reading (disconnected)
    pub fn disconnected() -> Self {
        Self::default()
    }

    /// Check if gyro data is valid (non-zero)
    pub fn is_connected(&self) -> bool {
        self.roll != 0.0 || self.pitch != 0.0 || self.yaw != 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn test_point_distance() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(3.0, 4.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_pose_transform_point() {
        let pose = Pose2D::new(100.0, 0.0, FRAC_PI_2); // 90 degrees
        let point = Point2D::new(10.0, 0.0);
        let transformed = pose.transform_point(&point);
        assert!((transformed.x - 100.0).abs() < 1e-4);
        assert!((transformed.y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_pose_compose_inverse() {
        let pose = Pose2D::new(100.0, 50.0, 0.5);
        let identity = pose.compose(&pose.inverse());
        assert!(identity.x.abs() < 1e-4);
        assert!(identity.y.abs() < 1e-4);
        assert!(identity.theta.abs() < 1e-4);
    }

    #[test]
    fn test_transform_roundtrip() {
        let pose = Pose2D::new(100.0, -50.0, 1.2);
        let transform = Transform2D::from_pose(&pose);
        let recovered = transform.to_pose();
        assert!((pose.x - recovered.x).abs() < 1e-4);
        assert!((pose.y - recovered.y).abs() < 1e-4);
        assert!((pose.theta - recovered.theta).abs() < 1e-4);
    }
}
