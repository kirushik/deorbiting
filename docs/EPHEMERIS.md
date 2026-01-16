# Ephemeris & Orbital Elements

## 1. Data Source

We use tables exported by JPL Horizons, falling back to **Analytic Keplerian Orbits** (with alignment to the table-provided data at the boundary, to avoid trajectory discontinuities) rather than N-body simulation for planets (too unstable for a game background).

 - **Source:** [NASA JPL Horizons System](https://ssd.jpl.nasa.gov/horizons/)
 - **Simplified Model:** For 2D, we ignore Inclination ($i$) and Longitude of Ascending Node ($\Omega$)

## 2. Reference Epoch & Simulation Time

- **Epoch:** J2000.0 (January 1, 2000, 12:00 TT)
- All ephemeris data is referenced to the J2000 ecliptic plane.
- **Simulation time origin:** `t = 0` is J2000.0.
- Time is measured as **seconds since J2000** (stored as `f64`).

### Time scale / TT vs UTC
The project treats J2000 seconds as a *uniform* game time axis. We intentionally ignore:
- leap seconds
- TT/UTC (or TT/TDB) offsets

This is accurate enough for a game and avoids pulling in heavy time libraries.

### Simulation coverage / constraints
To keep the runtime ephemeris simple and reproducible, the table-based ephemeris is generated for a fixed window starting at J2000:

- **Coverage:** from `t = 0` (J2000) forward for ~500–600 years.
- Negative times are intentionally **not supported** by the table ephemeris.

### Frame / origin (important)
The simulation uses a **heliocentric 2D frame**:

- **Origin:** Sun at `(0, 0)`
- **Plane:** J2000 ecliptic
- **Coordinates:** `x,y` in meters

All bodies (planets and major moons) are provided in this same heliocentric frame for simplicity.

### J2000 Epoch Constants
```rust
// J2000.0 = January 1, 2000, 12:00 TT
// Unix timestamp: 946728000 (approximately)
const J2000_UNIX: i64 = 946728000;

// Seconds per day
const SECONDS_PER_DAY: f64 = 86400.0;

fn unix_to_j2000_seconds(unix_timestamp: i64) -> f64 {
    (unix_timestamp - J2000_UNIX) as f64
}
```

## 3. The Math (Kepler Solver)

To get a planet's position at time $t$:

### Step 1: Mean Anomaly ($M$)
How far along the orbit (in radians) if speed were constant:

$$M(t) = M_0 + n(t - t_0)$$

Where $n$ is mean motion (rad/s), $M_0$ is mean anomaly at epoch.

### Step 2: Eccentric Anomaly ($E$)
The geometric angle. Solved via **Newton's Method** (Kepler's Equation):

$$M = E - e \sin(E)$$

Iterate until convergence:
$$E_{next} = E - \frac{E - e \sin(E) - M}{1 - e \cos(E)}$$

### Step 3: True Anomaly ($\nu$)
The actual angle from periapsis (closest point):

$$\tan(\nu/2) = \sqrt{\frac{1+e}{1-e}} \tan(E/2)$$

### Step 4: Radius ($r$)
Distance from the central body:

$$r = a(1 - e \cos(E))$$

Where $a$ is semi-major axis, $e$ is eccentricity.

### Step 5: Position ($x, y$)
Rotate by argument of periapsis ($\omega$):

$$x = r \cos(\nu + \omega)$$
$$y = r \sin(\nu + \omega)$$

## 4. Implementation

### KeplerOrbit Struct
```rust
#[derive(Clone, Debug)]
struct KeplerOrbit {
    semi_major_axis: f64,        // a (meters)
    eccentricity: f64,           // e (dimensionless, 0 ≤ e < 1 for ellipse)
    argument_of_periapsis: f64,  // ω (radians)
    mean_anomaly_at_epoch: f64,  // M₀ (radians)
    mean_motion: f64,            // n (radians per second)
    parent: Option<Entity>,      // None for heliocentric, Some for moons
}

impl KeplerOrbit {
    /// Solve Kepler's equation using Newton's method
    fn solve_eccentric_anomaly(&self, mean_anomaly: f64) -> f64 {
        let mut e_anomaly = mean_anomaly;  // Initial guess

        for _ in 0..50 {  // Max iterations
            let delta = (e_anomaly - self.eccentricity * e_anomaly.sin() - mean_anomaly)
                      / (1.0 - self.eccentricity * e_anomaly.cos());
            e_anomaly -= delta;

            if delta.abs() < 1e-12 {
                break;
            }
        }

        e_anomaly
    }

    /// Get position relative to parent body at given time
    fn get_local_position(&self, time: f64) -> DVec2 {
        // Mean anomaly at time t
        let mean_anomaly = self.mean_anomaly_at_epoch + self.mean_motion * time;

        // Solve for eccentric anomaly
        let e_anomaly = self.solve_eccentric_anomaly(mean_anomaly);

        // True anomaly
        let true_anomaly = 2.0 * ((1.0 + self.eccentricity).sqrt()
            * (e_anomaly / 2.0).tan())
            .atan2((1.0 - self.eccentricity).sqrt());

        // Radius
        let radius = self.semi_major_axis * (1.0 - self.eccentricity * e_anomaly.cos());

        // Position (rotated by argument of periapsis)
        let angle = true_anomaly + self.argument_of_periapsis;
        DVec2::new(radius * angle.cos(), radius * angle.sin())
    }

    /// Get absolute position (handles hierarchical orbits)
    fn get_position(&self, time: f64, ephemeris: &Ephemeris) -> DVec2 {
        let local_pos = self.get_local_position(time);

        match self.parent {
            None => local_pos,  // Heliocentric orbit
            Some(parent_entity) => {
                // Moon orbit: add parent planet's position
                let parent_pos = ephemeris.get_position(parent_entity, time);
                parent_pos + local_pos
            }
        }
    }
}
```

## 5. Hierarchical Orbits (Moons)

Moons use parent-relative Keplerian orbits. Their absolute position is computed as:

```
moon_absolute_pos = planet_pos + moon_local_orbit_pos
```

This is handled automatically by setting the `parent` field in `KeplerOrbit`.

**Important: Moons are decorative only.** They are rendered for visual interest but:
- Do NOT contribute to gravity calculations (only Sun + 8 planets)
- Have NO collision detection (asteroids pass through them)

This simplifies the physics model while keeping the simulation educationally accurate.

### Example: Earth's Moon
```rust
let moon_orbit = KeplerOrbit {
    semi_major_axis: 384_400_000.0,  // 384,400 km in meters
    eccentricity: 0.0549,
    argument_of_periapsis: /* ... */,
    mean_anomaly_at_epoch: /* ... */,
    mean_motion: 2.662e-6,  // ~27.3 day period
    parent: Some(earth_entity),  // Orbits Earth, not Sun
};
```

## 6. Orbital Elements (J2000)

### Planets (Heliocentric)

| Body    | a (AU)    | a (m)           | e       | ω (deg)  | M₀ (deg) | n (°/day) | Mass (kg)      |
|---------|-----------|-----------------|---------|----------|----------|-----------|----------------|
| Sun     | 0         | 0               | 0       | 0        | 0        | 0         | 1.989e30       |
| Mercury | 0.387     | 5.791e10        | 0.2056  | 29.12    | 174.79   | 4.0923    | 3.302e23       |
| Venus   | 0.723     | 1.082e11        | 0.0068  | 54.85    | 50.42    | 1.6021    | 4.869e24       |
| Earth   | 1.000     | 1.496e11        | 0.0167  | 102.94   | 357.53   | 0.9856    | 5.972e24       |
| Mars    | 1.524     | 2.279e11        | 0.0934  | 286.50   | 19.41    | 0.5240    | 6.417e23       |
| Jupiter | 5.203     | 7.785e11        | 0.0484  | 273.87   | 20.02    | 0.0831    | 1.898e27       |
| Saturn  | 9.537     | 1.427e12        | 0.0542  | 339.39   | 317.02   | 0.0335    | 5.683e26       |
| Uranus  | 19.19     | 2.871e12        | 0.0472  | 96.99    | 142.24   | 0.0117    | 8.681e25       |
| Neptune | 30.07     | 4.498e12        | 0.0086  | 273.19   | 256.23   | 0.0060    | 1.024e26       |

*Source: NASA JPL Horizons, simplified for 2D ecliptic plane*

### Unit Conversions
```rust
const AU_TO_METERS: f64 = 1.495978707e11;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
const DEG_PER_DAY_TO_RAD_PER_SEC: f64 = DEG_TO_RAD / 86400.0;
```

### Moons (Parent-Relative)

| Moon     | Parent  | a (km)    | a (m)       | e      | Period (days) | Mass (kg)  |
|----------|---------|-----------|-------------|--------|---------------|------------|
| Moon     | Earth   | 384,400   | 3.844e8     | 0.0549 | 27.32         | 7.342e22   |
| Io       | Jupiter | 421,800   | 4.218e8     | 0.0041 | 1.77          | 8.932e22   |
| Europa   | Jupiter | 671,100   | 6.711e8     | 0.0094 | 3.55          | 4.800e22   |
| Ganymede | Jupiter | 1,070,400 | 1.070e9     | 0.0011 | 7.15          | 1.482e23   |
| Callisto | Jupiter | 1,882,700 | 1.883e9     | 0.0074 | 16.69         | 1.076e23   |
| Titan    | Saturn  | 1,221,870 | 1.222e9     | 0.0288 | 15.95         | 1.345e23   |

*Note: ω and M₀ for moons should be looked up from JPL Horizons for accuracy*

### Physical Radii

| Body     | Radius (km) | Radius (m)  |
|----------|-------------|-------------|
| Sun      | 696,340     | 6.963e8     |
| Mercury  | 2,440       | 2.440e6     |
| Venus    | 6,052       | 6.052e6     |
| Earth    | 6,371       | 6.371e6     |
| Mars     | 3,390       | 3.390e6     |
| Jupiter  | 69,911      | 6.991e7     |
| Saturn   | 58,232      | 5.823e7     |
| Uranus   | 25,362      | 2.536e7     |
| Neptune  | 24,622      | 2.462e7     |
| Moon     | 1,737       | 1.737e6     |
| Io       | 1,822       | 1.822e6     |
| Europa   | 1,561       | 1.561e6     |
| Ganymede | 2,634       | 2.634e6     |
| Callisto | 2,410       | 2.410e6     |
| Titan    | 2,575       | 2.575e6     |

## 7. Ephemeris Resource

```rust
/// Number of gravity sources: Sun + 8 planets (moons are decorative)
pub const GRAVITY_SOURCE_COUNT: usize = 9;

/// Fixed-size array of gravity sources (no heap allocation)
pub type GravitySources = [(DVec2, f64); GRAVITY_SOURCE_COUNT];

#[derive(Resource)]
struct Ephemeris {
    body_data: HashMap<CelestialBodyId, CelestialBodyData>,
    gm_cache: [f64; GRAVITY_SOURCE_COUNT],  // Pre-computed GM values
    // ... entity mappings, Horizons tables, etc.
}

impl Ephemeris {
    fn get_position(&self, body: Entity, time: f64) -> DVec2 {
        // Query from Horizons tables (preferred) or fall back to Kepler
    }

    /// Returns fixed-size array of (position, GM) for Sun + 8 planets.
    /// Moons are excluded (decorative only).
    fn get_gravity_sources(&self, time: f64) -> GravitySources {
        // Batched sampling from Horizons tables or Kepler fallback
    }

    /// Check collision with Sun and planets only.
    /// Moons have no collision detection.
    fn check_collision(&self, pos: DVec2, time: f64) -> Option<CelestialBodyId> {
        // Sun: 2x multiplier
        // Planets: 50x multiplier (COLLISION_MULTIPLIER)
        // Moons: not checked
    }
}
```
