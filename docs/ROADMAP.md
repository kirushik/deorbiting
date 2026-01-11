# Implementation Roadmap

## Phase 1: The Static Universe
1.  **Setup Bevy:** Initialize app, `Camera3dBundle` (Orthographic), and basic systems.
2.  **Ephemeris System:** Implement the `KeplerOrbit` math (Newton's method solver).
3.  **Planet Rendering:** Spawn a Sun and the planets. Use `Ephemeris` to update their `BodyState` and `Transform` every frame. Verify they orbit correctly. Use today's date as the starting point. Draw them as simple spheres, colored to resemble their real-life counterparts.
4.  **Major moons:** add Earth's moon, Jupiter's Galilean moons, and Saturn's Titan. Make sure rings are present (visual-only) for Saturn, Uranus, and Neptune.
5. **Visual spice:** add simple lighting and a starfield background.

## Phase 2: The "Split-World" (Visual Distortion) and GUI
1.  **Time Control:** Use `bevy_egui`; add a panel at the bottom, with play/pause buttons, current time display/input, and time scaling (1x, 10x, 100x).
2.  **Distortion Algo:** Implement `apply_visual_distortion`.

## Phase 3: The Asteroid Physics
1.  **Asteroid Setup:** Create a simple asteroid entity with `BodyState` (pos, vel, mass). Make sure it's rendered as a small gray sphere. Place it initially near Earth.
2.  **Sync System:** Update the `sync_visuals` system to apply distortion to the asteroid's `Transform`.
3.  **Integrator:** Implement the **Velocity Verlet** system in `FixedUpdate`.
    *   Hardcode a starting velocity for the asteroid.
    *   Ensure it orbits the Earth (moon-like behavior).
4.  **Input:** Add mouse dragging to impart velocity (Vector UI) to the asteroid.

## Phase 4: Trajectory Prediction
1.  **Prediction Resource:** Create a resource to store simulation settings (max steps, time step).
2.  **Simulation Loop:** Create a system that runs the Verlet loop on a *clone* of the asteroid's state.
3.  **Line Rendering:** Use Bevy Gizmos to draw lines between predicted points.
4.  **Distortion integration:** Ensure the predicted points are passed through `apply_visual_distortion` before being drawn, so the line matches the distorted asteroid visuals.
