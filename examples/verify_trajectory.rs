use movement::gait::Gait;
use movement::gaits::GAITS;
use movement::legs::Leg;
use glam::Vec3;

fn main() {
    let template = &GAITS[0]; // Tripod gait
    let mut gait = Gait::new(template);
    
    // Simulate one full cycle (speed multiplier 1.0 for easier math)
    let dt = 0.05;
    let mut t = 0.0;
    
    println!("Time, Phase, Lift(Z)");
    
    // We want to see the swing phase particularly
    // Tripod gait: LeftFront offset is 0.0. Push fraction is 4/6 (~0.66).
    // So 0.0 -> 0.66 is Stance. 0.66 -> 1.0 is Swing.
    
    // Let's run for enough time to cover a full cycle
    while t < 2.0 {
        gait.update(dt);
        let phase = gait.get_phase();
        
        // Assume moving forward
        let velocity = Vec3::new(100.0, 0.0, 0.0);
        let pos = gait.calculate_leg_position(Leg::LeftFront, velocity, 0.0);
        
        // Only print interesting changes or sparse points
        // Or checking specifically the swing phase
        let swing_start = template.push_fraction;
        let leg_phase = (phase + 0.0) % 1.0; // LeftFront offset is 0.0
        
        if leg_phase > swing_start {
             println!("{:.2}, {:.2}, {:.2} (SWING)", t, leg_phase, pos.z);
        } else {
             println!("{:.2}, {:.2}, {:.2} (STANCE)", t, leg_phase, pos.z);
        }
        
        t += dt;
    }
}
