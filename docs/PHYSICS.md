# Physics Specification

## 1. The Integrator: Velocity Verlet
We use **Velocity Verlet** (a Symplectic Integrator).
*   **Why?** Unlike standard Euler (which drifts energy) or RK4 (which is computationally expensive for thousands of prediction points), Verlet is **Symplectic**. It conserves energy/momentum over long periods, making orbits stable rather than spiraling out.

### The Algorithm
For a timestep $\Delta t$:
1.  **Half-Velocity Update:** $v_{0.5} = v_t + 0.5 \cdot a_t \cdot \Delta t$
2.  **Position Update:** $p_{t+1} = p_t + v_{0.5} \cdot \Delta t$
3.  **Recalculate Forces:** Calculate gravity at $p_{t+1}$ to get new acceleration $a_{t+1}$.
4.  **Full-Velocity Update:** $v_{t+1} = v_{0.5} + 0.5 \cdot a_{t+1} \cdot \Delta t$

## 2. Gravity Model
Newtonian Gravity: $F = G \frac{m_1 m_2}{r^2}$
*   **G (Gravitational Constant):** For game feel, do not use real $6.67 \times 10^{-11}$. Pick a constant that makes 1 meter $\approx$ 1 pixel or unit for easier debugging, or use real units and scale the camera zoom.
*   **Optimization:** When calculating forces for the asteroids, sum the forces from all active celestial bodies.

## 3. The Prediction Loop
When the player drags a maneuver node, we must predict the future path.
*   **Method:** Run the Velocity Verlet loop $N$ times inside a `while` loop.
*   **Critical:** Inside the prediction loop, you cannot use the *current* planet positions. You must query `Ephemeris::get_pos(t_current + prediction_step)` because the planets will move while the asteroid travels.
