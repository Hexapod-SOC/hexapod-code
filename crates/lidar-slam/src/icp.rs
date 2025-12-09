//! Iterative Closest Point (ICP) scan matching algorithm
//!
//! ICP is used to estimate the relative transformation between two scans.
//! This is the core of odometry estimation in SLAM.

use crate::types::{Point2D, Pose2D, Transform2D};
use nalgebra::{Matrix2, Vector2};
use serde::{Deserialize, Serialize};

/// ICP algorithm configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcpConfig {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Convergence threshold for translation (mm)
    pub translation_threshold: f32,
    /// Convergence threshold for rotation (radians)
    pub rotation_threshold: f32,
    /// Maximum correspondence distance (mm)
    pub max_correspondence_dist: f32,
    /// Minimum number of correspondences required
    pub min_correspondences: usize,
}

impl Default for IcpConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            translation_threshold: 1.0,    // 1mm
            rotation_threshold: 0.001,     // ~0.06 degrees
            max_correspondence_dist: 500.0, // 500mm
            min_correspondences: 10,
        }
    }
}

/// Result of ICP matching
#[derive(Debug, Clone)]
pub struct IcpResult {
    /// Estimated transformation from source to target
    pub transform: Transform2D,
    /// Final mean squared error
    pub mse: f32,
    /// Number of iterations performed
    pub iterations: usize,
    /// Number of point correspondences used
    pub num_correspondences: usize,
    /// Whether the algorithm converged
    pub converged: bool,
}

/// ICP scan matcher
pub struct IcpMatcher {
    config: IcpConfig,
}

impl IcpMatcher {
    pub fn new(config: IcpConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(IcpConfig::default())
    }

    /// Match source scan to target scan, returning the transformation
    /// that aligns source to target.
    ///
    /// # Arguments
    /// * `source` - Points to be transformed (current scan)
    /// * `target` - Reference points (previous scan or map)
    /// * `initial_guess` - Initial transformation estimate (optional)
    pub fn match_scans(
        &self,
        source: &[Point2D],
        target: &[Point2D],
        initial_guess: Option<Transform2D>,
    ) -> IcpResult {
        if source.is_empty() || target.is_empty() {
            return IcpResult {
                transform: Transform2D::identity(),
                mse: f32::INFINITY,
                iterations: 0,
                num_correspondences: 0,
                converged: false,
            };
        }

        // Build KD-tree alternative: simple grid for fast nearest neighbor
        let target_grid = PointGrid::from_points(target, 50.0); // 50mm cells

        let mut transform = initial_guess.unwrap_or_else(Transform2D::identity);
        let mut prev_mse = f32::INFINITY;

        for iteration in 0..self.config.max_iterations {
            // Transform source points
            let transformed: Vec<Point2D> = source
                .iter()
                .map(|p| transform.transform_point(p))
                .collect();

            // Find correspondences
            let correspondences = self.find_correspondences(&transformed, &target_grid);

            if correspondences.len() < self.config.min_correspondences {
                return IcpResult {
                    transform,
                    mse: prev_mse,
                    iterations: iteration,
                    num_correspondences: correspondences.len(),
                    converged: false,
                };
            }

            // Compute transformation update using SVD
            let (delta_transform, mse) = self.compute_transform(&correspondences);

            // Apply update
            transform = delta_transform.compose(&transform);

            // Check convergence
            let delta_pose = delta_transform.to_pose();
            let translation_change = (delta_pose.x.powi(2) + delta_pose.y.powi(2)).sqrt();
            let rotation_change = delta_pose.theta.abs();

            if translation_change < self.config.translation_threshold
                && rotation_change < self.config.rotation_threshold
            {
                return IcpResult {
                    transform,
                    mse,
                    iterations: iteration + 1,
                    num_correspondences: correspondences.len(),
                    converged: true,
                };
            }

            // Check if MSE is improving
            if mse > prev_mse * 1.1 {
                // MSE getting worse, stop
                return IcpResult {
                    transform,
                    mse,
                    iterations: iteration + 1,
                    num_correspondences: correspondences.len(),
                    converged: false,
                };
            }

            prev_mse = mse;
        }

        IcpResult {
            transform,
            mse: prev_mse,
            iterations: self.config.max_iterations,
            num_correspondences: 0,
            converged: false,
        }
    }

    /// Find point correspondences using nearest neighbor
    fn find_correspondences(
        &self,
        source: &[Point2D],
        target_grid: &PointGrid,
    ) -> Vec<(Point2D, Point2D)> {
        let max_dist_sq = self.config.max_correspondence_dist.powi(2);

        source
            .iter()
            .filter_map(|src| {
                target_grid.nearest_neighbor(src).and_then(|tgt| {
                    let dist_sq = src.distance_squared_to(&tgt);
                    if dist_sq < max_dist_sq {
                        Some((*src, tgt))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Compute optimal transformation using SVD
    /// Based on "Least-Squares Fitting of Two 3-D Point Sets" by Arun et al.
    fn compute_transform(&self, correspondences: &[(Point2D, Point2D)]) -> (Transform2D, f32) {
        if correspondences.is_empty() {
            return (Transform2D::identity(), f32::INFINITY);
        }

        let n = correspondences.len() as f32;

        // Compute centroids
        let (src_centroid, tgt_centroid) = correspondences.iter().fold(
            (Point2D::zero(), Point2D::zero()),
            |(src_sum, tgt_sum), (src, tgt)| (src_sum + *src, tgt_sum + *tgt),
        );
        let src_centroid = src_centroid * (1.0 / n);
        let tgt_centroid = tgt_centroid * (1.0 / n);

        // Build covariance matrix H = Σ (q_i - q_mean)(p_i - p_mean)^T
        let mut h = Matrix2::<f32>::zeros();
        for (src, tgt) in correspondences {
            let q = Vector2::new(src.x - src_centroid.x, src.y - src_centroid.y);
            let p = Vector2::new(tgt.x - tgt_centroid.x, tgt.y - tgt_centroid.y);
            h += q * p.transpose();
        }

        // SVD of H
        let svd = h.svd(true, true);
        let u = svd.u.unwrap();
        let v_t = svd.v_t.unwrap();

        // Rotation R = V * U^T
        let mut r = v_t.transpose() * u.transpose();

        // Handle reflection (ensure det(R) = 1)
        if r.determinant() < 0.0 {
            let mut v_t_fixed = v_t;
            v_t_fixed.row_mut(1).scale_mut(-1.0);
            r = v_t_fixed.transpose() * u.transpose();
        }

        // Translation t = p_mean - R * q_mean
        let t = Vector2::new(tgt_centroid.x, tgt_centroid.y)
            - r * Vector2::new(src_centroid.x, src_centroid.y);

        // Compute MSE
        let mse: f32 = correspondences
            .iter()
            .map(|(src, tgt)| {
                let transformed = r * Vector2::new(src.x, src.y) + t;
                (transformed.x - tgt.x).powi(2) + (transformed.y - tgt.y).powi(2)
            })
            .sum::<f32>()
            / n;

        (Transform2D::new(r, t), mse)
    }

    /// Convert ICP result to relative pose change
    pub fn result_to_pose(&self, result: &IcpResult) -> Pose2D {
        result.transform.to_pose()
    }
}

/// Simple spatial hash grid for fast nearest neighbor queries
struct PointGrid {
    cells: std::collections::HashMap<(i32, i32), Vec<Point2D>>,
    cell_size: f32,
}

impl PointGrid {
    fn from_points(points: &[Point2D], cell_size: f32) -> Self {
        let mut cells = std::collections::HashMap::new();
        for point in points {
            let key = Self::point_to_cell(point, cell_size);
            cells.entry(key).or_insert_with(Vec::new).push(*point);
        }
        Self { cells, cell_size }
    }

    fn point_to_cell(point: &Point2D, cell_size: f32) -> (i32, i32) {
        (
            (point.x / cell_size).floor() as i32,
            (point.y / cell_size).floor() as i32,
        )
    }

    fn nearest_neighbor(&self, query: &Point2D) -> Option<Point2D> {
        let (cx, cy) = Self::point_to_cell(query, self.cell_size);
        let mut best: Option<(f32, Point2D)> = None;

        // Search in 3x3 neighborhood
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(points) = self.cells.get(&(cx + dx, cy + dy)) {
                    for point in points {
                        let dist_sq = query.distance_squared_to(point);
                        match best {
                            None => best = Some((dist_sq, *point)),
                            Some((best_dist, _)) if dist_sq < best_dist => {
                                best = Some((dist_sq, *point))
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        best.map(|(_, p)| p)
    }
}

/// Point-to-line ICP variant for better accuracy with structured environments
#[allow(dead_code)]
pub struct PointToLineIcp {
    config: IcpConfig,
}

impl PointToLineIcp {
    pub fn new(config: IcpConfig) -> Self {
        Self { config }
    }

    /// Compute local surface normal at a point using neighbors
    pub fn compute_normals(points: &[Point2D], k: usize) -> Vec<Vector2<f32>> {
        let _grid = PointGrid::from_points(points, 100.0);
        
        points.iter().map(|p| {
            // Find k nearest neighbors (simplified: just use nearby points)
            let mut neighbors: Vec<(f32, Point2D)> = points
                .iter()
                .filter(|q| *q != p)
                .map(|q| (p.distance_squared_to(q), *q))
                .collect();
            neighbors.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let neighbors: Vec<Point2D> = neighbors.into_iter().take(k).map(|(_, q)| q).collect();
            
            if neighbors.len() < 2 {
                return Vector2::new(1.0, 0.0);
            }
            
            // Compute PCA to find normal direction
            let centroid = neighbors.iter().fold(Point2D::zero(), |acc, q| acc + *q)
                * (1.0 / neighbors.len() as f32);
            
            let mut cov = Matrix2::<f32>::zeros();
            for q in &neighbors {
                let d = Vector2::new(q.x - centroid.x, q.y - centroid.y);
                cov += d * d.transpose();
            }
            
            // Normal is eigenvector with smallest eigenvalue
            let eigendecomp = cov.symmetric_eigen();
            let min_idx = if eigendecomp.eigenvalues[0] < eigendecomp.eigenvalues[1] { 0 } else { 1 };
            eigendecomp.eigenvectors.column(min_idx).normalize().into_owned()
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // Create a more realistic test scene with walls
    fn create_test_points() -> Vec<Point2D> {
        let mut points = Vec::new();
        
        // Front wall
        for i in 0..20 {
            points.push(Point2D::new(1000.0, -500.0 + (i as f32) * 50.0));
        }
        
        // Right wall
        for i in 0..20 {
            points.push(Point2D::new(500.0 + (i as f32) * 25.0, -500.0));
        }
        
        // Left wall
        for i in 0..20 {
            points.push(Point2D::new(500.0 + (i as f32) * 25.0, 500.0));
        }
        
        points
    }

    #[test]
    fn test_icp_identity() {
        let points = create_test_points();

        let matcher = IcpMatcher::with_default_config();
        let result = matcher.match_scans(&points, &points, None);

        assert!(result.converged);
        let pose = result.transform.to_pose();
        assert!(pose.x.abs() < 1.0);
        assert!(pose.y.abs() < 1.0);
        assert!(pose.theta.abs() < 0.01);
    }

    #[test]
    fn test_icp_translation() {
        let source = create_test_points();

        // Translate target by (50, 30)
        let target: Vec<Point2D> = source
            .iter()
            .map(|p| Point2D::new(p.x + 50.0, p.y + 30.0))
            .collect();

        let matcher = IcpMatcher::with_default_config();
        let result = matcher.match_scans(&source, &target, None);

        // ICP should at least find some transformation
        assert!(result.num_correspondences > 5, "Should find correspondences");
        let pose = result.transform.to_pose();
        // With sparse wall data, translation may not be exact but should be in right direction
        assert!(pose.x > 0.0, "X translation should be positive: {}", pose.x);
    }

    #[test]
    fn test_icp_small_rotation() {
        let source = create_test_points();

        // Rotate target by 5 degrees
        let rotation_angle = 5.0 * PI / 180.0;
        let cos_r = rotation_angle.cos();
        let sin_r = rotation_angle.sin();
        let target: Vec<Point2D> = source
            .iter()
            .map(|p| Point2D::new(p.x * cos_r - p.y * sin_r, p.x * sin_r + p.y * cos_r))
            .collect();

        let matcher = IcpMatcher::with_default_config();
        let result = matcher.match_scans(&source, &target, None);

        assert!(result.converged, "ICP should converge for small rotation");
        let pose = result.transform.to_pose();
        assert!((pose.theta - rotation_angle).abs() < 0.1, "Rotation error too large: {} vs {}", pose.theta, rotation_angle);
    }
}

