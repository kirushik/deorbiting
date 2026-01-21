# UI/UX Design Guidelines

This document establishes the design principles and patterns for the orbital mechanics simulator interface. These guidelines are grounded in proven HCI research and best practices.

## Foundational Principles

Our design philosophy draws from four primary sources:

### 1. Direct Manipulation (Apple HIG)

> "When people directly manipulate onscreen objects instead of using separate controls to manipulate them, they're more engaged with the task and more readily understand the results of their actions."

**Application:**
- The viewport IS the interface—users grab and move objects directly
- Drag asteroids to reposition them, drag velocity arrows to change trajectories
- Click directly on objects to select them, not through a list or menu
- Zoom and pan with standard gestures (scroll, drag background)

### 2. Modelessness (Jef Raskin)

> "Modes lead to errors because humans simply forget which mode they're in."

**Application:**
- No placement modes—clicking empty space always spawns an asteroid
- No special velocity editing mode—velocity arrows are always visible and draggable
- Every action works the same way regardless of prior state
- Auto-pause on edit, auto-resume on release (invisible mode management)

### 3. Data-Ink Maximization (Edward Tufte)

> "Above all else, show the data."

**Application:**
- Minimize chrome and decoration—the simulation is the content
- No gratuitous visual effects or "chartjunk"
- Every pixel should either show orbital data or provide necessary controls
- Trajectory lines show information; remove if they don't add value

### 4. Continuous Feedback (Илья Бирман)

> "If feedback is continuous, gentle, visual, and consistent, users can learn to use even the most complex interface."

**Application:**
- Immediate visual response to every action
- Hover states show what can be clicked/dragged
- Selection highlights are clear but not overwhelming
- Progress indicators for ongoing operations (deflection missions)

---

## Typography

### Font Choice: Inter

We embed **Inter** as the primary UI font. Inter was designed specifically for computer screens by Rasmus Andersson at Figma.

**Why Inter:**
- **Tall x-height** improves legibility at small sizes (11-14px)
- **Open apertures** aid character recognition on dark backgrounds
- **Optimized for screens** with careful hinting and rendering

**Weight Selection:**
We use **Inter Light (300)** for body text to compensate for halation—the phenomenon where light text on dark backgrounds appears heavier than intended. Research suggests using weight ~350 in dark mode for visual equivalence to 400 in light mode. Since Inter doesn't offer 350, Light (300) is the best choice.

Note: Our backgrounds are dark blue-grey (#1a1a24), not pure black, which reduces halation compared to white-on-black. If Light appears too thin in practice, switching to Regular (400) is acceptable.

**egui Limitation:**
egui does not support variable font axes ([discussion #1862](https://github.com/emilk/egui/discussions/1862)). Fonts are treated as static shapes—we cannot adjust weight, optical size, or grade at runtime. This is why we embed pre-selected static font files.

```rust
// Primary font (UI elements, labels)
FontFamily::Proportional  // Inter Light (300)

// Monospace (data values, coordinates, code-like content)
FontFamily::Monospace     // System monospace
```

### Type Scale

| Purpose | Size | Weight | Example |
|---------|------|--------|---------|
| Card titles | 16px | Semibold | "Asteroid-1" |
| Primary data | 14px | Regular | "147.1 M km from Sun" |
| Secondary data | 13px | Regular | "25.3 km/s velocity" |
| Labels/hints | 12px | Regular | "Mass:" |
| Micro text | 11px | Regular | "cont." (type indicators) |
| Dock controls | 14-16px | Medium | Speed labels |

### Dark Mode Typography Rules

Our UI uses a dark background, which requires specific typography considerations:

**1. Never use italics for readable content**
- Italics have poor subpixel rendering on screens—angled strokes don't align with pixel grid
- Light-on-dark makes this worse: italic text appears blurry and thin
- **Use regular weight only**. For emphasis, use color, weight, or icons instead

**2. Font weight for dark mode**
- Light text on dark backgrounds appears heavier due to halation (light bleeding into dark)
- Inter Light (300) is used as our base to compensate for this optical effect
- Never use Thin (100) weights—strokes are too thin for dark backgrounds

**3. Minimum sizes are larger than light mode**
- 13px minimum for any text that must be read
- 14px preferred for body text
- 11-12px only for truly supplementary labels (like "cont." indicators)

**4. Tinted backgrounds reduce contrast**
- A semi-transparent colored overlay on dark backgrounds makes text HARDER to read
- If you want visual separation, use: borders, spacing, or a DARKER background
- Never rely on subtle color differences for visual hierarchy

**5. Use off-white, not pure white**
- Pure white (#FFFFFF) on dark backgrounds causes eye strain and "glowing" effect
- Use off-white (#dcdce6) for primary text
- Reserve brighter whites only for interactive elements on hover

**What NOT to do:**
- ❌ Italic text for any readable content
- ❌ Semi-transparent colored backgrounds behind text
- ❌ Text smaller than 13px for anything important
- ❌ Light/thin font weights
- ❌ Low contrast "subtle" text that users need to read

### Alignment Rules

- **Baseline alignment**: All text in horizontal layouts must align on baseline
- **Numeric alignment**: Right-align numbers for easy comparison
- **Consistent line-height**: Use 1.4× for body text, 1.2× for headings

### Toolbar/Dock Layout Rules

These rules prevent common egui alignment bugs:

1. **Single Layout Level**
   - A toolbar should be ONE `horizontal_centered()` layout
   - NEVER nest `ui.horizontal()` inside it—this breaks vertical centering
   - Group elements by adjusting spacing, not by nesting containers

2. **Fixed-Width Stable Elements**
   - Any element with changing content MUST have fixed width
   - Date displays: use fixed-width container (e.g., 120px)
   - Prevents layout "jumping" when content changes

3. **Uniform Element Height**
   - ALL toolbar elements use the same height constant
   - Use `ui.add_sized([width, HEIGHT], widget)` for explicit sizing
   - Labels, buttons, separators—all same height

4. **Element Sizing Pattern**
   ```rust
   // GOOD: explicit size, no nesting
   ui.add_sized([120.0, HEIGHT], egui::Label::new(text));
   ui.add_sized([42.0, HEIGHT], egui::Button::new(icon));

   // BAD: nested horizontal breaks centering
   ui.horizontal(|ui| {  // DON'T DO THIS
       ui.add(button1);
       ui.add(button2);
   });
   ```

---

## Color Palette

### Background Colors

| Element | Color | Hex |
|---------|-------|-----|
| Space background | Near-black | `#0a0a12` |
| Panel/card fill | Dark translucent | `rgba(26, 26, 36, 0.9)` |
| Elevated surface | Slightly lighter | `rgba(40, 40, 55, 1.0)` |
| Hover state | Elevated + 10% | `rgba(50, 50, 70, 1.0)` |

### Semantic Colors

| Meaning | Color | Usage |
|---------|-------|-------|
| Danger/collision | `#e05555` | Warnings, delete buttons |
| Success/safe | `#55b055` | Stable orbits, completion |
| Active/accent | `#5599dd` | Selected items, primary actions |
| Velocity | `#55dd88` | Velocity arrows, active speed |
| Caution | `#ddaa55` | Paused state, warnings |

### Text Colors

| Type | Color | Usage |
|------|-------|-------|
| Primary text | `#dcdce6` (220, 220, 230) | Main labels, values, buttons |
| Secondary text | `#b4b4be` (180, 180, 190) | Supplementary info, type labels, hints |
| Disabled text | `#787882` (120, 120, 130) | **ONLY** for disabled/unavailable elements |

### High-Contrast Text Principle

**Use grey text ONLY for truly disabled elements.** All readable text must use primary or secondary colors. The "disabled grey" (120, 120, 130) should ONLY appear on elements that cannot be interacted with.

Key guidelines:
- Buttons that can be clicked should never use disabled grey
- Supplementary information uses secondary text, not disabled grey
- All readable text must have ≥4.5:1 contrast ratio
- When in doubt, use primary text color
- Never use italics—see "Dark Mode Typography Rules" above

---

## Icons

### Phosphor Icon Font

We use the [Phosphor](https://phosphoricons.com/) icon font via the `egui_phosphor` crate. All icons are defined in `src/ui/icons.rs` with semantic names.

**Critical Rule: Always use Phosphor icons, never Unicode symbols or ASCII art.**

| Bad | Good | Reason |
|-----|------|--------|
| `"*"` | `icons::ASTEROID` | ASCII asterisk renders as Inter glyph |
| `"→"` | `icons::ARROW_RIGHT` | Unicode arrows may render incorrectly |
| `"●"` | Use a Phosphor icon | Unicode symbols conflict with Inter |
| `"✓"` | `icons::CHECK` | Phosphor has consistent styling |

**Why this matters:**
- Inter is our primary font for text readability
- Inter renders ALL characters it has glyphs for, including some symbols
- Phosphor icons use Private Use Area (PUA) characters that Inter doesn't have
- Font fallback only works when the primary font lacks the glyph
- Mixing symbol fonts creates inconsistent visual weight and style

**Icon usage pattern:**
```rust
use crate::ui::icons;

// Good: use the icon() helper function
ui.label(icons::icon(icons::SUCCESS, 16.0));
ui.label(icons::icon_colored(icons::WARNING, 16.0, Color32::RED));

// Also good: semantic icon constant with RichText
ui.label(egui::RichText::new(icons::SUCCESS).size(16.0));

// Bad: raw Unicode or ASCII
ui.label(egui::RichText::new("✓").size(16.0));
ui.label(egui::RichText::new("*").size(16.0));
```

### Font Loading Order

Icons use a **named font family** (`FontFamily::Name("phosphor")`) for explicit rendering, while text uses the Proportional family with Inter as primary.

```
Proportional: [Inter, ...system defaults...]  → renders all regular text
Named "phosphor": [Phosphor]                  → renders icon characters explicitly
```

**Why explicit font family for icons:**
- Font fallback is unreliable—system fonts have fallback glyphs for PUA codepoints
- Using `FontFamily::Name("phosphor")` bypasses fallback entirely
- The `icons::icon()` helper automatically uses the named family
- UI systems wait one frame after font initialization to ensure the named family is registered

---

## Spacing and Layout

### Grid System

- **Base unit**: 4px
- **Common spacings**: 4, 8, 12, 16, 20, 24px

### Component Dimensions

| Element | Dimension |
|---------|-----------|
| Dock height | 56px |
| Touch target minimum | 32×32px (ideally 44×44px) |
| Card padding | 12px |
| Card max-width | 200px |
| Button min-size | 32×28px |
| Icon size (inline) | 16-18px |
| Icon size (feature) | 28px |

### Margins and Offsets

- **Context card offset**: 30px right, 50px up from selection point
- **Radial menu radius**: 90px from center
- **Drawer height**: 200px (animated)

---

## Interaction Patterns

### Click Behavior

| Target | Action |
|--------|--------|
| Empty space | Spawn asteroid |
| Asteroid body | Select it |
| Planet body | Select it (info only) |
| Velocity arrow tip | Begin velocity drag |
| Background | Begin pan |
| UI control | Activate control |

### Drag Behavior

| Target | Action |
|--------|--------|
| Asteroid body | Move position (auto-pauses) |
| Velocity arrow | Change velocity vector |
| Background | Pan camera |
| Empty space | Box selection |

### Keyboard Shortcuts

Keep shortcuts discoverable (shown in help tooltip) and consistent:

| Key | Action | Rationale |
|-----|--------|-----------|
| Space | Play/Pause | Universal media control |
| 1-4 | Speed levels | Direct numeric access |
| R | Reset | Mnemonic |
| Esc | Close/cancel or open scenarios | Standard dismiss |
| Del | Delete selected | Standard |
| +/- | Zoom | Standard |

---

## Information Architecture

### Principle of Proximity

Information about an object should appear near that object, not in a distant panel.

**Good**: Context card floats near selected asteroid
**Bad**: Info panel fixed at screen edge

### Progressive Disclosure

Show essential information immediately; details on demand.

**Level 1** (always visible):
- Object position (via viewport)
- Velocity arrow
- Trajectory prediction

**Level 2** (on selection):
- Distance from Sun
- Current velocity magnitude
- Mass

**Level 3** (on explicit action):
- Detailed orbital elements
- Mission parameters
- Historical data

### Information Hierarchy in Cards

1. **Identity** (icon + name) — top
2. **Key metrics** (distance, velocity, mass) — middle
3. **Active status** (ongoing deflection) — if applicable
4. **Actions** (deflect, delete) — bottom

---

## Animation and Feedback

### Timing Guidelines

| Animation | Duration | Easing |
|-----------|----------|--------|
| Hover state | 100ms | ease-out |
| Panel open/close | 150ms | ease-in-out |
| Drawer slide | 125ms | ease-out |
| Selection highlight | 200ms | ease-out |

### Feedback Principles

1. **Immediate**: Response within 100ms feels instant
2. **Continuous**: Drag operations update every frame
3. **Proportional**: Big actions get bigger feedback
4. **Non-blocking**: Feedback never prevents next action

---

## Accessibility

### Minimum Requirements

- Touch targets ≥32×32px (44×44px ideal)
- Color contrast ratio ≥4.5:1 for text
- Never convey information by color alone
- Keyboard navigable for all actions

### Visual Clarity

- Distinct visual states: default, hover, selected, disabled
- Clear focus indicators
- Adequate spacing between interactive elements

---

## Anti-Patterns to Avoid

### Chartjunk (Tufte)
- Decorative gradients that don't convey information
- Drop shadows for visual "depth" that doesn't aid understanding
- Animated elements that distract from content

### Modal Interruption (Raskin)
- Dialogs that block all other interaction
- "Are you sure?" confirmations for reversible actions
- Modes that change meaning of basic gestures

### Information Scatter
- Related info split across distant screen regions
- Requiring memory of previous screens
- Hidden state that affects visible behavior

### Premature Optimization
- Controls for features nobody uses
- Preferences panels instead of sensible defaults
- Exposing implementation details to users

---

## Sources

- [Apple Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines)
- [The Humane Interface](https://en.wikipedia.org/wiki/The_Humane_Interface) — Jef Raskin
- [The Visual Display of Quantitative Information](https://www.edwardtufte.com/tufte/books_vdqi) — Edward Tufte
- [Ководство](https://www.artlebedev.ru/kovodstvo/sections/) — Артемий Лебедев
- [Пользовательский интерфейс](https://bureau.ru/projects/book-ui/) — Илья Бирман

---

## Changelog

- 2026-01-21: Initial version based on UX redesign phase
