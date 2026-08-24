const WALK_SPEED_PX_PER_SEC: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Walk,
}

pub struct Frame {
    pub clip: &'static str,
    pub x: f64,
    pub facing_right: bool,
}

pub struct Motion {
    state: State,
    x: f64,
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
        }

        Frame {
            clip: match self.state {
                State::Idle => "idle",
                State::Walk => "walk",
            },
            x: self.x,
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
}
