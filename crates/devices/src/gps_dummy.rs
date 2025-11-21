use std::time::Instant;

/// GPS position data
#[derive(Debug, Clone, Copy)]
pub struct GpsPosition {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    pub speed_kmh: f32,
    pub heading: Option<f32>,
    pub satellites: u8,
    pub fix_quality: FixQuality,
    pub last_update: Option<Instant>,
}

impl Default for GpsPosition {
    fn default() -> Self {
        Self {
            latitude: 48.600633,  // Sample latitude (Bratislava area)
            longitude: 18.085552, // Sample longitude
            altitude: 208.3,
            speed_kmh: 0.0,
            heading: None,
            satellites: 32, // Simulated satellite count
            fix_quality: FixQuality::GpsFix,
            last_update: Some(Instant::now()),
        }
    }
}

/// GPS fix quality indicator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FixQuality {
    NoFix,
    GpsFix,
    DifferentialFix,
    PpsFix,
    RtkFixed,
    RtkFloat,
    Estimated,
    Manual,
    Simulation,
}

impl From<u8> for FixQuality {
    fn from(val: u8) -> Self {
        match val {
            0 => FixQuality::NoFix,
            1 => FixQuality::GpsFix,
            2 => FixQuality::DifferentialFix,
            3 => FixQuality::PpsFix,
            4 => FixQuality::RtkFixed,
            5 => FixQuality::RtkFloat,
            6 => FixQuality::Estimated,
            7 => FixQuality::Manual,
            8 => FixQuality::Simulation,
            _ => FixQuality::NoFix,
        }
    }
}

pub struct GpsController {
    position: GpsPosition,
    update_count: u32,
}

impl GpsController {
    pub fn new(_port_path: &str) -> Self {
        println!("(Dummy) Initializing GPS controller...");
        Self {
            position: GpsPosition::default(),
            update_count: 0,
        }
    }

    /// Update GPS position by reading from UART
    /// Returns true if new data was received
    pub fn update(&mut self) -> bool {
        self.update_count += 1;

        // Simulate slight movement
        self.position.latitude += (rand::random::<f64>() - 0.5) * 0.00001; // ~1m drift
        self.position.longitude += (rand::random::<f64>() - 0.5) * 0.00001;
        
        self.position.altitude += (rand::random::<f32>() - 0.5) * 0.1;
        self.position.altitude = self.position.altitude.clamp(200.0, 220.0);
        
        // Simulate speed variation
        self.position.speed_kmh += (rand::random::<f32>() - 0.5) * 0.5;
        self.position.speed_kmh = self.position.speed_kmh.clamp(0.0, 2.0);
        
        // Simulate heading if moving
        if self.position.speed_kmh > 0.1 {
            let heading = self.position.heading.unwrap_or(0.0);
            self.position.heading = Some((heading + (rand::random::<f32>() - 0.5) * 10.0).rem_euclid(360.0));
        }
        
        self.position.last_update = Some(Instant::now());
        
        // Print every 10 updates to reduce spam
        if self.update_count % 10 == 0 {
            println!(
                "(Dummy) GPS: {:.6}°, {:.6}° | Alt: {:.1}m | Spd: {:.2} km/h | Sats: {} | Fix: {:?}",
                self.position.latitude,
                self.position.longitude,
                self.position.altitude,
                self.position.speed_kmh,
                self.position.satellites,
                self.position.fix_quality
            );
        }
        
        true
    }

    /// Get current GPS position
    pub fn get_position(&self) -> GpsPosition {
        self.position
    }

    /// Check if GPS has a valid fix
    pub fn has_fix(&self) -> bool {
        self.position.fix_quality != FixQuality::NoFix
            && self.position.satellites > 0
    }

    /// Check if UART connection is available
    pub fn is_connected(&self) -> bool {
        true // Always connected in dummy mode
    }
}

// Simple random function for dummy implementation
mod rand {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};

    thread_local! {
        static SEED: Cell<u64> = Cell::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
    }

    pub fn random<T>() -> T
    where
        T: From<f32> + From<f64>,
    {
        SEED.with(|seed| {
            let mut s = seed.get();
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            seed.set(s);
            
            // Generate a value between 0.0 and 1.0
            let val = (s as f64) / (u64::MAX as f64);
            
            // Try to convert to the target type
            // This is a simplified approach - for production use proper traits
            if std::mem::size_of::<T>() == std::mem::size_of::<f32>() {
                T::from(val as f32)
            } else {
                T::from(val)
            }
        })
    }
}
