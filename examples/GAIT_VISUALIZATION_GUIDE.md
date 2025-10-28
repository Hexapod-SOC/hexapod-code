# Hexapod Gait Visualization Guide

## Overview
The `visualize_gait.rs` example has been reworked to showcase the different gaits your hexapod can perform. It now uses the `GaitController` and displays all 6 available gaits in an interactive 3D visualization.

## Available Gaits

The hexapod supports 6 different gaits, each with unique characteristics:

1. **Tripod (tri)** - Fast, stable gait with 3 legs moving at once
   - Default gait
   - Good balance of speed and stability
   - Best for general walking

2. **Wave (wave)** - Slow, sequential leg movement
   - One leg at a time
   - Very stable but slower
   - Good for rough terrain

3. **Ripple (ripple)** - Medium speed with 2 legs moving
   - Good balance of stability and speed
   - Smooth motion pattern

4. **Bipod (bi)** - Fast gait with paired leg movement
   - Higher lift height
   - More dynamic movement
   - Good for faster speeds

5. **Quad (quad)** - Quadruped-style gait
   - Similar to how 4-legged animals walk
   - Good stability

6. **Hop (hop)** - All legs move together
   - Highest lift height
   - Most dynamic
   - Fun to watch!

## Controls

### Movement Controls
- **Arrow Up**: Move forward
- **Arrow Down**: Move backward  
- **Arrow Left**: Strafe left
- **Arrow Right**: Strafe right
- **Q**: Rotate counter-clockwise
- **E**: Rotate clockwise

### Gait Controls
- **G**: Cycle through all 6 gaits
- **P**: Pause/Play gait animation
- **R**: Reset velocity and rotation to zero

### Camera Controls
- **W**: Zoom in
- **S**: Zoom out
- **A**: Rotate camera left
- **D**: Rotate camera right

## Running the Example

To run the gait visualization:

```bash
nix-shell --run "cargo run --example visualize_gait --features dummy --release"
```

The `--release` flag is recommended for better performance.

## What You'll See

- **Hexagon body**: Central body of the hexapod
- **6 Colored legs**: 
  - Red: Coxa segments
  - Green: Femur segments  
  - Blue: Tibia segments
- **Yellow spheres**: Foot positions
- **Ground plane**: Green reference plane
- **UI Display**: Shows current gait name, phase, velocity, and rotation

## Understanding the Display

The UI in the top-right shows:
- **Gait**: Current gait name (and [PAUSED] if paused)
- **Phase**: Current position in the gait cycle (0.0 to 1.0)
- **Velocity**: Forward/strafe speed in mm/s
- **Rotation**: Rotation speed in radians/s

## Tips

1. Start with the default Tripod gait to see stable walking
2. Press **G** to cycle through gaits and compare their leg patterns
3. Use **Arrow Up** to make the hexapod walk forward and see the gait in action
4. Press **P** to pause and examine the leg positions
5. Try different combinations: walk forward while rotating with **Q** or **E**
6. The Hop gait is most dramatic - watch all 6 legs lift together!

## Technical Details

The visualization uses:
- Bevy game engine for 3D rendering
- Your actual IK (Inverse Kinematics) calculations from `movement::ik::SimpleIK`
- Your gait controller from `movement::controller::GaitController`
- Real-time gait phase updates with configurable time step

This means the visualization shows exactly how your physical hexapod will move!
