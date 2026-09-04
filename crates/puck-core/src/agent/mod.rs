mod client;
mod session;
mod tools;
mod turn;

pub use client::{
    Client, ClientError, ContentBlock, Message, MessagesApi, MessagesResponse, ToolDefinition,
};
pub use session::Session;
pub use tools::{RunShell, ToolHandler, ToolOutcome};
pub use turn::{run_turn, Approver};

/// Reads simple `KEY=VALUE` lines from a `.env` file at `path` (if it
/// exists) and sets each as an environment variable unless it's already
/// set. Not a full dotenv parser (no quoting, no multiline values) —
/// matches puck-mac's documented `.env` credential file for this MVP.
/// Shared by both `puck-agent` and `puck-client` so their credential
/// handling can't drift apart.
pub fn load_dotenv(path: &std::path::Path) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value.trim());
            }
        }
    }
}

#[cfg(test)]
mod dotenv_tests {
    use super::load_dotenv;

    #[test]
    fn sets_vars_from_file_without_overriding_existing_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "# a comment\nPUCK_TEST_NEW=from-file\nPUCK_TEST_EXISTING=from-file\n\nPUCK_TEST_SPACED = trimmed \n",
        )
        .unwrap();

        std::env::remove_var("PUCK_TEST_NEW");
        std::env::set_var("PUCK_TEST_EXISTING", "from-env");
        std::env::remove_var("PUCK_TEST_SPACED");

        load_dotenv(&path);

        assert_eq!(std::env::var("PUCK_TEST_NEW").unwrap(), "from-file");
        assert_eq!(std::env::var("PUCK_TEST_EXISTING").unwrap(), "from-env");
        assert_eq!(std::env::var("PUCK_TEST_SPACED").unwrap(), "trimmed");
    }

    #[test]
    fn missing_file_is_a_silent_no_op() {
        load_dotenv(std::path::Path::new("/nonexistent/.env"));
    }
}
