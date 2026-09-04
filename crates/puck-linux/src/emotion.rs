/// How long an emotion clip is shown for once triggered.
pub const EMOTION_DURATION_SECS: f64 = 3.0;

/// Temporarily overrides which clip the pet displays — set by an incoming
/// `bridge::BridgeMessage::SetEmotion`, independent of `motion::Motion`'s
/// own idle/walk/drag/fall state. `main.rs`'s tick loop checks this first
/// each frame and only falls back to the motion-driven clip once it
/// expires.
///
/// Measured in seconds, not in frames: this used to count 188 ticks and
/// call that three seconds, which is only true while every frame arrives
/// on time. A busy machine, or a lid closed and opened again, stretched a
/// reaction well past the three seconds it was meant to last. `motion`
/// already steps by the time that actually passed; this now does too.
#[derive(Default)]
pub struct EmotionOverride {
    clip: Option<String>,
    seconds_remaining: f64,
}

impl EmotionOverride {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts (or restarts) showing `clip` for `seconds`.
    pub fn set(&mut self, clip: String, seconds: f64) {
        self.clip = Some(clip);
        self.seconds_remaining = seconds;
    }

    /// Advances by `dt_secs` and returns the clip to show, if the override
    /// is still active.
    pub fn tick(&mut self, dt_secs: f64) -> Option<&str> {
        if self.seconds_remaining <= 0.0 {
            self.clip = None;
            return None;
        }
        self.seconds_remaining -= dt_secs;
        self.clip.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_the_clip_for_the_requested_time_then_expires() {
        let mut e = EmotionOverride::new();
        e.set("thinking".to_string(), 1.0);
        assert_eq!(e.tick(0.5), Some("thinking"));
        assert_eq!(e.tick(0.5), Some("thinking"));
        assert_eq!(e.tick(0.5), None);
        assert_eq!(e.tick(0.5), None);
    }

    #[test]
    fn a_long_frame_expires_the_clip_in_one_step() {
        // The whole point of measuring seconds: a frame that took a second
        // spends a second of the reaction, not one frame's worth of it.
        let mut e = EmotionOverride::new();
        e.set("happy".to_string(), 3.0);
        assert_eq!(e.tick(3.5), Some("happy"));
        assert_eq!(e.tick(0.0), None);
    }

    #[test]
    fn a_new_set_call_restarts_the_countdown() {
        let mut e = EmotionOverride::new();
        e.set("sad".to_string(), 0.1);
        assert_eq!(e.tick(0.1), Some("sad"));
        assert_eq!(e.tick(0.1), None);

        e.set("happy".to_string(), 0.2);
        assert_eq!(e.tick(0.1), Some("happy"));
        assert_eq!(e.tick(0.1), Some("happy"));
        assert_eq!(e.tick(0.1), None);
    }

    #[test]
    fn inactive_by_default() {
        let mut e = EmotionOverride::new();
        assert_eq!(e.tick(0.016), None);
    }
}
