# Visual Distortion Specification

## Problem Statement

Two critical visual bugs in the solar system visualization:
1. **Moon larger than Earth**: At certain zoom levels, dynamic scaling inflates the Moon more aggressively than Earth
2. **Jupiter moons inside Jupiter**: Moons render at physical positions (0.4-1.9 Gm) while Jupiter's visual radius is 3.5+ Gm

### Root Cause Analysis

| Body | Physical Radius | visual_scale | Base Render Size | Physical Orbital Distance |
|------|-----------------|--------------|------------------|---------------------------|
| Earth | 6.371e6 m | 150 | 0.96 Gm | - |
| Moon | 1.737e6 m | 250 | 0.43 Gm | 0.384 Gm |
| Jupiter | 6.991e7 m | 50 | 3.5 Gm | - |
| Io | 1.822e6 m | 300 | 0.55 Gm | 0.422 Gm |
| Europa | 1.561e6 m | 300 | 0.47 Gm | 0.671 Gm |
| Ganymede | 2.634e6 m | 280 | 0.74 Gm | 1.070 Gm |
| Callisto | 2.410e6 m | 280 | 0.67 Gm | 1.883 Gm |

**All Jupiter moons orbit INSIDE Jupiter's visual radius (3.5 Gm).**

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Max moon size | **40% of parent** | Clear visual hierarchy, moons distinctly smaller |
| Moon orbit paths | **Distort to match moon positions** | Visual consistency |
| Tuning UI | **No, hardcode defaults** | Less clutter, adjust in code if needed |

---

## Solution: Hierarchical Distortion System

### Constants

```rust
/// Maximum moon size as fraction of parent's visual radius.
pub const MAX_MOON_FRACTION: f32 = 0.4;

/// Margin between parent visual edge and closest moon (fraction of parent radius).
pub const MARGIN_FRACTION: f32 = 0.15;

/// Minimum spacing between consecutive moons (render units / Gm).
pub const MIN_MOON_SPACING: f32 = 0.3;
```

### Part 1: Hierarchical Size Calculation

**Two-pass algorithm replacing `update_body_scales`:**

**Pass 1 - Sun & Planets (no parent):**
```
base_radius = body.radius * RENDER_SCALE * body.visual_scale
visibility_scale = max(1.0, min_render_radius / base_radius)
visibility_scale = min(visibility_scale, max_scale)
effective_radius = base_radius * visibility_scale
```

**Pass 2 - Moons:**
```
base_radius = body.radius * RENDER_SCALE * body.visual_scale

// Inherit parent's scale (moons scale proportionally)
moon_scale = max(visibility_scale, parent_scale)

// Cap at 40% of parent's visual radius
max_radius = parent_effective_radius * MAX_MOON_FRACTION
if base_radius * moon_scale > max_radius:
    moon_scale = max_radius / base_radius

effective_radius = base_radius * moon_scale
```

### Part 2: Position Distortion

Push moons outward so their edge clears the parent's visual edge:

```
// Sort moons by orbital distance (inner to outer)
moons.sort_by_orbital_distance()

for moon in moons:
    direction = normalize(moon_pos - parent_pos)
    current_distance = distance(moon_pos, parent_pos)

    // Minimum distance: parent edge + moon radius + margin
    // This ensures moon's EDGE is outside parent's EDGE
    clearance = parent_effective_radius * MARGIN_FRACTION
    min_distance = parent_effective_radius + moon_effective_radius + clearance

    if current_distance < min_distance:
        moon_pos = parent_pos + direction * min_distance
        current_distance = min_distance

    // Update parent radius for next moon (stacking effect)
    // Next moon must clear this moon's position
    parent_effective_radius = current_distance + moon_effective_radius
```

### Part 3: Moon Orbit Path Distortion

Moon orbits are drawn centered on the parent planet's current position, scaled radially to match the visual distortion applied to moons.

For each moon:
1. Compute Kepler ellipse shape from orbital elements
2. Scale the orbit so the moon's current distorted position lies on it
3. Ensure the orbit clears the parent's visual radius
4. Draw the scaled orbit centered on the parent

---

## Components

```rust
/// Tracks effective visual radius after all scaling.
#[derive(Component, Default)]
pub struct EffectiveVisualRadius(pub f32);

/// Per-body distortion offset (how far pushed from physics position).
#[derive(Component, Default)]
pub struct DistortionOffset(pub Vec2);
```

---

## System Integration

### System Ordering

```rust
app.add_systems(Update, (
    sync_celestial_positions,        // Physics -> initial render pos
    compute_hierarchical_scales,     // Two-pass size calculation
    apply_moon_position_distortion,  // Push moons outward
).chain());

// Orbit drawing (runs after distortion)
app.add_systems(Update, (draw_orbit_paths, draw_moon_orbit_paths));
```

### Implementation Files

| File | Purpose |
|------|---------|
| `src/render/scaling.rs` | Hierarchical scaling + position distortion |
| `src/render/bodies.rs` | Component definitions and spawning |
| `src/render/orbits.rs` | Planet and moon orbit path rendering |

---

## Edge Cases

1. **Moon at parent center**: Use arbitrary direction (Vec2::X) for push
2. **Extreme zoom out**: All moons form clusters outside their parents
3. **Extreme zoom in**: Near-physical scale, minimal distortion
4. **Camera focus on moon**: Centers on distorted position (correct)
5. **Highlighting/labels**: Use distorted positions (automatic via Transform)

---

## Critical Invariant

**At ALL zoom levels:** `moon_edge_distance >= parent_edge` (no intersection)

This is guaranteed by the position distortion algorithm:
```
min_distance = parent_radius + moon_radius + (parent_radius * MARGIN_FRACTION)
             = parent_radius + moon_radius + clearance
```

Therefore:
- Moon center is at distance `min_distance` from parent center
- Moon edge closest to parent: `min_distance - moon_radius = parent_radius + clearance`
- Since clearance > 0, moon edge is always OUTSIDE parent edge by at least `clearance`

With `MARGIN_FRACTION = 0.15`, the gap between moon edge and parent edge is at least 15% of the parent's radius.

---

## Verification Checklist

1. **Full solar system view:**
   - [ ] Moon visually smaller than Earth
   - [ ] Jupiter moons form cluster outside Jupiter
   - [ ] Saturn's Titan outside Saturn
   - [ ] No moon intersects or overlaps its parent planet

2. **Jupiter close-up:**
   - [ ] All 4 Galilean moons visible outside Jupiter's disc
   - [ ] Moons arranged in order (Io closest, Callisto farthest)
   - [ ] Moon orbit paths follow moon positions
   - [ ] No moon touches Jupiter's visual edge

3. **Earth close-up:**
   - [ ] Moon visible outside Earth
   - [ ] Moon clearly smaller than Earth
   - [ ] Moon does not intersect Earth

4. **Zoom transitions:**
   - [ ] No visual jumps when zooming
   - [ ] Smooth size changes
   - [ ] At no zoom level do moons intersect their parent planet

5. **Interaction:**
   - [ ] Double-click moon centers camera correctly
   - [ ] Hover highlighting works on distorted positions
