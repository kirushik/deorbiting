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

## Notes

- If tables are missing, the app will fall back to the Keplerian model.
- TT/UTC/leap-second differences are ignored by design (game-time axis).
- The currently used Horizon ephemeris data is available up for 500 years from J2000 for the first 4 planets and the Moon, but only 200 years for the outer planets and major moons.
