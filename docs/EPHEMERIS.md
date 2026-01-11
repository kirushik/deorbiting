# Ephemeris & Orbital Elements

## 1. Data Source
We will not use N-Body simulation for planets (too unstable for a game background). We will use **Analytic Keplerian Orbits**.
*   **Source:** [NASA JPL Horizons System](https://ssd.jpl.nasa.gov/horizons/).
*   **Simplified Model:** For 2D, we ignore Inclination ($i$) and Longitude of Ascending Node ($\Omega$).

## 2. The Math (Kepler Solver)
To get a planet's position at time $t$:

1.  **Mean Anomaly ($M$):** How far along the orbit (in radians) if speed were constant.
    $$M(t) = M_0 + n(t - t_0)$$
    *(Where $n$ is mean motion, $M_0$ is angle at epoch).*

2.  **Eccentric Anomaly ($E$):** The geometric angle. Solved via **Newton's Method** (Kepler's Equation):
    $$M = E - e \sin(E)$$
    *Iterate $E_{next} = E - (E - e \sin(E) - M) / (1 - e \cos(E))$ until convergence.*

3.  **True Anomaly ($\nu$):** The actual angle from the periapsis (closest point).
    $$\tan(\nu/2) = \sqrt{\frac{1+e}{1-e}} \tan(E/2)$$

4.  **Radius ($r$):** Distance from star.
    $$r = a(1 - e \cos(E))$$
    *(Where $a$ is semi-major axis, $e$ is eccentricity).*

5.  **Position ($x, y$):**
    $$x = r \cos(\nu)$$
    $$y = r \sin(\nu)$$

## 3. Implementation Strategy
Create a `KeplerOrbit` struct.
```rust
struct KeplerOrbit {
    semi_major_axis: f32, // a
    eccentricity: f32,    // e
    argument_of_periapsis: f32, // w (rotation of the oval)
    mean_anomaly_at_epoch: f32, // M0
    mean_motion: f32,     // n (speed)
}

impl KeplerOrbit {
    fn get_position(&self, time: f32) -> Vec2 { ... }
}
