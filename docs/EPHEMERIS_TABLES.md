# Table-based Ephemeris (JPL Horizons)

This project supports an optional **table-based ephemeris** generated from the official NASA/JPL SSD Horizons API.

If present, these tables are preferred over the baked-in Keplerian element model, because they:
- reduce long-term phase drift
- better match the actual Solar System motion (without running an n-body integrator for planets)

## What is generated

We export **2D heliocentric vectors** for the bodies in scope:

- Planets: Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune
- Major moons in-scope for gameplay: Moon, Io, Europa, Ganymede, Callisto, Titan

All vectors are exported in the same frame:

- **Origin:** Sun at `(0, 0)`
- **Plane:** ecliptic-of-J2000
- Units: meters and meters/second
- Time: seconds since J2000 (t=0 at 2000-01-01 12:00)

Coverage is **forward-only** from J2000 for a configurable number of years.

## Exporter

Script:
- `scripts/export_horizons_ephemeris.py`

It downloads vector ephemerides via the Horizons API:
- https://ssd-api.jpl.nasa.gov/doc/horizons.html

### Example

Generate 200 years from J2000, with 1-day planet cadence and 2-hour moon cadence:

```sh
python3 scripts/export_horizons_ephemeris.py --years 200 --planet_step "1 d" --moon_step "2 h" --out assets/ephemeris
```

Outputs:
- `assets/ephemeris/*.bin` (one per body)
- `assets/ephemeris/manifest.json`

## Binary format

Each `.bin` file is little-endian and contains:

- magic: `DEOEPH1\0`
- version: `u32 = 1`
- body_id: `u32` (stable mapping shared with the Rust loader)
- step_seconds: `f64`
- start_t0: `f64` (seconds since J2000; currently expected to be near 0.0)
- count: `u32`
- reserved: `u32 = 0`
- `count` samples, each:
  - `x, y, vx, vy` as `f64`

## Runtime interpolation

Runtime uses **cubic Hermite interpolation** per segment using `pos` and `vel` samples.
This gives smooth positions/velocities with relatively coarse sampling.

Implementation:
- `src/ephemeris/table.rs`
- `src/ephemeris/horizons_tables.rs`

## Extrapolation Past Table End

When simulation time exceeds table coverage (e.g., year 2200+ for outer planets), the ephemeris falls back to **pure Kepler orbits** without any offset correction.

### Why Pure Kepler (No Offsets)

Earlier versions attempted to maintain continuity at the table boundary by computing a position offset (Horizons - Kepler) at table end and applying it to subsequent Kepler positions. This approach failed catastrophically for outer planets:

| Approach | Result for Jupiter at year 2222 |
|----------|--------------------------------|
| Offset extrapolation | 2.9 AU (wrong!) |
| Pure Kepler | 5.2 AU (correct) |

**Root cause:** The Horizons-Kepler offset at the boundary represents a position difference at that instant. 22 years later, Jupiter has completed ~2 orbits, and the offset direction has rotated. Applying a fixed offset to a different orbital phase produces nonsensical results.

### Current Behavior

1. **Within table coverage:** Use Horizons table with cubic Hermite interpolation
2. **Past table end:** Use pure Kepler (no offset, no "bridge")

This produces a small discontinuity at the boundary (< 0.1 AU for inner planets), but ensures:
- Orbits remain physically meaningful ellipses
- Planets stay at correct distances from Sun
- No accumulating errors over time

For a visualization application, correct orbital shapes matter more than a one-time boundary discontinuity.

### Regression Test

`test_drifting_offset_does_not_distort_orbits` verifies that Mars remains closer to the Sun than Jupiter when extrapolating past table end. This catches any future regression that might reintroduce offset-based extrapolation.

## Notes

- If tables are missing, the app will fall back to the Keplerian model.
- TT/UTC/leap-second differences are ignored by design (game-time axis).
- The currently used Horizon ephemeris data is available up for 500 years from J2000 for the first 4 planets and the Moon, but only 200 years for the outer planets and major moons.
