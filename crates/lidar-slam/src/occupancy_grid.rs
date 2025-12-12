//! 2D Occupancy Grid Map
//!
//! A grid-based map representation where each cell stores the probability
//! of being occupied. This is the standard representation for 2D SLAM.

use crate::types::{Point2D, Pose2D};
use serde::{Deserialize, Serialize};

/// Cell state in the occupancy grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellState {
    /// Cell has not been observed
    Unknown,
    /// Cell is known to be free (no obstacle)
    Free,
    /// Cell is known to be occupied (obstacle present)
    Occupied,
}

impl CellState {
    /// Convert to log-odds value
    pub fn to_log_odds(&self) -> f32 {
        match self {
            CellState::Unknown => 0.0,
            CellState::Free => -2.0,
            CellState::Occupied => 2.0,
        }
    }

    /// Convert from log-odds value
    pub fn from_log_odds(log_odds: f32) -> Self {
        if log_odds > 0.5 {
            CellState::Occupied
        } else if log_odds < -0.5 {
            CellState::Free
        } else {
            CellState::Unknown
        }
    }
}

/// Configuration for the occupancy grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OccupancyGridConfig {
    /// Cell size in millimeters
    pub resolution: f32,
    /// Grid width in cells
    pub width: usize,
    /// Grid height in cells
    pub height: usize,
    /// Log-odds value for free cells
    pub log_odds_free: f32,
    /// Log-odds value for occupied cells
    pub log_odds_occupied: f32,
    /// Maximum log-odds value (clamping)
    pub log_odds_max: f32,
    /// Minimum log-odds value (clamping)
    pub log_odds_min: f32,
}

impl Default for OccupancyGridConfig {
    fn default() -> Self {
        Self {
            resolution: 50.0, // 50mm = 5cm per cell
            width: 400,       // 20m x 20m map
            height: 400,
            log_odds_free: -0.4,
            log_odds_occupied: 0.85,
            log_odds_max: 5.0,
            log_odds_min: -5.0,
        }
    }
}

/// 2D Occupancy Grid Map
#[derive(Clone, Serialize, Deserialize)]
pub struct OccupancyGrid {
    /// Grid cells stored as log-odds values
    cells: Vec<f32>,
    /// Configuration
    config: OccupancyGridConfig,
    /// Origin of the map in world coordinates (bottom-left corner)
    origin: Point2D,
    /// Number of updates performed
    update_count: u64,
}

impl OccupancyGrid {
    /// Create a new occupancy grid centered at the origin
    pub fn new(config: OccupancyGridConfig) -> Self {
        let cells = vec![0.0; config.width * config.height];

        // Center the grid at world origin
        let origin = Point2D::new(
            -(config.width as f32 * config.resolution) / 2.0,
            -(config.height as f32 * config.resolution) / 2.0,
        );

        Self {
            cells,
            config,
            origin,
            update_count: 0,
        }
    }

    /// Create with custom origin
    pub fn with_origin(config: OccupancyGridConfig, origin: Point2D) -> Self {
        let cells = vec![0.0; config.width * config.height];
        Self {
            cells,
            config,
            origin,
            update_count: 0,
        }
    }

    /// Get grid dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.config.width, self.config.height)
    }

    /// Get cell resolution in mm
    pub fn resolution(&self) -> f32 {
        self.config.resolution
    }

    /// Get grid origin in world coordinates
    pub fn origin(&self) -> Point2D {
        self.origin
    }

    /// Convert world coordinates to grid cell indices
    pub fn world_to_grid(&self, point: &Point2D) -> Option<(usize, usize)> {
        let gx = ((point.x - self.origin.x) / self.config.resolution).floor() as i32;
        let gy = ((point.y - self.origin.y) / self.config.resolution).floor() as i32;

        if gx >= 0 && gx < self.config.width as i32 && gy >= 0 && gy < self.config.height as i32 {
            Some((gx as usize, gy as usize))
        } else {
            None
        }
    }

    /// Convert grid cell indices to world coordinates (cell center)
    pub fn grid_to_world(&self, gx: usize, gy: usize) -> Point2D {
        Point2D::new(
            self.origin.x + (gx as f32 + 0.5) * self.config.resolution,
            self.origin.y + (gy as f32 + 0.5) * self.config.resolution,
        )
    }

    /// Get cell index in the flat array
    fn cell_index(&self, gx: usize, gy: usize) -> usize {
        gy * self.config.width + gx
    }

    /// Get log-odds value at grid coordinates
    pub fn get_log_odds(&self, gx: usize, gy: usize) -> f32 {
        if gx < self.config.width && gy < self.config.height {
            self.cells[self.cell_index(gx, gy)]
        } else {
            0.0
        }
    }

    /// Get cell state at grid coordinates
    pub fn get_cell(&self, gx: usize, gy: usize) -> CellState {
        CellState::from_log_odds(self.get_log_odds(gx, gy))
    }

    /// Get cell state at world coordinates
    pub fn get_cell_at(&self, point: &Point2D) -> CellState {
        self.world_to_grid(point)
            .map(|(gx, gy)| self.get_cell(gx, gy))
            .unwrap_or(CellState::Unknown)
    }

    /// Get occupancy probability (0.0 to 1.0) at world coordinates
    pub fn get_probability(&self, point: &Point2D) -> f32 {
        self.world_to_grid(point)
            .map(|(gx, gy)| {
                let log_odds = self.get_log_odds(gx, gy);
                1.0 / (1.0 + (-log_odds).exp())
            })
            .unwrap_or(0.5)
    }

    /// Update a single cell with log-odds
    fn update_cell(&mut self, gx: usize, gy: usize, log_odds_update: f32) {
        if gx < self.config.width && gy < self.config.height {
            let idx = self.cell_index(gx, gy);
            self.cells[idx] = (self.cells[idx] + log_odds_update)
                .clamp(self.config.log_odds_min, self.config.log_odds_max);
        }
    }

    /// Update the map with a laser scan from the given pose
    pub fn update_from_scan(&mut self, pose: &Pose2D, points: &[Point2D]) {
        let robot_grid = match self.world_to_grid(&pose.position()) {
            Some(pos) => pos,
            None => return, // Robot is outside the map
        };

        for point in points {
            // Transform point to world coordinates
            let world_point = pose.transform_point(point);

            if let Some((end_x, end_y)) = self.world_to_grid(&world_point) {
                // Trace ray from robot to endpoint using Bresenham's line algorithm
                self.trace_ray(robot_grid.0, robot_grid.1, end_x, end_y);

                // Mark endpoint as occupied
                self.update_cell(end_x, end_y, self.config.log_odds_occupied);
            }
        }

        self.update_count += 1;
    }

    /// Trace a ray using Bresenham's line algorithm, marking cells as free
    fn trace_ray(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let mut x = x0 as i32;
        let mut y = y0 as i32;
        let x1 = x1 as i32;
        let y1 = y1 as i32;

        let dx = (x1 - x).abs();
        let dy = -(y1 - y).abs();
        let sx = if x < x1 { 1 } else { -1 };
        let sy = if y < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            // Don't mark the endpoint as free (it will be marked occupied)
            if x == x1 && y == y1 {
                break;
            }

            if x >= 0 && x < self.config.width as i32 && y >= 0 && y < self.config.height as i32 {
                self.update_cell(x as usize, y as usize, self.config.log_odds_free);
            }

            let e2 = 2 * err;
            if e2 >= dy {
                if x == x1 {
                    break;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 {
                    break;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Get all occupied cells as world points
    pub fn get_occupied_points(&self) -> Vec<Point2D> {
        let mut points = Vec::new();
        for gy in 0..self.config.height {
            for gx in 0..self.config.width {
                if self.get_cell(gx, gy) == CellState::Occupied {
                    points.push(self.grid_to_world(gx, gy));
                }
            }
        }
        points
    }

    /// Get map as a grayscale image (0=occupied, 128=unknown, 255=free)
    pub fn to_image_data(&self) -> Vec<u8> {
        self.cells
            .iter()
            .map(|&log_odds| {
                let prob = 1.0 / (1.0 + (-log_odds).exp());
                if log_odds.abs() < 0.1 {
                    128 // Unknown
                } else {
                    ((1.0 - prob) * 255.0) as u8 // 0=occupied, 255=free
                }
            })
            .collect()
    }

    /// Get map as RGBA image data for web display
    pub fn to_rgba_image(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.cells.len() * 4);
        for &log_odds in &self.cells {
            let (r, g, b, a) = if log_odds.abs() < 0.1 {
                (128, 128, 128, 255) // Unknown - gray
            } else if log_odds > 0.0 {
                let intensity = ((log_odds / self.config.log_odds_max) * 255.0) as u8;
                (intensity, 0, 0, 255) // Occupied - red
            } else {
                let intensity = ((-log_odds / -self.config.log_odds_min) * 255.0) as u8;
                (255 - intensity, 255 - intensity, 255, 255) // Free - white to light blue
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        rgba
    }

    /// Export map as JSON for web visualization
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "width": self.config.width,
            "height": self.config.height,
            "resolution": self.config.resolution,
            "origin_x": self.origin.x,
            "origin_y": self.origin.y,
            "cells": self.to_image_data(),
            "update_count": self.update_count,
        })
    }

    /// Clear the map
    pub fn clear(&mut self) {
        self.cells.fill(0.0);
        self.update_count = 0;
    }

    /// Get number of updates performed
    pub fn update_count(&self) -> u64 {
        self.update_count
    }

    /// Calculate map entropy (measure of uncertainty)
    pub fn entropy(&self) -> f32 {
        self.cells
            .iter()
            .map(|&log_odds| {
                let p = 1.0 / (1.0 + (-log_odds).exp());
                if p > 0.01 && p < 0.99 {
                    -p * p.ln() - (1.0 - p) * (1.0 - p).ln()
                } else {
                    0.0
                }
            })
            .sum()
    }
}

impl std::fmt::Debug for OccupancyGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OccupancyGrid")
            .field("width", &self.config.width)
            .field("height", &self.config.height)
            .field("resolution", &self.config.resolution)
            .field("origin", &self.origin)
            .field("update_count", &self.update_count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_creation() {
        let config = OccupancyGridConfig::default();
        let grid = OccupancyGrid::new(config);
        assert_eq!(grid.dimensions(), (400, 400));
    }

    #[test]
    fn test_world_grid_conversion() {
        let config = OccupancyGridConfig {
            resolution: 100.0,
            width: 100,
            height: 100,
            ..Default::default()
        };
        let grid = OccupancyGrid::new(config);

        // Center of grid should be at world origin
        let center = grid.world_to_grid(&Point2D::zero());
        assert_eq!(center, Some((50, 50)));

        // Convert back
        let world = grid.grid_to_world(50, 50);
        assert!(world.x.abs() < 60.0);
        assert!(world.y.abs() < 60.0);
    }

    #[test]
    fn test_ray_tracing() {
        let config = OccupancyGridConfig {
            resolution: 50.0, // Smaller cells for more precise ray tracing
            width: 200,
            height: 200,
            log_odds_free: -1.0, // Stronger signal
            log_odds_occupied: 2.0,
            ..Default::default()
        };
        let mut grid = OccupancyGrid::new(config);

        let pose = Pose2D::origin();
        let point = Point2D::new(1000.0, 0.0); // 1m away

        // Multiple scans to strengthen the signal
        for _ in 0..5 {
            grid.update_from_scan(&pose, &[point]);
        }

        // Endpoint should be occupied
        let endpoint_cell = grid.get_cell_at(&point);
        assert_eq!(
            endpoint_cell,
            CellState::Occupied,
            "Endpoint should be occupied"
        );
    }
}
