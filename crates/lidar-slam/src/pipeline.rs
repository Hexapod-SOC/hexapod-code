use crate::driver::{DriverResult, Ld19Parser};
use crate::slam::{BreezySLAM, SlamParams};
use crate::types::{LaserScan, Pose2D, PoseDelta};

/// Convenience wrapper wiring the LD19 parser into the SLAM engine.
pub struct LidarSlam {
    pub parser: Ld19Parser,
    pub slam: BreezySLAM,
}

impl LidarSlam {
    pub fn new(params: SlamParams) -> Self {
        Self {
            parser: Ld19Parser::new(),
            slam: BreezySLAM::new(params),
        }
    }

    /// Feed raw bytes from the LD19 and optional odometry; returns poses for
    /// any completed scans integrated into the map.
    pub fn ingest(
        &mut self,
        data: &[u8],
        odom: Option<PoseDelta>,
    ) -> DriverResult<Vec<(Pose2D, LaserScan)>> {
        self.ingest_with_heading(data, odom, None)
    }

    /// Feed raw bytes from the LD19 with optional odometry and heading hint.
    pub fn ingest_with_heading(
        &mut self,
        data: &[u8],
        odom: Option<PoseDelta>,
        heading_rad: Option<f32>,
    ) -> DriverResult<Vec<(Pose2D, LaserScan)>> {
        let scans = self.parser.ingest_bytes(data)?;
        let mut results = Vec::with_capacity(scans.len());
        for scan in scans {
            let pose = self.slam.update_with_heading(&scan, odom, heading_rad);
            results.push((pose, scan));
        }
        Ok(results)
    }

    pub fn map(&self) -> &crate::map::OccupancyGrid {
        self.slam.map()
    }

    pub fn map_mut(&mut self) -> &mut crate::map::OccupancyGrid {
        self.slam.map_mut()
    }

    pub fn pose(&self) -> Pose2D {
        self.slam.pose()
    }
}
