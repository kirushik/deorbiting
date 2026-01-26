# Orbital Mechanics & Visualization Tool

A Rust + Bevy based Linux desktop app for simulating asteroid deorbiting missions with accurate orbital mechanics and a user-friendly interface.

## Project Overview

Desktop GUI app, Google Maps style: 2D view of the solar system with zoom/pan, click-and-drag spacecraft positions and velocity vectors, and real-time orbital mechanics simulation.
Simulation time is adjustable (pause, real-time, fast-forward).
Visual representation of orbits, trajectories, and celestial bodies with accurate physics.

## Documentation Structure

### Specifications (_what_ to build)
 - [Architecture Overview](./docs/ARCHITECTURE.md) - Split-world pattern, ECS components, coordinate systems
 - [Physics Specification](./docs/PHYSICS.md) - IAS15 integrator, time system, gravity model
 - [Ephemeris & Orbital Elements](./docs/EPHEMERIS.md) - Kepler solver, J2000 orbital elements
 - [Ephemeris Tables Usage](./docs/EPHEMERIS_TABLES.md) - Description of the optional ephemeris tables usage for planets' and moons' trajectories
 - [Deflection Methods](./docs/DEFLECTION_METHODS.md) - All deflection methods, physics formulas, gameplay parameters, visualization details
 - [UI Specification](./docs/UI.md) - Time controls, velocity handle, panels, overlays
 - [UI/UX Guidelines](./docs/UI_GUIDELINES.md) - Design principles, typography, colors, interaction patterns

### Implementation Guides (_how_ to build)
 - [Implementation Roadmap](./docs/ROADMAP.md) - High-level phase descriptions
 - [Implementation Checklist](./docs/CHECKLIST.md) - **Detailed coding tasks with checkboxes**
 - [Testing Guide](./docs/TESTING.md) - Testing strategies and best practices

### Serena Memories (_why_ decisions were made)
Use Serena's memory system to persist and retrieve design rationale across sessions:
 - `design-decisions.md` - Core technical decisions with rationale

## Agent Workflow

### Starting a New Session

1. **Read the checklist first:** Open `docs/CHECKLIST.md` to see current progress and next tasks
2. **Load relevant memories:**
   ```
   list_memories        # See available context
   read_memory design-decisions.md   # Understand why decisions were made
   ```
3. **Check current phase:** Identify which phase you're in and what's incomplete
4. **Work through tasks:** Complete checklist items in order, checking them off as done

### During Implementation

- **Checklist = _what_:** The specific coding tasks to complete
- **Spec docs = _details_:** Implementation specifics, data structures, algorithms
- **Memories = _why_:** Design rationale, trade-offs considered, decisions made

### Running code snippets

When you need to test something in the running project, don't just run `cargo run` naively. It's a GUI app, which is hard for agents to control.
Instead, leverage the `cargo run --example` feature: create a small example in the `examples/` folder and run only what you need, with a console output.
Feel free to create multiple example files for different test cases; treat the more useful ones as mini-integration tests, and keep them around for future use.
This might also be a better alternative to running one-off Python scripts for testing calculations.

### Tests vs Examples

This project uses Rust's standard directory conventions:

#### `tests/` - Integration Tests
- **Purpose**: Verify that major features work correctly end-to-end
- **Built in CI**: Yes (`cargo build --tests` and `cargo test`)
- **Naming**: Descriptive names without `test_` prefix (e.g., `scenarios.rs`, `time_sync.rs`)
- **When to add**: When you implement a new feature that needs verification
- **Imports**: Must use `use deorbiting::*;` since tests are external to the crate
- **Run individually**: `cargo test --test <name>` (e.g., `cargo test --test scenarios`)

#### `examples/` - Developer Utilities
- **Purpose**: Benchmarks, diagnostic tools, debug utilities, one-off analysis
- **Built in CI**: No (too memory-intensive for GitHub Actions)
- **Naming**: Descriptive with category prefix (e.g., `benchmark_*.rs`, `debug_*.rs`)
- **When to add**: For performance analysis, debugging sessions, or analysis tools
- **Cleanup**: Delete throwaway debug examples after use; keep only reusable utilities
- **Run individually**: `cargo run --example <name>` (e.g., `cargo run --example benchmark_gravity`)

#### Guidelines
- New feature? Add integration test to `tests/`
- Performance analysis? Add benchmark to `examples/`
- Debugging an issue? Create temp example, delete when done
- One-off analysis? Create example, keep if reusable, delete otherwise

### Before Ending a Session

1. **Update checklist:** Mark completed items with `[x]`
2. **Update memories:** If you made significant decisions, write them to memory:
   ```
   write_memory <topic>.md <content>
   ```
3. **Note blockers:** If something is blocked, add a note in the checklist

### Cross-Session Continuity

The checklist and memories enable any agent (or the same agent in a new context) to:
- Pick up exactly where work left off
- Understand not just _what_ to build but _why_ it's designed that way
- Avoid re-debating settled decisions
- Maintain consistency across implementation

## Code Style

 - Rust 2024 edition
 - Bevy 0.15 ECS architecture
 - Modular systems and components
 - Test-driven development for core physics and ephemeris calculations
 - Concise inline documentation
 - f64 for physics, f32 for rendering

## MCP Tooling

### Context7
Use for looking up documentation for external libraries (Bevy, bevy_egui, glam, etc.). If a library isn't documented there, flag it so it can be added.

### Serena
Primary tool for this project:
- **Code navigation:** `find_symbol`, `get_symbols_overview`, `find_referencing_symbols`
- **Code editing:** `replace_symbol_body`, `insert_after_symbol`, `insert_before_symbol`
- **Memory management:** `list_memories`, `read_memory`, `write_memory`, `edit_memory`
- **Pattern search:** `search_for_pattern` for finding code across files

IMPORTANT: Never use Serena's `execute_shell_command` tool, it is broken and unsafe. Use MCP's built-in file operations instead, and if you need to launch shell commands, do so manually outside MCP.

### Sequential Thinking
Use for complex planning or design decisions that require structured reasoning across multiple steps. Especially useful when:
- Designing new systems or features
- Debugging complex issues
- Making architectural decisions
