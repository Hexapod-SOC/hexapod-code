//! Near-range filtering for LiDAR data
//!
//! Filters out unreliable close-range measurements based on intensity and grouping

use super::point::Point;

const INTENSITY_LOW: u8 = 15;
const INTENSITY_SINGLE: u8 = 220;
const SCAN_FREQUENCY: f32 = 4500.0;

/// Near-range point filter
pub struct NearRangeFilter {
    near_range_threshold: u16, // millimeters
}

impl NearRangeFilter {
    /// Create a new filter with default 5m threshold
    pub fn new() -> Self {
        Self {
            near_range_threshold: 5000,
        }
    }

    /// Create a filter with custom threshold
    pub fn with_threshold(threshold: u16) -> Self {
        Self {
            near_range_threshold: threshold,
        }
    }

    /// Filter points, removing unreliable near-range measurements
    pub fn filter(&self, points: &[Point], speed: u16) -> Vec<Point> {
        let mut normal = Vec::new();
        let mut pending = Vec::new();

        // Separate near and far points
        for &point in points {
            if point.distance < self.near_range_threshold {
                pending.push(point);
            } else {
                normal.push(point);
            }
        }

        if pending.is_empty() {
            return normal;
        }

        // Calculate angular grouping threshold
        let angle_delta_limit = (speed as f32) / SCAN_FREQUENCY * 2.0;

        // Sort pending points by angle
        pending.sort_by(|a, b| a.angle.partial_cmp(&b.angle).unwrap());

        // Group nearby points
        let groups = self.group_points(&pending, angle_delta_limit);

        // Handle wrap-around at 0/360 degrees
        let groups = self.merge_wraparound_groups(groups, angle_delta_limit);

        // Filter groups and add valid points to normal
        for group in groups {
            let filtered = self.filter_group(&group);
            normal.extend(filtered);
        }

        normal
    }

    /// Group points that are close in angle and distance
    fn group_points(&self, points: &[Point], angle_threshold: f32) -> Vec<Vec<Point>> {
        let mut groups: Vec<Vec<Point>> = Vec::new();
        let mut current_group = Vec::new();
        let mut last_point: Option<Point> = None;

        for &point in points {
            if let Some(last) = last_point {
                let angle_diff = (point.angle - last.angle).abs();
                let dist_diff = (point.distance as i32 - last.distance as i32).abs();
                let dist_threshold = (last.distance as f32 * 0.03) as i32;

                if angle_diff > angle_threshold || dist_diff > dist_threshold {
                    if !current_group.is_empty() {
                        groups.push(current_group.clone());
                        current_group.clear();
                    }
                }
            }

            current_group.push(point);
            last_point = Some(point);
        }

        if !current_group.is_empty() {
            groups.push(current_group);
        }

        groups
    }

    /// Merge first and last groups if they connect across 0/360 degree boundary
    fn merge_wraparound_groups(
        &self,
        mut groups: Vec<Vec<Point>>,
        angle_threshold: f32,
    ) -> Vec<Vec<Point>> {
        if groups.len() < 2 {
            return groups;
        }

        let first_angle = groups[0][0].angle;
        let last_angle = groups[groups.len() - 1].last().unwrap().angle;
        let first_dist = groups[0][0].distance;
        let last_dist = groups[groups.len() - 1].last().unwrap().distance;

        let angle_wrap_diff = (first_angle + 360.0 - last_angle).abs();
        let dist_diff = (first_dist as i32 - last_dist as i32).abs();
        let dist_threshold = (last_dist as f32 * 0.03) as i32;

        if angle_wrap_diff < angle_threshold && dist_diff < dist_threshold {
            // Merge last group into first
            let last_group = groups.pop().unwrap();
            groups[0].splice(0..0, last_group);
        }

        groups
    }

    /// Filter a single group of points
    fn filter_group(&self, group: &[Point]) -> Vec<Point> {
        if group.is_empty() {
            return Vec::new();
        }

        // Large groups pass through
        if group.len() > 15 {
            return group.to_vec();
        }

        let mut result = group.to_vec();

        // Small groups need intensity validation
        if group.len() < 3 {
            let avg_intensity: u32 = group.iter().map(|p| p.intensity as u32).sum::<u32>() / group.len() as u32;
            
            if avg_intensity < INTENSITY_SINGLE as u32 {
                // Mark as invalid
                for point in &mut result {
                    *point = Point::new(point.angle, 0, 0);
                }
            }
        } else {
            // Medium groups - check average intensity
            let avg_intensity: u32 = group.iter().map(|p| p.intensity as u32).sum::<u32>() / group.len() as u32;

            if avg_intensity <= INTENSITY_LOW as u32 {
                // Mark as invalid
                for point in &mut result {
                    *point = Point::new(point.angle, 0, 0);
                }
            }
        }

        result
    }
}

impl Default for NearRangeFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_creation() {
        let filter = NearRangeFilter::new();
        assert_eq!(filter.near_range_threshold, 5000);
    }

    #[test]
    fn test_filter_far_points() {
        let filter = NearRangeFilter::new();
        let points = vec![
            Point::new(10.0, 6000, 128),
            Point::new(20.0, 7000, 128),
        ];
        let filtered = filter.filter(&points, 3600);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_low_intensity() {
        let filter = NearRangeFilter::new();
        let points = vec![
            Point::new(10.0, 1000, 10), // Low intensity
            Point::new(11.0, 1010, 10),
        ];
        let filtered = filter.filter(&points, 3600);
        // Should be filtered out (marked as distance 0)
        assert!(filtered.iter().all(|p| p.distance == 0));
    }

    #[test]
    fn test_filter_high_intensity_group() {
        let filter = NearRangeFilter::new();
        let points = vec![
            Point::new(10.0, 1000, 200),
            Point::new(11.0, 1010, 200),
            Point::new(12.0, 1020, 200),
            Point::new(13.0, 1030, 200),
        ];
        let filtered = filter.filter(&points, 3600);
        // Should pass through
        assert!(filtered.iter().all(|p| p.distance > 0));
    }
}
