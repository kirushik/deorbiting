#  Orbital Mechanics & Visualization Tool

A Rust + Bevy based Linux desktop app for simulating asteroid deorbiting missions with accurate orbital mechanics and a user-friendly interface.

## Project overview

Desktop GUI app, Google Maps style: 2D view of the solar system with zoom/pan, click-and-drag spacecraft positions and velocity vectors, and real-time orbital mechanics simulation.
Simulation time is adjustable (pause, real-time, fast-forward).
Visual representation of orbits, trajectories, and celestial bodies with accurate physics.

## Specs and documentation
 - [Architecture Overview](./docs/ARCHITECTURE.md)
 - [Implementation Roadmap](./docs/ROADMAP.md)
 - [Ephemeris & Orbital Elements](./docs/EPHEMERIS.md)
 - [Physics Specification](./docs/PHYSICS.md)
 - [UI Specification](./docs/UI.md)

## Code Style
 - Rust 2024 edition
 - Bevy ECS architecture
 - Modular systems and components
 - Test-driven development for core physics and ephemeris calculations
 - Concise inline documentation

## Useful MCP tooling
Please use **Context7** MCP server for looking up any relevant documentation for software libraries. If some library you need is not documented there, please highlight that to me, so I can add the missing ones.
Please use **Serena** MCP server for structured code search, generation and editing, as well as for **memory management** during this project. It's like IDE for AI.
When planning/designing, please rely extensively on the **Sequential Thinking** MCP server to ensure all aspects are covered and thinking is coordinated across sub-agents.
