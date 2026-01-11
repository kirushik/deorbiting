# UI Specification

## Overview

The UI follows a minimal, non-intrusive design that keeps the solar system visualization as the primary focus. Panels appear at screen edges and can be collapsed.

## 1. Time Controls Panel (Bottom)

A horizontal panel anchored to the bottom of the screen.

### Components

```
┌─────────────────────────────────────────────────────────────────────────┐
│  [⏸/▶]  │  2026-01-15 14:32:05 UTC  │  [1x] [10x] [100x] [1000x]  │  [↺]  │
└─────────────────────────────────────────────────────────────────────────┘
```

- **Play/Pause Button** (`⏸/▶`): Toggle simulation running state
- **Date/Time Display**: Current simulation time in human-readable format
  - Click to open date picker for jumping to specific time
- **Time Scale Buttons**: Mutually exclusive selection
  - 1x: 1 sim-day = 1 real-second
  - 10x: 1 sim-day = 0.1 real-seconds
  - 100x: 1 sim-day = 0.01 real-seconds
  - 1000x: 1 sim-day = 0.001 real-seconds
- **Reset Button** (`↺`): Return to scenario's initial state

### Behavior
- Panel is always visible (not collapsible)
- Semi-transparent background to see space behind
- Keyboard shortcuts: Space = play/pause, 1-4 = time scales

## 2. Velocity Handle

An interactive arrow for setting asteroid velocity.

### Appearance
```
         ↗ (drag handle)
        /
       /
      /
     ●───────────→ (velocity direction)
  asteroid
```

- **Arrow Base**: Centered on selected asteroid
- **Arrow Tip**: Draggable handle (larger hit area than visual)
- **Arrow Length**: Proportional to speed (logarithmic scale for large range)
- **Arrow Color**: Cyan/teal for visibility against space background

### Interaction
1. **Selection**: Click asteroid to show velocity handle
2. **Drag**: Click and drag arrow tip to set new velocity
3. **Real-time Feedback**: Trajectory prediction updates while dragging
4. **Release**: Velocity is set, handle remains visible
5. **Deselection**: Click elsewhere to hide handle

### Visual Feedback
- Arrow pulses subtly when hovered
- Different color when actively dragging
- Small velocity magnitude label near arrow tip

## 3. Info Panel (Right Side)

A collapsible panel showing information about the selected body.

### Layout (Expanded)
```
┌──────────────────────┐
│ ◀ Earth             │
├──────────────────────┤
│ Type: Planet         │
│                      │
│ Position:            │
│   X: 147.1 M km      │
│   Y: -32.4 M km      │
│   (or: 0.98 AU)      │
│                      │
│ Velocity:            │
│   29.78 km/s         │
│   (or: 0.017 AU/day) │
│                      │
│ [km/AU toggle]       │
└──────────────────────┘
```

### Layout (Collapsed)
```
┌──┐
│ ▶│
└──┘
```

### Components
- **Collapse Button** (`◀/▶`): Toggle panel visibility
- **Body Name**: Name of selected celestial body or asteroid
- **Type**: Planet, Moon, Asteroid, Sun
- **Position**: X, Y coordinates in selected unit system
- **Velocity**: Speed and optionally direction
- **Unit Toggle**: Switch between km and AU display

### Behavior
- Updates in real-time as simulation runs
- For asteroids, shows orbital elements if on stable orbit
- For planets/moons, shows orbital period

## 4. Scenario Menu

Modal dialog for scenario selection.

### Trigger
- Menu button in top-left corner
- Keyboard shortcut: Escape or M

### Layout
```
┌────────────────────────────────────────────┐
│           Select Scenario                  │
├────────────────────────────────────────────┤
│  ○ Apophis Approach                        │
│    Near-Earth asteroid on collision course │
│                                            │
│  ○ Jupiter Gravity Assist                  │
│    Plan a flyby to change trajectory       │
│                                            │
│  ○ Earth-Moon System                       │
│    Asteroid orbiting in cislunar space     │
│                                            │
│  ○ Hyperbolic Comet                        │
│    Interstellar object passing through     │
│                                            │
│  ○ Sandbox                                 │
│    Empty canvas - place asteroid anywhere  │
├────────────────────────────────────────────┤
│        [Cancel]         [Load]             │
└────────────────────────────────────────────┘
```

### Behavior
- Pauses simulation while open
- Shows brief description of each scenario
- "Load" replaces current state with scenario
- "Cancel" returns to current simulation

## 5. Impact Overlay

Full-screen overlay shown when asteroid collides with a planet.

### Layout
```
┌────────────────────────────────────────────┐
│                                            │
│                                            │
│              💥 IMPACT! 💥                 │
│                                            │
│         Asteroid collided with Earth       │
│                                            │
│     [Reset Scenario]    [New Scenario]     │
│                                            │
└────────────────────────────────────────────┘
```

### Behavior
- Simulation pauses automatically
- Flash effect on collision
- Semi-transparent dark overlay
- Shows which body was impacted
- Two options:
  - **Reset Scenario**: Return to scenario's initial state
  - **New Scenario**: Open scenario selection menu

## 6. Camera Controls (No UI - Input Only)

Controls handled via mouse/keyboard without visible UI elements.

### Mouse
- **Scroll Wheel**: Zoom in/out (logarithmic)
- **Middle Mouse Drag**: Pan camera
- **Left Mouse Drag** (on background): Pan camera
- **Left Click** (on body): Select body
- **Double-Click** (on body): Center camera on body

### Keyboard
- **Space**: Play/pause
- **1, 2, 3, 4**: Set time scale
- **R**: Reset to scenario start
- **Escape**: Open scenario menu
- **+/-**: Zoom in/out

## 7. Visual Indicators

### Selection Highlight
- Selected bodies have a subtle glow or ring
- Different color for asteroid vs planets

### Trajectory Line
- Colored line showing predicted path
- Gradient fade as it extends into future
- Different color/style for past trajectory vs prediction

### Orbit Paths (Optional Toggle)
- Faint ellipses showing planetary orbits
- Toggle via keyboard (O key) or menu

## 8. Implementation Notes

### bevy_egui Integration
- Use `egui::SidePanel` for info panel
- Use `egui::TopBottomPanel` for time controls
- Use `egui::Window` for scenario menu and impact overlay
- Style with dark theme to match space aesthetic

### Responsiveness
- Panels should handle window resize gracefully
- Minimum window size: 1024x768
- Info panel collapses automatically on narrow windows

### Accessibility
- All interactive elements keyboard-accessible
- Sufficient contrast for text
- Tooltips on hover for buttons
