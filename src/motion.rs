const WALK_SPEED_PX_PER_SEC: f64 = 40.0;
const GRAVITY_PX_PER_SEC2: f64 = 800.0;
// Screen-down coordinates: `y` is the sprite's on-screen vertical position,
// increasing downward. `begin_drag` sets `y` directly from the cursor, and a
// fall increases it (moving down the screen) until it reaches `GROUND_Y`,
// which stands in for the ground the sprite lands on. Motion doesn't track
// the real monitor height (only `screen_width`, for walk turnaround), so
// this is a fixed fall distance rather than the actual bottom of the
// screen — good enough for this MVP slice; wiring the real screen height
// through is a natural follow-up if the fixed distance ever looks wrong.
const GROUND_Y: f64 = 500.0;
const LAND_DURATION_SECS: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Walk,
    Drag,
    Fall,
    Land,
}

pub struct Frame {
    pub clip: &'static str,
    pub x: f64,
    pub y: f64,
    pub facing_right: bool,
}

pub struct Motion {
    state: State,
    x: f64,
    y: f64,
    // Window position when the current drag started; drag_to's (offset_x,
    // offset_y) — deltas from the gesture's own start point, not absolute
    // coordinates — are added to this to get the new absolute position.
    drag_anchor: (f64, f64),
    fall_velocity: f64,
    facing_right: bool,
    hitbox_width: f64,
    screen_width: f64,
    time_in_state: f64,
    idle_duration: f64,
    walk_duration: f64,
}

impl Motion {
    pub fn new(hitbox_width: f64, screen_width: f64) -> Self {
        Self {
            state: State::Idle,
            x: 0.0,
            y: 0.0,
            drag_anchor: (0.0, 0.0),
            fall_velocity: 0.0,
            facing_right: true,
            hitbox_width,
            screen_width,
            time_in_state: 0.0,
            idle_duration: 3.0,
            walk_duration: 4.0,
        }
    }

    #[cfg(test)]
    pub fn force_state_for_test(&mut self, state: State, facing_right: bool) {
        self.state = state;
        self.facing_right = facing_right;
        self.time_in_state = 0.0;
    }

    /// Starts a drag from the sprite's current position. `drag_to` then
    /// takes offsets from the gesture's own start point, not absolute
    /// coordinates, so no position is passed in here.
    pub fn begin_drag(&mut self) {
        self.state = State::Drag;
        self.drag_anchor = (self.x, self.y);
        self.fall_velocity = 0.0;
        self.time_in_state = 0.0;
    }

    /// `offset_x`/`offset_y` are deltas from where the drag gesture began
    /// (as GTK's `GestureDrag` reports them), not absolute coordinates.
    /// Returns the new absolute position so callers can move the window
    /// immediately, without waiting for the next animation tick — a drag
    /// gesture can report updates faster than the tick interval, and
    /// waiting for the next tick to move the window makes dragging feel
    /// laggy.
    pub fn drag_to(&mut self, offset_x: f64, offset_y: f64) -> (f64, f64) {
        self.x = self.drag_anchor.0 + offset_x;
        self.y = self.drag_anchor.1 + offset_y;
        (self.x, self.y)
    }

    pub fn end_drag(&mut self) {
        self.state = State::Fall;
        self.time_in_state = 0.0;
    }

    pub fn tick(&mut self, dt_secs: f64) -> Frame {
        self.time_in_state += dt_secs;

        match self.state {
            State::Idle => {
                if self.time_in_state >= self.idle_duration {
                    self.state = State::Walk;
                    self.time_in_state = 0.0;
                }
            }
            State::Walk => {
                let delta = WALK_SPEED_PX_PER_SEC * dt_secs;
                let max_x = (self.screen_width - self.hitbox_width).max(0.0);

                if self.facing_right {
                    self.x += delta;
                    if self.x >= max_x {
                        self.x = max_x;
                        self.facing_right = false;
                    }
                } else {
                    self.x -= delta;
                    if self.x <= 0.0 {
                        self.x = 0.0;
                        self.facing_right = true;
                    }
                }

                if self.time_in_state >= self.walk_duration {
                    self.state = State::Idle;
                    self.time_in_state = 0.0;
                }
            }
            State::Drag => {
                // Position is driven by drag_to(); nothing to advance here.
            }
            State::Fall => {
                self.fall_velocity += GRAVITY_PX_PER_SEC2 * dt_secs;
                self.y += self.fall_velocity * dt_secs;
                if self.y >= GROUND_Y {
                    self.y = GROUND_Y;
                    self.state = State::Land;
                    self.time_in_state = 0.0;
                }
            }
            State::Land => {
                if self.time_in_state >= LAND_DURATION_SECS {
                    self.state = State::Idle;
                    self.time_in_state = 0.0;
                }
            }
        }

        Frame {
            clip: match self.state {
                State::Idle => "idle",
                State::Walk => "walk",
                State::Drag => "idle",
                State::Fall => "fall",
                State::Land => "land",
            },
            x: self.x,
            y: self.y,
            facing_right: self.facing_right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle_at_x_zero() {
        let mut m = Motion::new(50.0, 800.0);
        let f = m.tick(0.0);
        assert_eq!(f.clip, "idle");
        assert_eq!(f.x, 0.0);
    }

    #[test]
    fn walking_moves_right_and_faces_right() {
        let mut m = Motion::new(50.0, 800.0);
        m.force_state_for_test(State::Walk, true);
        let f = m.tick(1.0);
        assert_eq!(f.clip, "walk");
        assert!(f.x > 0.0);
        assert!(f.facing_right);
    }

    #[test]
    fn turns_around_at_right_edge() {
        let mut m = Motion::new(50.0, 100.0);
        m.force_state_for_test(State::Walk, true);
        // screen_width 100, hitbox 50 -> right edge for sprite's left-x is 50.
        // At 40px/s that's reached in 1.25s; walk_duration is 4s, so staying
        // well inside one walk episode (2s) avoids crossing into the next
        // idle/walk cycle, which would make the end state ambiguous.
        for _ in 0..20 {
            m.tick(0.1);
        }
        let f = m.tick(0.0);
        assert!(
            !f.facing_right,
            "should have turned around before falling off the right edge"
        );
        assert!(f.x <= 50.0);
        assert!(f.x >= 0.0);
    }

    #[test]
    fn drag_moves_freely_and_release_starts_fall() {
        let mut m = Motion::new(50.0, 800.0);
        m.begin_drag();
        let f = m.tick(0.1);
        assert_eq!(f.clip, "idle"); // dragged sprite shows idle
        assert_eq!(f.x, 0.0);
        assert_eq!(f.y, 0.0);

        // offsets from the gesture's start point, not absolute coordinates
        m.drag_to(20.0, 5.0);
        let f = m.tick(0.1);
        assert_eq!(f.x, 20.0);
        assert_eq!(f.y, 5.0);

        m.end_drag();
        let f = m.tick(0.1);
        assert_eq!(f.clip, "fall");
        assert!(
            f.y > 5.0,
            "should have started falling (gravity increases y)"
        );
    }

    #[test]
    fn drag_offset_is_relative_to_the_sprites_position_at_drag_start() {
        let mut m = Motion::new(50.0, 800.0);
        m.force_state_for_test(State::Walk, true);
        let x_before_drag = m.tick(2.0).x; // walk away from x = 0 first
        assert!(x_before_drag > 0.0);

        m.begin_drag();
        // A zero offset right after begin_drag must report the position the
        // sprite already had - not snap to (0, 0) or to the offset itself.
        m.drag_to(0.0, 0.0);
        assert_eq!(m.tick(0.0).x, x_before_drag);

        m.drag_to(10.0, 0.0);
        assert_eq!(m.tick(0.0).x, x_before_drag + 10.0);
    }

    #[test]
    fn fall_lands_at_ground_and_returns_to_idle() {
        let mut m = Motion::new(50.0, 800.0);
        m.begin_drag();
        m.end_drag();
        // Simulate a long fall; ground is y = GROUND_Y.
        for _ in 0..1000 {
            let f = m.tick(0.05);
            if f.clip == "idle" {
                return; // reached idle again after landing - test passes
            }
        }
        panic!("never returned to idle after falling");
    }
}
