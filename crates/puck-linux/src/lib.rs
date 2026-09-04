//! The pet half of Puck: reading an avatar package, the motion state
//! machine that walks and drops it around the screen, the agent-driven
//! emotion override, and the always-on-top transparent window it is drawn
//! in. The agent half is `puck-core`.
pub mod avatar;
pub mod emotion;
pub mod motion;
pub mod window;
