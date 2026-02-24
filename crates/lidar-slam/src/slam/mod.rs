use crate::map::OccupancyGrid;
use crate::types::{LaserScan, Pose2D, PoseDelta};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

#[derive(Clone, Debug)]
pub struct SlamParams {
    pub map_size_pixels: usize,
    pub map_resolution: f32,
    pub max_range_m: f32,
    pub hole_width_mm: u16,
    pub hit_log_odds: i8,
    pub miss_log_odds: i8,
    pub min_log_odds: i8,
    pub max_log_odds: i8,
    pub search_iters: usize,
    pub sigma_xy: f32,
    pub sigma_theta_rad: f32,
    pub min_score: i32,
    pub update_score_ratio: f32,
    pub sample_step: usize,
    pub heading_prior_weight: f32,
    pub heading_blend: f32,
    pub heading_max_error_rad: f32,
    pub max_rejects_before_reset: u32,
}

impl Default for SlamParams {
    fn default() -> Self {
        Self {
            map_size_pixels: 800,
            map_resolution: 0.05,
            max_range_m: 12.0,
            hole_width_mm: 600,
            hit_log_odds: 6,
            miss_log_odds: -2,
            min_log_odds: -90,
            max_log_odds: 90,
            search_iters: 400,
            sigma_xy: 0.05,
            sigma_theta_rad: 0.05,
            min_score: 0,
            update_score_ratio: 0.5,
            sample_step: 2,
            heading_prior_weight: 0.6,
            heading_blend: 0.2,
            heading_max_error_rad: 0.6,
            max_rejects_before_reset: 8,
        }
    }
}

/// BreezySLAM-inspired 2D SLAM engine; math will mirror the C reference.
pub struct BreezySLAM {
    pose: Pose2D,
    params: SlamParams,
    map: OccupancyGrid,
    rng: SmallRng,
    last_score: i32,
    reject_count: u32,
}

impl BreezySLAM {
    pub fn new(params: SlamParams) -> Self {
        let half_extent = params.map_size_pixels as f32 * params.map_resolution * 0.5;
        let origin = Pose2D {
            x: -half_extent,
            y: -half_extent,
            theta: 0.0,
        };
        let map = OccupancyGrid::new(
            params.map_size_pixels,
            params.map_size_pixels,
            params.map_resolution,
            origin,
        );
        Self {
            pose: Pose2D::default(),
            params,
            map,
            rng: SmallRng::from_entropy(),
            last_score: 0,
            reject_count: 0,
        }
    }

    /// Update SLAM with a new scan and optional odometry delta.
    pub fn update(&mut self, scan: &LaserScan, odom: Option<PoseDelta>) -> Pose2D {
        self.update_with_heading(scan, odom, None)
    }

    /// Update SLAM with a new scan, optional odometry delta, and optional heading hint.
    pub fn update_with_heading(
        &mut self,
        scan: &LaserScan,
        odom: Option<PoseDelta>,
        heading_rad: Option<f32>,
    ) -> Pose2D {
        let mut start = self.pose;
        if let Some(delta) = odom {
            self.apply_odometry_in_place(&mut start, delta);
        }
        if let Some(heading) = heading_rad {
            let diff = angle_diff(heading, start.theta)
                .clamp(-self.params.heading_max_error_rad, self.params.heading_max_error_rad);
            start.theta = wrap_angle(start.theta + diff * self.params.heading_prior_weight);
        }

        let (best_pose, best_score) = self.search_pose(start, scan);
        let mut pose = best_pose;
        if let Some(heading) = heading_rad {
            let diff = angle_diff(heading, pose.theta)
                .clamp(-self.params.heading_max_error_rad, self.params.heading_max_error_rad);
            pose.theta = wrap_angle(pose.theta + diff * self.params.heading_blend);
        }

        self.pose = pose;
        let accept = best_score >= self.params.min_score
            && (self.last_score == 0
                || best_score as f32 >= self.params.update_score_ratio * self.last_score as f32);
        if accept {
            self.integrate_scan_at(scan, pose);
            self.last_score = best_score;
            self.reject_count = 0;
            return self.pose;
        }

        self.reject_count = self.reject_count.saturating_add(1);
        if self.reject_count >= self.params.max_rejects_before_reset {
            self.reset_map(heading_rad);
            self.integrate_scan_at(scan, self.pose);
            self.last_score = best_score;
            self.reject_count = 0;
        }

        self.pose
    }

    pub fn pose(&self) -> Pose2D {
        self.pose
    }

    pub fn map(&self) -> &OccupancyGrid {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut OccupancyGrid {
        &mut self.map
    }

    pub fn params(&self) -> &SlamParams {
        &self.params
    }

    fn apply_odometry_in_place(&self, pose: &mut Pose2D, delta: PoseDelta) {
        pose.x += delta.forward;
        pose.y += delta.sideways;
        pose.theta += delta.dtheta;
        pose.theta = wrap_angle(pose.theta);
    }

    fn integrate_scan_at(&mut self, scan: &LaserScan, pose: Pose2D) {
        for point in &scan.points {
            if point.distance_m <= 0.0 || point.distance_m.is_nan() {
                continue;
            }
            if point.distance_m > self.params.max_range_m {
                continue;
            }

            let beam_theta = pose.theta + point.angle_deg.to_radians();
            let hit_x = pose.x + point.distance_m * beam_theta.cos();
            let hit_y = pose.y + point.distance_m * beam_theta.sin();

            if let Some((gx, gy)) = self.world_to_grid(hit_x, hit_y) {
                // Raytrace frees along the beam.
                if let Some((sx, sy)) = self.world_to_grid(pose.x, pose.y) {
                    self.raytrace_free(sx, sy, gx, gy);
                }
                self.map.add_log_odds(
                    gx,
                    gy,
                    self.params.hit_log_odds,
                    self.params.min_log_odds,
                    self.params.max_log_odds,
                );
            }
        }
    }

    fn search_pose(&mut self, start: Pose2D, scan: &LaserScan) -> (Pose2D, i32) {
        let mut best = start;
        let mut best_score = self.score_scan(&best, scan);

        for _ in 0..self.params.search_iters {
            let mut candidate = best;
            candidate.x += self
                .rng
                .gen_range(-self.params.sigma_xy..self.params.sigma_xy);
            candidate.y += self
                .rng
                .gen_range(-self.params.sigma_xy..self.params.sigma_xy);
            candidate.theta += self
                .rng
                .gen_range(-self.params.sigma_theta_rad..self.params.sigma_theta_rad);
            let score = self.score_scan(&candidate, scan);
            if score > best_score {
                best = candidate;
                best_score = score;
            }
        }
        (best, best_score)
    }

    fn score_scan(&self, pose: &Pose2D, scan: &LaserScan) -> i32 {
        let mut score: i32 = 0;
        for (idx, point) in scan.points.iter().enumerate() {
            if self.params.sample_step > 1 && (idx % self.params.sample_step != 0) {
                continue;
            }
            if point.distance_m <= 0.0 || point.distance_m.is_nan() {
                continue;
            }
            if point.distance_m > self.params.max_range_m {
                continue;
            }
            let beam_theta = pose.theta + point.angle_deg.to_radians();
            let hit_x = pose.x + point.distance_m * beam_theta.cos();
            let hit_y = pose.y + point.distance_m * beam_theta.sin();
            if let Some((gx, gy)) = self.world_to_grid(hit_x, hit_y) {
                if let Some(cell) = self.map.get(gx as usize, gy as usize) {
                    score += cell as i32;
                }
            } else {
                score -= 1;
            }
        }
        score
    }

    fn world_to_grid(&self, x_m: f32, y_m: f32) -> Option<(isize, isize)> {
        let origin = self.map.origin();
        let gx = ((x_m - origin.x) / self.map.resolution()).floor() as isize;
        let gy = ((y_m - origin.y) / self.map.resolution()).floor() as isize;
        if gx < 0 || gy < 0 || gx >= self.map.width() as isize || gy >= self.map.height() as isize {
            None
        } else {
            Some((gx, gy))
        }
    }

    fn raytrace_free(&mut self, x0: isize, y0: isize, x1: isize, y1: isize) {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x == x1 && y == y1 {
                break;
            }
            self.map.add_log_odds(
                x,
                y,
                self.params.miss_log_odds,
                self.params.min_log_odds,
                self.params.max_log_odds,
            );

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }

            if x < 0 || y < 0 || x >= self.map.width() as isize || y >= self.map.height() as isize {
                break;
            }
        }
    }

    fn reset_map(&mut self, heading_rad: Option<f32>) {
        let half_extent = self.params.map_size_pixels as f32 * self.params.map_resolution * 0.5;
        let origin = Pose2D {
            x: -half_extent,
            y: -half_extent,
            theta: 0.0,
        };
        self.map = OccupancyGrid::new(
            self.params.map_size_pixels,
            self.params.map_size_pixels,
            self.params.map_resolution,
            origin,
        );
        self.pose = Pose2D::default();
        if let Some(heading) = heading_rad {
            self.pose.theta = wrap_angle(heading);
        }
        self.last_score = 0;
    }
}

fn wrap_angle(theta: f32) -> f32 {
    let mut t = theta;
    if t > std::f32::consts::PI {
        t -= 2.0 * std::f32::consts::PI;
    } else if t < -std::f32::consts::PI {
        t += 2.0 * std::f32::consts::PI;
    }
    t
}

fn angle_diff(target: f32, current: f32) -> f32 {
    let mut diff = target - current;
    if diff > std::f32::consts::PI {
        diff -= 2.0 * std::f32::consts::PI;
    } else if diff < -std::f32::consts::PI {
        diff += 2.0 * std::f32::consts::PI;
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LaserScan, LidarPoint};

    #[test]
    fn integrates_hit_cell() {
        let params = SlamParams::default();
        let mut slam = BreezySLAM::new(params);
        let scan = LaserScan {
            timestamp_ns: 0,
            rpm: 10.0,
            start_angle_deg: 0.0,
            end_angle_deg: 0.0,
            points: vec![LidarPoint {
                angle_deg: 0.0,
                distance_m: 1.0,
                intensity: 100,
            }],
        };

        let pose_before = slam.pose();
        slam.update(&scan, None);
        let pose_after = slam.pose();
        assert!((pose_before.x - pose_after.x).abs() < f32::EPSILON);

        let map = slam.map();
        let origin = map.origin();
        let gx = ((pose_after.x + 1.0 - origin.x) / map.resolution()).floor() as usize;
        let gy = ((pose_after.y - origin.y) / map.resolution()).floor() as usize;
        let cell = map.get(gx, gy).expect("cell in map");
        assert!(cell > 0);
    }
}
