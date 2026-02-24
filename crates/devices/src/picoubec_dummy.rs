use std::time::Instant;

/// Battery and power status information
#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    pub voltage: f32,
    pub current: f32,
    pub last_update: Option<Instant>,
}

impl Default for BatteryStatus {
    fn default() -> Self {
        Self {
            voltage: 7.4, // Default simulated voltage
            current: 1.5, // Default simulated current
            last_update: Some(Instant::now()),
        }
    }
}

/// Power system state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerState {
    Normal,
    LowBatteryWarning { timeout_seconds: u32 },
    Critical,
    ShuttingDown { remaining_seconds: u32 },
}

pub struct PicoUbecController {
    battery_status: BatteryStatus,
    power_state: PowerState,
}

impl PicoUbecController {
    pub fn new(_port_path: &str) -> Self {
        println!("(Dummy) Initializing UBEC controller...");
        Self {
            battery_status: BatteryStatus::default(),
            power_state: PowerState::Normal,
        }
    }

    /// Update battery status by reading from UART
    /// Returns true if new data was received
    pub fn update(&mut self) -> bool {
        // Simulate voltage drift for testing
        self.battery_status.voltage += (rand::random::<f32>() - 0.5) * 0.01;
        self.battery_status.voltage = self.battery_status.voltage.clamp(6.0, 8.4);

        self.battery_status.current += (rand::random::<f32>() - 0.5) * 0.05;
        self.battery_status.current = self.battery_status.current.clamp(0.5, 3.0);

        self.battery_status.last_update = Some(Instant::now());

        println!(
            "(Dummy) Battery: {:.2}V, {:.2}A - State: {:?}",
            self.battery_status.voltage, self.battery_status.current, self.power_state
        );

        true
    }

    /// Get current battery status
    pub fn get_battery_status(&self) -> BatteryStatus {
        self.battery_status
    }

    /// Get current power state
    pub fn get_power_state(&self) -> PowerState {
        self.power_state
    }

    /// Check if battery voltage is critically low
    pub fn is_critical(&self) -> bool {
        matches!(self.power_state, PowerState::Critical)
    }

    /// Send shutdown command to UBEC
    pub fn send_shutdown(&mut self, delay_seconds: u32) {
        println!("(Dummy) Shutdown scheduled in {} seconds", delay_seconds);
        self.power_state = PowerState::ShuttingDown {
            remaining_seconds: delay_seconds,
        };
    }

    /// Check if UART connection is available
    pub fn is_connected(&self) -> bool {
        true
    }

    /// Enable servos via UART command
    pub fn enable_servos(&mut self) {
        println!("(Dummy) Enable servos command sent");
    }

    /// Disable servos via UART command
    pub fn disable_servos(&mut self) {
        println!("(Dummy) Disable servos command sent");
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

    pub fn random<T: From<f32>>() -> T {
        SEED.with(|seed| {
            let mut s = seed.get();
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            seed.set(s);
            T::from(s as f32 / u64::MAX as f32)
        })
    }
}
