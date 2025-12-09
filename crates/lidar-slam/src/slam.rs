//! SLAM Processor
//!
//! Main SLAM implementation that combines scan matching with mapping.
//! Processes LiDAR scans to estimate robot pose and build an occupancy grid map.

use crate::icp::{IcpConfig, IcpMatcher, IcpResult};
use crate::occupancy_grid::{OccupancyGrid, OccupancyGridConfig};
use crate::types::{GyroData, Point2D, Pose2D, Scan2D};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// SLAM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlamConfig {
    /// ICP scan matching configuration
    pub icp: IcpConfig,
    /// Occupancy grid configuration
    pub grid: OccupancyGridConfig,
    /// Minimum travel distance (mm) before updating map
    pub min_update_distance: f32,
    /// Minimum rotation (radians) before updating map
    pub min_update_rotation: f32,
    /// Maximum scan age for matching (ms)
    pub max_scan_age: u64,
    /// Number of previous scans to keep for matching
    pub scan_history_size: usize,
    /// Downsample factor for ICP (use every Nth point)
    pub icp_downsample: usize,
    /// Maximum range for scan points (mm)
    pub max_range: f32,
    /// Height of the LiDAR sensor from ground (mm) - for future 3D
    pub lidar_height: f32,
    /// Use gyro data for orientation (when connected)
    pub use_gyro: bool,
}

impl Default for SlamConfig {
    fn default() -> Self {
        Self {
            icp: IcpConfig::default(),
            grid: OccupancyGridConfig::default(),
            min_update_distance: 50.0,   // 5cm
            min_update_rotation: 0.05,   // ~3 degrees
            max_scan_age: 500,           // 500ms
            scan_history_size: 10,
            icp_downsample: 2,
            max_range: 8000.0,           // 8m
            lidar_height: 100.0,         // 10cm from ground (hexapod body height)
            use_gyro: false,             // Disabled since gyro is disconnected
        }
    }
}

/// SLAM state for serialization/debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlamState {
    pub pose: Pose2D,
    pub velocity: Pose2D,
    pub last_update_pose: Pose2D,
    pub scan_count: u64,
    pub match_quality: f32,
    pub is_initialized: bool,
}

/// Scan matching result
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Estimated pose change
    pub delta_pose: Pose2D,
    /// ICP result details
    pub icp_result: IcpResult,
    /// Quality score (0-1)
    pub quality: f32,
}

/// Main SLAM processor
pub struct SlamProcessor {
    /// Current robot pose estimate
    pose: Pose2D,
    /// Velocity estimate (for prediction)
    velocity: Pose2D,
    /// Pose at last map update
    last_update_pose: Pose2D,
    /// Occupancy grid map
    map: OccupancyGrid,
    /// ICP scan matcher
    icp: IcpMatcher,
    /// Previous scans for matching
    scan_history: VecDeque<(Pose2D, Scan2D)>,
    /// Configuration
    config: SlamConfig,
    /// Total scans processed
    scan_count: u64,
    /// Last match quality
    last_match_quality: f32,
    /// Is SLAM initialized with first scan
    is_initialized: bool,
    /// Latest gyro reading
    gyro: GyroData,
    /// Pose history for trajectory visualization
    trajectory: Vec<Pose2D>,
}

impl SlamProcessor {
    /// Create a new SLAM processor
    pub fn new(config: SlamConfig) -> Self {
        let map = OccupancyGrid::new(config.grid.clone());
        let icp = IcpMatcher::new(config.icp.clone());

        Self {
            pose: Pose2D::origin(),
            velocity: Pose2D::origin(),
            last_update_pose: Pose2D::origin(),
            map,
            icp,
            scan_history: VecDeque::with_capacity(config.scan_history_size),
            config,
            scan_count: 0,
            last_match_quality: 0.0,
            is_initialized: false,
            gyro: GyroData::disconnected(),
            trajectory: Vec::new(),
        }
    }

    /// Process a new LiDAR scan
    pub fn process_scan(&mut self, cloud: &devices::lidar::PointCloud) -> Option<MatchResult> {
        let scan = Scan2D::from_point_cloud(cloud);
        self.process_scan_2d(&scan)
    }

    /// Process a 2D scan directly
    pub fn process_scan_2d(&mut self, scan: &Scan2D) -> Option<MatchResult> {
        // Filter and downsample scan
        let filtered_scan = scan
            .filter_by_distance(self.config.max_range)
            .downsample(self.config.icp_downsample);

        if filtered_scan.points.len() < self.config.icp.min_correspondences {
            return None;
        }

        self.scan_count += 1;

        // First scan - just initialize
        if !self.is_initialized {
            self.initialize_with_scan(&filtered_scan);
            return None;
        }

        // Predict pose using velocity model (or gyro if available)
        let predicted_pose = self.predict_pose(&filtered_scan);

        // Match against previous scan(s)
        let match_result = self.match_scan(&filtered_scan, &predicted_pose);

        // Update pose if match is good enough
        if let Some(ref result) = match_result {
            if result.quality > 0.3 {
                self.pose = predicted_pose.compose(&result.delta_pose);
                self.last_match_quality = result.quality;

                // Update velocity estimate
                // Simple: assume delta_pose happened over one scan period
                self.velocity = result.delta_pose;
            }
        }

        // Check if we should update the map
        if self.should_update_map() {
            self.update_map(&filtered_scan);
            self.last_update_pose = self.pose;
            self.trajectory.push(self.pose);
        }

        // Store scan in history
        self.add_to_history(filtered_scan);

        match_result
    }

    /// Initialize SLAM with the first scan
    fn initialize_with_scan(&mut self, scan: &Scan2D) {
        // Update map with initial scan at origin
        self.map.update_from_scan(&self.pose, &scan.points);
        self.add_to_history(scan.clone());
        self.trajectory.push(self.pose);
        self.is_initialized = true;
    }

    /// Predict next pose based on motion model
    fn predict_pose(&self, _scan: &Scan2D) -> Pose2D {
        // If gyro is connected, use it for orientation
        if self.config.use_gyro && self.gyro.is_connected() {
            Pose2D::new(
                self.pose.x + self.velocity.x,
                self.pose.y + self.velocity.y,
                self.gyro.yaw,
            )
        } else {
            // Simple constant velocity prediction
            self.pose.compose(&self.velocity)
        }
    }

    /// Match scan against reference (previous scans or map)
    fn match_scan(&self, scan: &Scan2D, predicted_pose: &Pose2D) -> Option<MatchResult> {
        // Get reference points - prefer map if it has enough points
        let map_points = self.map.get_occupied_points();
        let reference_points = if map_points.len() > 100 {
            // Use map points for scan-to-map matching (more stable)
            map_points
        } else if !self.scan_history.is_empty() {
            // Fall back to scan-to-scan matching
            self.get_reference_points()
        } else {
            return None;
        };

        if reference_points.len() < 20 {
            return None;
        }

        // Transform current scan to predicted pose
        let current_points: Vec<Point2D> = scan
            .points
            .iter()
            .map(|p| predicted_pose.transform_point(p))
            .collect();

        // Run ICP
        let icp_result = self.icp.match_scans(&current_points, &reference_points, None);

        // Calculate quality score
        let quality = self.calculate_match_quality(&icp_result);
        
        // Get the delta pose
        let mut delta_pose = icp_result.transform.to_pose();
        
        // Limit maximum pose change to prevent jumps (motion constraints)
        // Max 100mm translation, 10 degrees rotation per scan
        let max_trans = 100.0;
        let max_rot = 0.175; // ~10 degrees
        
        let trans_mag = (delta_pose.x.powi(2) + delta_pose.y.powi(2)).sqrt();
        if trans_mag > max_trans {
            let scale = max_trans / trans_mag;
            delta_pose.x *= scale;
            delta_pose.y *= scale;
        }
        delta_pose.theta = delta_pose.theta.clamp(-max_rot, max_rot);

        Some(MatchResult {
            delta_pose,
            icp_result,
            quality,
        })
    }

    /// Get reference points from scan history
    fn get_reference_points(&self) -> Vec<Point2D> {
        // Combine points from recent scans, transformed to world coordinates
        let mut points = Vec::new();
        for (pose, scan) in self.scan_history.iter().take(3) {
            for p in &scan.points {
                points.push(pose.transform_point(p));
            }
        }
        
        // Alternatively, use map points for matching against the global map
        // This could be an option in config
        // points.extend(self.map.get_occupied_points());
        
        points
    }

    /// Calculate match quality score (0-1)
    fn calculate_match_quality(&self, result: &IcpResult) -> f32 {
        if !result.converged {
            return 0.1; // Still give small score for non-converged but usable results
        }

        // Quality based on:
        // 1. Number of correspondences (more is better)
        let corr_score = (result.num_correspondences as f32 / 50.0).min(1.0);
        
        // 2. MSE (lower is better) - adjusted for mm scale
        let mse_score = (-result.mse / 500.0).exp();
        
        // 3. Number of iterations (fewer is better, indicates good initial guess)
        let iter_score = 1.0 - (result.iterations as f32 / self.config.icp.max_iterations as f32);

        (corr_score * 0.5 + mse_score * 0.35 + iter_score * 0.15).clamp(0.0, 1.0)
    }

    /// Check if we should update the map
    fn should_update_map(&self) -> bool {
        // Always update on first few scans to build initial map
        if self.scan_count < 10 {
            return true;
        }
        
        let dx = self.pose.x - self.last_update_pose.x;
        let dy = self.pose.y - self.last_update_pose.y;
        let distance = (dx * dx + dy * dy).sqrt();

        let dtheta = (self.pose.theta - self.last_update_pose.theta).abs();

        distance >= self.config.min_update_distance || dtheta >= self.config.min_update_rotation
    }

    /// Update the occupancy grid map
    fn update_map(&mut self, scan: &Scan2D) {
        self.map.update_from_scan(&self.pose, &scan.points);
    }

    /// Add scan to history
    fn add_to_history(&mut self, scan: Scan2D) {
        if self.scan_history.len() >= self.config.scan_history_size {
            self.scan_history.pop_back();
        }
        self.scan_history.push_front((self.pose, scan));
    }

    /// Update gyro reading
    pub fn update_gyro(&mut self, gyro: GyroData) {
        self.gyro = gyro;
    }

    /// Get current pose estimate
    pub fn current_pose(&self) -> Pose2D {
        self.pose
    }

    /// Get reference to the map
    pub fn get_map(&self) -> &OccupancyGrid {
        &self.map
    }

    /// Get mutable reference to the map
    pub fn get_map_mut(&mut self) -> &mut OccupancyGrid {
        &mut self.map
    }

    /// Get trajectory history
    pub fn trajectory(&self) -> &[Pose2D] {
        &self.trajectory
    }

    /// Get current state for serialization
    pub fn get_state(&self) -> SlamState {
        SlamState {
            pose: self.pose,
            velocity: self.velocity,
            last_update_pose: self.last_update_pose,
            scan_count: self.scan_count,
            match_quality: self.last_match_quality,
            is_initialized: self.is_initialized,
        }
    }

    /// Reset SLAM to initial state
    pub fn reset(&mut self) {
        self.pose = Pose2D::origin();
        self.velocity = Pose2D::origin();
        self.last_update_pose = Pose2D::origin();
        self.map.clear();
        self.scan_history.clear();
        self.scan_count = 0;
        self.last_match_quality = 0.0;
        self.is_initialized = false;
        self.trajectory.clear();
    }

    /// Set robot pose manually (for localization)
    pub fn set_pose(&mut self, pose: Pose2D) {
        self.pose = pose;
        self.last_update_pose = pose;
    }

    /// Get number of scans processed
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }

    /// Check if SLAM is initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Export SLAM data as JSON for web visualization
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "pose": {
                "x": self.pose.x,
                "y": self.pose.y,
                "theta": self.pose.theta,
            },
            "map": self.map.to_json(),
            "trajectory": self.trajectory.iter().map(|p| {
                serde_json::json!({"x": p.x, "y": p.y, "theta": p.theta})
            }).collect::<Vec<_>>(),
            "state": {
                "scan_count": self.scan_count,
                "match_quality": self.last_match_quality,
                "is_initialized": self.is_initialized,
            }
        })
    }
}

/// Builder for SlamProcessor with fluent API
pub struct SlamBuilder {
    config: SlamConfig,
}

impl SlamBuilder {
    pub fn new() -> Self {
        Self {
            config: SlamConfig::default(),
        }
    }

    pub fn with_grid_resolution(mut self, resolution: f32) -> Self {
        self.config.grid.resolution = resolution;
        self
    }

    pub fn with_grid_size(mut self, width: usize, height: usize) -> Self {
        self.config.grid.width = width;
        self.config.grid.height = height;
        self
    }

    pub fn with_max_range(mut self, max_range: f32) -> Self {
        self.config.max_range = max_range;
        self
    }

    pub fn with_icp_config(mut self, icp: IcpConfig) -> Self {
        self.config.icp = icp;
        self
    }

    pub fn with_gyro(mut self, enabled: bool) -> Self {
        self.config.use_gyro = enabled;
        self
    }

    pub fn with_lidar_height(mut self, height: f32) -> Self {
        self.config.lidar_height = height;
        self
    }

    pub fn build(self) -> SlamProcessor {
        SlamProcessor::new(self.config)
    }
}

impl Default for SlamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slam_creation() {
        let slam = SlamProcessor::new(SlamConfig::default());
        assert!(!slam.is_initialized());
        assert_eq!(slam.scan_count(), 0);
    }

    #[test]
    fn test_slam_builder() {
        let slam = SlamBuilder::new()
            .with_grid_resolution(100.0)
            .with_max_range(5000.0)
            .build();

        assert_eq!(slam.get_map().resolution(), 100.0);
    }

    #[test]
    fn test_slam_initialization() {
        let mut slam = SlamProcessor::new(SlamConfig::default());
        
        // Create a simple scan
        let points: Vec<Point2D> = (0..36)
            .map(|i| {
                let angle = (i as f32) * 10.0 * std::f32::consts::PI / 180.0;
                Point2D::from_polar(angle, 1000.0)
            })
            .collect();
        let scan = Scan2D::new(points, 0);

        slam.process_scan_2d(&scan);
        assert!(slam.is_initialized());
    }
}
