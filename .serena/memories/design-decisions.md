# Design Decisions Summary

## Core Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Precision | f64 physics, f32 render | Orbital accuracy requires f64 for meter-scale solar system |
| Integrator | IAS15 | Adaptive 15th-order, machine-precision energy conservation |
| Time base | `f64` seconds since J2000 (approx, uniform; ignore TT/UTC + leap seconds) | Good-enough game time axis; avoids heavy time libs | Good balance for observing planetary motion |
| Moons | Treat as heliocentric in ephemeris tables (2D), but keep `CelestialBodyId::parent()` for gameplay/organization | Simplest runtime ephemeris usage; avoids mixed frames | Natural hierarchy: moon_pos = planet_pos + local_orbit |
| Distortion | Nearest planet only | Simple v1, may have visual artifacts at boundaries |
| Camera | Scroll zoom + drag pan | Standard map controls |
| Orbit data | Table-based ephemeris generated from JPL Horizons (vectors) covering ~200y from J2000 | Much less drift than fixed Kepler elements; still lightweight at runtime | Accuracy with real planetary positions |
| Bevy version | 0.15 | Latest stable |
| Collision | Pause + impact effect | Clear feedback for hit/miss scenarios |
| Velocity UI | Draggable arrow | KSP-style, intuitive |
| Display units | km/AU toggleable | User flexibility |
| Prediction | Always visible | User awareness of trajectory |
| Game mode | Sandbox + preset scenarios | Flexibility with guided experiences |

## Documentation Structure

- `ARCHITECTURE.md` - Split-world pattern, ECS components, coordinate systems
- `PHYSICS.md` - IAS15 integrator, time system, gravity model
- `EPHEMERIS.md` - Kepler solver, J2000 orbital elements, hierarchical orbits
- `ROADMAP.md` - 6-phase implementation plan (Phase 0-5)
- `UI.md` - Time controls, velocity handle, info panel, scenario menu

## Key Constants

- J2000 Unix timestamp: 946728000
- G (gravitational constant): 6.67430e-11 m³·kg⁻¹·s⁻²
- AU to meters: 1.495978707e11
