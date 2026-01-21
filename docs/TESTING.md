# Testing Strategy

This document describes the testing strategy for the orbital mechanics simulator.

## Test Categories

### Unit Tests (inline `#[cfg(test)]` modules)

Located within source files, these test individual functions and components:

- **Physics tests** (`src/physics/`): Integrator accuracy, energy conservation, gravity calculations
- **Ephemeris tests** (`src/ephemeris/`): Kepler solver, planetary positions, continuity
- **Type tests** (`src/types.rs`): Unit conversions, state representations
- **Prediction tests** (`src/prediction.rs`): Trajectory computation, caching
- **Collision tests** (`src/collision.rs`): Detection algorithms
- **Input tests** (`src/input.rs`): Keyboard/mouse handling
- **Camera tests** (`src/camera.rs`): Zoom, pan, focus
- **UI tests** (`src/ui/`): Velocity handle, context cards

### Property-Based Tests (proptest)

Located in `src/physics/proptest_physics.rs` and `src/ephemeris/proptest_ephemeris.rs`:

- Randomly generated orbital parameters
- Verify invariants hold across parameter space
- Useful for finding edge cases in numerical algorithms

### Integration Tests (`tests/` directory)

End-to-end tests that verify system behavior:

- **Physics integration**: Multi-body dynamics, long-term stability
- **Prediction integration**: Cache continuation, outcome detection
- **Scenario integration**: Scenario loading, state management
- **Bevy headless**: Resource initialization, system execution

## Physical Invariants

Tests verify these fundamental properties:

1. **Energy Conservation**: Specific orbital energy `E = v²/2 - GM/r` should drift <1% over 100 orbits
2. **Angular Momentum Conservation**: `L = r × v` should drift <0.1% for central force
3. **Kepler's Third Law**: Orbital period `T² ∝ a³` within 1%
4. **Kepler Solver Convergence**: Newton-Raphson converges for all `e ∈ [0, 0.9999]`

## Running Tests

### All Tests

```bash
cargo test
```

### Specific Categories

```bash
# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test '*'

# Property tests
cargo test prop_

# Specific module
cargo test physics::
cargo test ephemeris::

# With output
cargo test -- --nocapture
```

### Coverage Report

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate HTML report
cargo tarpaulin --out html

# Open report
open tarpaulin-report.html
```

## Test Utilities

The `src/test_utils.rs` module provides:

### Fixtures

```rust
use crate::test_utils::fixtures;

// Create standard orbital states
let earth_like = fixtures::circular_orbit(1.0);        // 1 AU circular
let comet_like = fixtures::elliptical_orbit(1.0, 0.9); // e=0.9 ellipse
let escaping = fixtures::escape_trajectory(1.0);       // Hyperbolic
```

### Assertions

```rust
use crate::test_utils::assertions;

// Compute conserved quantities
let energy = assertions::orbital_energy(pos, vel);
let angular_momentum = assertions::angular_momentum(pos, vel);

// Assert conservation
assertions::assert_energy_conserved(initial_e, final_e, 0.01);
assertions::assert_angular_momentum_conserved(initial_l, final_l, 0.001);
```

### Headless Bevy

```rust
use crate::test_utils::bevy_test;

let mut app = bevy_test::headless_app();
app.insert_resource(MyResource::default());
app.add_systems(Update, my_system);
app.update();
```

## Coverage Targets

These are informal guidelines, not CI-enforced:

| Module | Target Coverage |
|--------|-----------------|
| `physics/` | ~80% |
| `ephemeris/` | ~80% |
| `prediction.rs` | ~70% |
| `collision.rs` | ~70% |
| `types.rs` | ~90% |
| `input.rs` | ~50% |
| `camera.rs` | ~50% |
| `ui/` | ~40% |

## Writing New Tests

### Physics Tests

When testing orbital mechanics:

1. Use `test_utils::fixtures` to create orbital states
2. Run simulation for appropriate duration (use orbital period as reference)
3. Assert invariants with `test_utils::assertions`

Example:
```rust
#[test]
fn test_energy_conservation() {
    let state = fixtures::elliptical_orbit(1.0, 0.5);
    let initial_energy = assertions::orbital_energy(state.pos, state.vel);

    // Simulate one orbit
    let period = assertions::orbital_period(semi_major_axis);
    let final_state = simulate(state, period);

    let final_energy = assertions::orbital_energy(final_state.pos, final_state.vel);
    assertions::assert_energy_conserved(initial_energy, final_energy, 0.01);
}
```

### Property Tests

Use proptest for parameter space exploration:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_bound_orbit_returns(
        distance in 0.3f64..30.0,
        eccentricity in 0.0f64..0.95,
    ) {
        let state = fixtures::elliptical_orbit(distance, eccentricity);
        prop_assert!(assertions::is_bound(state.pos, state.vel));
    }
}
```

## CI Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs:

1. **test**: All unit and integration tests
2. **clippy**: Lint checks with `-D warnings`
3. **fmt**: Formatting verification
4. **examples**: Build all examples
5. **coverage**: Generate coverage report (uploaded to Codecov)

All jobs must pass for PR merges.
