const WALK_SPEED_PX_PER_SEC: f64 = 40.0;
const GRAVITY_PX_PER_SEC2: f64 = 800.0;
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
    hitbox_height: f64,
    screen_width: f64,
    // The sprite's resting y (top-left corner) — screen_height minus the
    // sprite's own height, so its bottom edge sits on the screen's bottom
    // edge. Idle/Walk hold y here; a Fall ends (lands) when y reaches it
    // too, so walking and landing share the same ground instead of the
    // sprite drifting at y=0 (visually "floating" at the top of the
    // screen) while only falls knew about a ground level.
    ground_y: f64,
    time_in_state: f64,
    idle_duration: f64,
    walk_duration: f64,
}

impl Motion {
    pub fn new(
        hitbox_width: f64,
        hitbox_height: f64,
        screen_width: f64,
        screen_height: f64,
    ) -> Self {
        let ground_y = (screen_height - hitbox_height).max(0.0);
        Self {
            state: State::Idle,
            x: 0.0,
            y: ground_y,
            drag_anchor: (0.0, 0.0),
            fall_velocity: 0.0,
            facing_right: true,
            hitbox_width,
            hitbox_height,
            screen_width,
            ground_y,
            time_in_state: 0.0,
            idle_duration: 3.0,
            walk_duration: 4.0,
        }
    }

    /// Recomputes `ground_y` for a new screen size (a display was
    /// reconfigured — resolution changed, a monitor was added/removed).
    /// If the sprite was standing on the ground when this happens:
    /// - a shorter screen means the new floor rose up past it, and
    ///   nothing falls *upward*, so it's pulled directly onto the new
    ///   floor;
    /// - a taller screen means the sprite is still inside the visible
    ///   area but now has nothing under it, so it's dropped into `Fall`
    ///   and gravity carries it down to the new floor the normal way.
    ///
    /// Dragging or already-falling sprites are left alone — the fall
    /// already lands on whatever `ground_y` is by the time it gets there.
    pub fn update_screen_size(&mut self, screen_width: f64, screen_height: f64) {
        let new_ground_y = (screen_height - self.hitbox_height).max(0.0);
        self.screen_width = screen_width;

        let was_grounded = matches!(self.state, State::Idle | State::Walk | State::Land)
            && self.y >= self.ground_y;
        self.ground_y = new_ground_y;

        if was_grounded {
            if self.y > new_ground_y {
                self.y = new_ground_y;
            } else if self.y < new_ground_y {
                self.state = State::Fall;
                self.fall_velocity = 0.0;
                self.time_in_state = 0.0;
            }
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
                if self.y >= self.ground_y {
                    self.y = self.ground_y;
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
        let mut m = Motion::new(50.0, 50.0, 800.0, 50.0);
        let f = m.tick(0.0);
        assert_eq!(f.clip, "idle");
        assert_eq!(f.x, 0.0);
    }

    #[test]
    fn walking_moves_right_and_faces_right() {
        let mut m = Motion::new(50.0, 50.0, 800.0, 50.0);
        m.force_state_for_test(State::Walk, true);
        let f = m.tick(1.0);
        assert_eq!(f.clip, "walk");
        assert!(f.x > 0.0);
        assert!(f.facing_right);
    }

    #[test]
    fn starts_and_stays_at_the_screens_ground_not_the_top() {
        // hitbox 50 tall, screen 200 tall -> ground_y = 150 (sprite's
        // bottom edge sits on the screen's bottom edge). Idle and Walk
        // must both hold y there, not drift to 0 (the top of the screen).
        let mut m = Motion::new(50.0, 50.0, 800.0, 200.0);
        assert_eq!(m.tick(0.0).y, 150.0);

        m.force_state_for_test(State::Walk, true);
        assert_eq!(m.tick(1.0).y, 150.0);
    }

    #[test]
    fn turns_around_at_right_edge() {
        let mut m = Motion::new(50.0, 50.0, 100.0, 50.0);
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
        // ground_y = 750 here (screen 800 tall, hitbox 50). The sprite
        // starts standing on it; dragging up (negative y offset) lifts it
        // off the ground so releasing it has room to actually fall.
        let mut m = Motion::new(50.0, 50.0, 800.0, 800.0);
        let ground_y = 750.0;

        m.begin_drag();
        let f = m.tick(0.1);
        assert_eq!(f.clip, "idle"); // dragged sprite shows idle
        assert_eq!(f.x, 0.0);
        assert_eq!(f.y, ground_y);

        // offsets from the gesture's start point, not absolute coordinates
        m.drag_to(20.0, -100.0);
        let f = m.tick(0.1);
        assert_eq!(f.x, 20.0);
        assert_eq!(f.y, ground_y - 100.0);

        m.end_drag();
        let f = m.tick(0.1);
        assert_eq!(f.clip, "fall");
        assert!(
            f.y > ground_y - 100.0,
            "should have started falling back down (gravity increases y)"
        );
    }

    #[test]
    fn drag_offset_is_relative_to_the_sprites_position_at_drag_start() {
        let mut m = Motion::new(50.0, 50.0, 800.0, 50.0);
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
        let mut m = Motion::new(50.0, 50.0, 800.0, 800.0);
        m.begin_drag();
        m.end_drag();
        // Simulate a long fall; ground is y = ground_y (750 here).
        for _ in 0..1000 {
            let f = m.tick(0.05);
            if f.clip == "idle" {
                return; // reached idle again after landing - test passes
            }
        }
        panic!("never returned to idle after falling");
    }

    #[test]
    fn a_shrinking_screen_pulls_a_grounded_sprite_directly_onto_the_new_floor() {
        // hitbox 50, screen 200 -> ground_y = 150.
        let mut m = Motion::new(50.0, 50.0, 800.0, 200.0);
        assert_eq!(m.tick(0.0).y, 150.0);

        // Screen shrinks to 100 tall -> new ground_y = 50, above (smaller
        // y than) where the sprite is standing. Nothing falls upward, so
        // it must be placed on the new floor directly, not left floating
        // below the visible area.
        m.update_screen_size(800.0, 100.0);
        let f = m.tick(0.0);
        assert_eq!(f.y, 50.0);
        assert_eq!(
            f.clip, "idle",
            "should not be falling - it was placed directly"
        );
    }

    #[test]
    fn a_growing_screen_drops_a_grounded_sprite_to_the_new_floor() {
        // hitbox 50, screen 100 -> ground_y = 50.
        let mut m = Motion::new(50.0, 50.0, 800.0, 100.0);
        assert_eq!(m.tick(0.0).y, 50.0);

        // Screen grows to 800 tall -> new ground_y = 750. The sprite is
        // still inside the visible area but now has nothing under it, so
        // gravity should carry it down rather than leaving it floating.
        m.update_screen_size(800.0, 800.0);
        let f = m.tick(0.1);
        assert_eq!(f.clip, "fall");
        assert!(
            f.y > 50.0,
            "should have started falling toward the new floor"
        );
    }

    #[test]
    fn resizing_does_not_disturb_a_sprite_that_is_dragging_or_already_falling() {
        let mut m = Motion::new(50.0, 50.0, 800.0, 200.0); // ground_y = 150
        m.begin_drag();
        m.drag_to(0.0, -100.0); // lifted well off the ground, y = 50
        let before = m.tick(0.0).y;

        m.update_screen_size(800.0, 400.0); // ground_y now 350, doesn't matter mid-drag
        let after = m.tick(0.0);
        assert_eq!(after.clip, "idle"); // still shows as dragged (idle clip)
        assert_eq!(
            after.y, before,
            "a resize must not move a sprite being actively dragged"
        );
    }
}
