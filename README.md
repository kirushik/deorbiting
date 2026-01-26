# Deorbiting

An orbital mechanics simulator for exploring asteroid deflection scenarios. Pan and zoom around the solar system, adjust asteroid trajectories, and test planetary defense strategies.

## What It Does

- Real-time 2D solar system simulation with accurate orbital mechanics
- Interactive trajectory editing: drag velocity vectors to see orbit changes instantly
- Six scenarios: from Earth collision courses to Jupiter gravity assists
- Deflection methods: kinetic impactors, nuclear standoff, ion beams, gravity tractors, laser ablation, solar sails

Physics uses the Velocity Verlet integrator with gravity from the Sun and all 8 planets. Asteroid trajectories update in real time as you adjust parameters.

## Download

Pre-built binaries are available from [GitHub Releases](https://github.com/kirushik/deorbiting/releases):

| Platform | File | Notes |
|----------|------|-------|
| Linux | `deorbiting_vX.Y.Z_linux_x86_64` | Make executable: `chmod +x` |
| Windows | `deorbiting_vX.Y.Z_windows_x86_64.zip` | Extract and run |
| macOS | `deorbiting_vX.Y.Z_macos_universal` | Unsigned; right-click → Open on first launch |

Requires a GPU with OpenGL 3.3+ or Vulkan support.

## Build from Source

```bash
git clone https://github.com/kirushik/deorbiting.git
cd deorbiting
cargo run --release
```

Requires Rust 1.85+ (2024 edition). First build compiles Bevy and dependencies, which takes a while.

**Linux**: install `lld` linker first (`apt install lld` on Debian/Ubuntu).

### Ephemeris Tables

Planet positions come from JPL Horizons data. The repo includes pre-generated tables in `assets/ephemeris/`. To regenerate or customize:

```bash
python3 scripts/export_horizons_ephemeris.py
```

Requires Python 3.9+ (stdlib only). Downloads ~200 years of orbital data from NASA's Horizons API.

### Standalone Binary

To embed ephemeris data into the binary (no `assets/` folder needed):

```bash
cargo build --profile dist --features embedded-ephemeris
```

Output in `target/dist/`. Binary is ~18MB larger but fully self-contained.

## Controls

| Action | Input |
|--------|-------|
| Pan | Left-click drag or middle-click drag |
| Zoom | Scroll wheel |
| Select body | Left-click |
| Edit velocity | Drag the cyan arrow on selected asteroid |
| Focus on point | Double-click |
| Play/Pause | Space |
| Time scale | 1-4 keys |
| Scenario menu | Escape or M |
| Reset scenario | R |

## Scenarios

1. **Earth Collision Course** — Tutorial: asteroid heading for Earth in ~23 days
2. **Apophis Flyby** — Watch orbital period change after a close approach
3. **Jupiter Slingshot** — Classic gravity assist, asteroid gains ~10 km/s
4. **Interstellar Visitor** — Oumuamua-style hyperbolic trajectory
5. **Deflection Challenge** — Save Earth with minimal delta-v
6. **Sandbox** — Place asteroid anywhere, experiment freely

## Deflection Methods

**Instant** (interceptor arrives and applies delta-v):
- Kinetic impactor — DART-style collision
- Nuclear standoff — Surface vaporization thrust
- Nuclear split — Armageddon-style fragmentation

**Continuous** (spacecraft operates over months):
- Ion beam shepherd — Ion exhaust pushes asteroid
- Gravity tractor — Gravitational pull from nearby spacecraft
- Laser ablation — Surface vaporization from focused light
- Solar sail — Radiation pressure deflection

Parameters are boosted for gameplay. Real missions take years; here you see results in days.

## Project Structure

```
src/
├── main.rs          # Bevy app setup
├── ephemeris/       # Kepler solver, orbital elements
├── physics/         # Gravity, integrator
├── prediction.rs    # Trajectory prediction
├── scenarios/       # Preset starting conditions
├── interceptor/     # Instant deflection methods
├── continuous/      # Continuous deflection methods
├── render/          # Visualization systems
└── ui/              # egui panels and controls
```

Design documentation is in `docs/`. Agent instructions are in `AGENTS.md`.
