use crate::types::Pose2D;

/// Occupancy grid storing log-odds values.
#[derive(Clone, Debug)]
pub struct OccupancyGrid {
    width: usize,
    height: usize,
    resolution: f32,
    origin: Pose2D,
    cells: Vec<i8>,
}

impl OccupancyGrid {
    pub fn new(width: usize, height: usize, resolution: f32, origin: Pose2D) -> Self {
        let cells = vec![0i8; width * height];
        Self {
            width,
            height,
            resolution,
            origin,
            cells,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn resolution(&self) -> f32 {
        self.resolution
    }

    pub fn origin(&self) -> Pose2D {
        self.origin
    }

    pub fn origin_xy(&self) -> (f32, f32) {
        (self.origin.x, self.origin.y)
    }

    pub fn add_log_odds(&mut self, x: isize, y: isize, delta: i8, min: i8, max: i8) {
        if x < 0 || y < 0 {
            return;
        }
        let (xu, yu) = (x as usize, y as usize);
        if let Some(idx) = self.index_of(xu, yu) {
            let new_val =
                (self.cells[idx] as i16 + delta as i16).clamp(min as i16, max as i16) as i8;
            self.cells[idx] = new_val;
        }
    }

    pub fn index_of(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(y * self.width + x)
        } else {
            None
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<i8> {
        self.index_of(x, y).map(|i| self.cells[i])
    }

    pub fn set(&mut self, x: usize, y: usize, value: i8) {
        if let Some(i) = self.index_of(x, y) {
            self.cells[i] = value;
        }
    }

    pub fn cells(&self) -> &[i8] {
        &self.cells
    }
}
