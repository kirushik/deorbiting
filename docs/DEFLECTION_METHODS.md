# Deflection Methods

This document describes all asteroid deflection methods available in the simulation, their physics, gameplay parameters, and implementation details.

## Overview

Deflection methods are divided into two categories:

1. **Instant Methods** - Applied on interceptor arrival, provide immediate delta-v
2. **Continuous Methods** - Apply thrust over time, integrated into physics simulation

All methods use **gameplay-boosted parameters** to provide meaningful deflection at simulation timescales (days to months rather than years to decades).

---

## Instant Deflection Methods

These are delivered by interceptors that travel from Earth to the asteroid. On arrival, they instantly apply a delta-v to the asteroid.

### Kinetic Impactor

**Concept:** A spacecraft collides with the asteroid at high relative velocity, transferring momentum.

**Physics:**
```
Δv = β × (m_impactor × v_relative) / M_asteroid
```

Where:
- `β` = momentum enhancement factor (ejecta contribution)
- `m_impactor` = impactor mass (kg)
- `v_relative` = impact velocity (m/s)
- `M_asteroid` = asteroid mass (kg)

**Real-world reference:** NASA's DART mission measured β ≈ 3.6 for Dimorphos.

**Gameplay parameters:**
| Parameter | Realistic | Gameplay | Boost |
|-----------|-----------|----------|-------|
| Impactor mass | 560 kg (DART) | 200,000 kg | 357× |
| β factor | 3.6 | 40.0 | 11× |

**Effectiveness:** ~8 m/s delta-v per impact against a 300m asteroid (3×10¹⁰ kg)

**Source:** `src/interceptor/payload.rs` - `DeflectionPayload::Kinetic`

---

### Nuclear Standoff

**Concept:** A nuclear device detonates at standoff distance, vaporizing surface material. The expanding vapor acts as a rocket exhaust.

**Physics:**
```
Δv = reference_Δv × (yield / reference_yield) × (reference_mass / M_asteroid)
```

**Real-world reference:** LLNL research suggests ~2 cm/s per 100 kt for a 300m asteroid.

**Gameplay parameters:**
| Parameter | Realistic | Gameplay | Boost |
|-----------|-----------|----------|-------|
| Reference Δv | 0.02 m/s | 0.30 m/s | 15× |
| Default yield | 100 kt | 12,000 kt (12 MT) | 120× |

**Effectiveness:** ~36 m/s delta-v per detonation against a 300m asteroid

**Source:** `src/interceptor/payload.rs` - `DeflectionPayload::Nuclear`

---

### Nuclear Split (Fragmentation)

**Concept:** "Armageddon-style" deep penetration detonation that breaks the asteroid into fragments with diverging trajectories.

**Physics:**
```
E_kinetic = efficiency × yield × 4.184×10¹² J/kt
v_separation = sqrt(2 × E_kinetic / M_asteroid)
```

**Gameplay parameters:**
| Parameter | Value |
|-----------|-------|
| Default yield | 50 MT |
| Energy efficiency | 2% converted to kinetic |
| Split ratio | Configurable (default 0.5 = equal split) |

**Behavior:**
- Original asteroid is destroyed
- Two new asteroids spawn with diverging velocities perpendicular to the deflection direction
- **Warning:** May create multiple collision threats!

**Source:** `src/interceptor/payload.rs` - `DeflectionPayload::NuclearSplit`

---

## Continuous Deflection Methods

These methods apply thrust over time. A spacecraft (or ground-based system for laser) operates near the asteroid for the mission duration.

### Ion Beam Shepherd

**Concept:** Spacecraft hovers 50-500m from asteroid, directing ion engine exhaust at the surface to transfer momentum.

**Physics:**
```
acceleration = thrust_N / M_asteroid
fuel_rate = thrust / (Isp × g₀)
```

Where:
- `Isp` = specific impulse (~3000-5000 s for ion engines)
- `g₀` = 9.80665 m/s² (standard gravity)

**Gameplay parameters:**
| Parameter | Value |
|-----------|-------|
| Default thrust | 50 kN |
| Default fuel mass | 2,000,000 kg |
| Specific impulse | 3000 s |
| Hover distance | 200 m |

**Effectiveness:** ~2.3 m/s delta-v over mission duration against a 300m asteroid

**Visualization:** Cyan exhaust cone with 7 flickering lines and particle effects

**Reference:** Bombardelli, C. et al. (2011) "Ion Beam Shepherd for Asteroid Deflection"

**Source:** `src/continuous/payload.rs` - `ContinuousPayload::IonBeam`, `src/continuous/thrust.rs`

---

### Laser Ablation (DE-STAR)

**Concept:** Earth-based or space-based high-power laser vaporizes asteroid surface material, creating thrust from the ablation plume.

**Physics:**
```
thrust_N = 115 × (power_kW / 100) × solar_efficiency
solar_efficiency = min(1.0, 1 / distance_AU²)
```

The 115 N per 100 kW is a 50× gameplay boost from the realistic DE-STARLITE value of 2.3 N per 100 kW.

**Gameplay parameters:**
| Parameter | Value |
|-----------|-------|
| Default power | 50 MW |
| Default duration | 6 months |
| Flight time | 0 (Earth-based) |

**Effectiveness:** ~24 m/s delta-v over mission duration against a 300m asteroid

**Visualization (non-trivial implementation):**

The laser beam visualization includes several sophisticated features:

1. **Full Earth-to-asteroid beam:** Draws the complete path from Earth's actual position to the asteroid (not a short segment)

2. **Traveling energy pulses:** 8 bright spots travel along the beam from Earth to asteroid, creating a dynamic "beaming" effect with varying brightness

3. **Occlusion by celestial bodies:**
   - Queries Sun and planets for their positions and `EffectiveVisualRadius` (inflated visual sizes)
   - Performs line-circle intersection tests for each body
   - If the beam would pass through a celestial body's visual representation:
     - Draws a **dashed line** from Earth to the occlusion point (not solid, to avoid implying we're firing at the Sun)
     - Shows a red "X" marker at the occlusion point
     - Does **not** draw impact effects (glow, ablation plume)
   - Earth itself is excluded from occlusion checks

4. **Impact effects:** Pulsing orange glow circle and ablation plume particles flying back toward Earth

**Source:** `src/continuous/payload.rs` - `ContinuousPayload::LaserAblation`, `src/render/deflectors.rs` - `draw_laser_beam()`

---

### Solar Sail

**Concept:** Large reflective sail attached to asteroid uses solar radiation pressure for propellantless deflection.

**Physics:**
```
thrust_N = SRP × sail_area × (1 AU / distance)²
```

Where:
- `SRP` = 9.08×10⁻⁴ N/m² at 1 AU (100× gameplay boost from realistic 9.08×10⁻⁶)
- Thrust direction is always away from the Sun

**Gameplay parameters:**
| Parameter | Value |
|-----------|-------|
| Default sail area | 10 km² |
| Default duration | 1 year |
| Reflectivity | 0.9 |

**Effectiveness:** ~17 m/s delta-v over mission duration against a 300m asteroid

**Visualization:** Diamond shape perpendicular to sun direction, with incoming and reflected solar ray lines

**Source:** `src/continuous/payload.rs` - `ContinuousPayload::SolarSail`, `src/render/deflectors.rs` - `draw_solar_sail()`

---

## Thrust Directions

Continuous methods support configurable thrust directions:

| Direction | Description |
|-----------|-------------|
| **Retrograde** (default) | Opposite to velocity - slows asteroid, lowers orbit |
| **Prograde** | Same as velocity - speeds up asteroid, raises orbit |
| **Radial** | Perpendicular inward - changes orbital plane |
| **AntiRadial** | Perpendicular outward |
| **SunPointing** | Away from Sun (forced for Solar Sail) |
| **Custom** | User-specified direction vector |

**Source:** `src/continuous/thrust.rs` - `ThrustDirection`

---

## Combined Effectiveness

The test example `examples/test_continuous_deflection.rs` validates combined effectiveness:

| Method | Delta-v (300m asteroid) |
|--------|------------------------|
| Ion Beam | 2.29 m/s |
| Laser Ablation | 24.19 m/s |
| Solar Sail | 17.19 m/s |
| **Combined** | **43.67 m/s** |

This is sufficient to deflect a 300m asteroid at 0.07 AU (~4 days before impact).

---

## Interceptor Flight

All deflection methods (except Earth-based laser) require launching an interceptor from Earth:

1. **Lambert solver** computes the optimal transfer orbit
2. **Transfer arc** is visualized as a curved trajectory
3. **Flight time** depends on distance and `BASE_INTERCEPTOR_SPEED` (100 km/s, gameplay-boosted from realistic 15 km/s)

Typical flight times:
- 0.1 AU: ~1.7 days
- 0.5 AU: ~8.7 days
- 2.0 AU: ~34.6 days

**Source:** `src/interceptor/mod.rs`, `src/lambert.rs`

---

## Key Files

| File | Purpose |
|------|---------|
| `src/interceptor/payload.rs` | Instant deflection payloads and delta-v calculations |
| `src/interceptor/mod.rs` | Interceptor entity, flight, and impact handling |
| `src/continuous/payload.rs` | Continuous deflection payloads |
| `src/continuous/thrust.rs` | Thrust physics calculations |
| `src/continuous/mod.rs` | Continuous deflector component and state machine |
| `src/render/deflectors.rs` | Visualization for all continuous methods |
| `src/lambert.rs` | Lambert solver for transfer orbits |
| `examples/test_continuous_deflection.rs` | Validation of combined effectiveness |
