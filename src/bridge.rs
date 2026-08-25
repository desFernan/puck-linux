//! A minimal local socket bridge between the pet overlay and an agent
//! front end (`puck-agent`/`puck-client`), matching puck-mac's
//! pet-talks-to-client-over-a-local-socket architecture. This first slice
//! carries one message: "show this emotion clip for a bit" — enough for
//! an agent to make the pet visibly react (thinking/happy/sad) without the
//! pet knowing anything about the agent's internals.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Overrides the well-known socket path when set — used by tests so they
/// don't collide with a real running pet/agent on the same machine.
pub const SOCKET_ENV_OVERRIDE: &str = "PUCK_BRIDGE_SOCKET";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeMessage {
    /// Show `clip` (an avatar clip name, e.g. "thinking"/"happy"/"sad") in
    /// place of whatever the pet would otherwise be doing, for a few
    /// seconds. If the avatar has no such clip the pet falls back to idle
    /// — see `main.rs`'s clip-to-path lookup.
    SetEmotion { clip: String },
}

/// Where the bridge socket lives: `$PUCK_BRIDGE_SOCKET` if set (tests),
/// else `$XDG_RUNTIME_DIR/puck.sock`, else a temp-dir fallback.
pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var(SOCKET_ENV_OVERRIDE) {
        return PathBuf::from(p);
    }
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    dir.join("puck.sock")
}

/// Binds a fresh listener at `path`, clearing any stale socket file left
/// behind by a previous run (Unix sockets don't get cleaned up on an
/// unclean exit).
pub fn listen(path: &Path) -> std::io::Result<UnixListener> {
    let _ = std::fs::remove_file(path);
    UnixListener::bind(path)
}

/// Reads one newline-delimited JSON message from `stream`. `Ok(None)`
/// means the peer closed the connection cleanly without sending anything
/// further.
pub fn read_message(stream: &UnixStream) -> std::io::Result<Option<BridgeMessage>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let message = serde_json::from_str(line.trim())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(message))
}

/// Connects to the bridge socket at `path` and sends one message.
/// Best-effort by design at the call site (agents ignore the error — the
/// pet overlay may simply not be running).
pub fn send_to(path: &Path, message: &BridgeMessage) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(path)?;
    let json = serde_json::to_string(message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(stream, "{json}")?;
    Ok(())
}

/// Connects to the well-known bridge socket (`socket_path()`) and sends
/// one message. Best-effort — callers should ignore the error.
pub fn send(message: &BridgeMessage) -> std::io::Result<()> {
    send_to(&socket_path(), message)
}

/// Convenience wrapper for agent front ends: best-effort tells the pet
/// overlay to show `clip` for a few seconds. Silently does nothing if no
/// pet is running (no listener at the socket) — an agent shouldn't fail or
/// even log just because there's no pet to react.
pub fn notify_emotion(clip: &str) {
    let _ = send(&BridgeMessage::SetEmotion {
        clip: clip.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "puck-bridge-test-{name}-{}.sock",
            std::process::id()
        ))
    }

    #[test]
    fn round_trips_a_set_emotion_message_over_a_real_socket() {
        let path = unique_socket_path("roundtrip");
        let listener = listen(&path).unwrap();

        let sender = std::thread::spawn({
            let path = path.clone();
            move || {
                send_to(
                    &path,
                    &BridgeMessage::SetEmotion {
                        clip: "thinking".to_string(),
                    },
                )
                .unwrap();
            }
        });

        let (stream, _) = listener.accept().unwrap();
        let received = read_message(&stream).unwrap();
        sender.join().unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            received,
            Some(BridgeMessage::SetEmotion {
                clip: "thinking".to_string()
            })
        );
    }

    #[test]
    fn listen_removes_a_stale_socket_file_left_by_a_previous_run() {
        let path = unique_socket_path("stale");
        std::fs::write(&path, b"not a real socket").unwrap();

        let listener = listen(&path);
        assert!(
            listener.is_ok(),
            "listen should clear the stale file and bind cleanly"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_message_returns_none_on_a_clean_close_with_nothing_sent() {
        let path = unique_socket_path("empty-close");
        let listener = listen(&path).unwrap();

        let closer = std::thread::spawn({
            let path = path.clone();
            move || {
                let _ = UnixStream::connect(&path).unwrap();
                // dropped immediately, closing the connection
            }
        });

        let (stream, _) = listener.accept().unwrap();
        let received = read_message(&stream).unwrap();
        closer.join().unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(received, None);
    }

    #[test]
    fn socket_path_honors_the_env_override() {
        std::env::set_var(SOCKET_ENV_OVERRIDE, "/tmp/example-override.sock");
        assert_eq!(socket_path(), PathBuf::from("/tmp/example-override.sock"));
        std::env::remove_var(SOCKET_ENV_OVERRIDE);
    }
}
