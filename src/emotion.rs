/// Number of animation ticks an emotion clip is shown for once triggered.
/// At the 16ms tick interval `main.rs` runs, this is ~3 seconds.
pub const EMOTION_TICKS: u32 = 188;

/// Temporarily overrides which clip the pet displays — set by an incoming
/// `bridge::BridgeMessage::SetEmotion`, independent of `motion::Motion`'s
/// own idle/walk/drag/fall state. `main.rs`'s tick loop checks this first
/// each frame and only falls back to the motion-driven clip once it
/// expires.
#[derive(Default)]
pub struct EmotionOverride {
    clip: Option<String>,
    ticks_remaining: u32,
}

impl EmotionOverride {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts (or restarts) showing `clip` for `ticks` ticks.
    pub fn set(&mut self, clip: String, ticks: u32) {
        self.clip = Some(clip);
        self.ticks_remaining = ticks;
    }

    /// Advances one tick and returns the clip to show, if the override is
    /// still active.
    pub fn tick(&mut self) -> Option<&str> {
        if self.ticks_remaining == 0 {
            self.clip = None;
            return None;
        }
        self.ticks_remaining -= 1;
        self.clip.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_the_clip_for_the_requested_number_of_ticks_then_expires() {
        let mut e = EmotionOverride::new();
        e.set("thinking".to_string(), 2);
        assert_eq!(e.tick(), Some("thinking"));
        assert_eq!(e.tick(), Some("thinking"));
        assert_eq!(e.tick(), None);
        assert_eq!(e.tick(), None);
    }

    #[test]
    fn a_new_set_call_restarts_the_countdown() {
        let mut e = EmotionOverride::new();
        e.set("sad".to_string(), 1);
        assert_eq!(e.tick(), Some("sad"));
        assert_eq!(e.tick(), None);

        e.set("happy".to_string(), 2);
        assert_eq!(e.tick(), Some("happy"));
        assert_eq!(e.tick(), Some("happy"));
        assert_eq!(e.tick(), None);
    }

    #[test]
    fn inactive_by_default() {
        let mut e = EmotionOverride::new();
        assert_eq!(e.tick(), None);
    }
}
