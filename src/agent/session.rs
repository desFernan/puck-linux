use super::client::Message;

/// Conversation history for one chat session. The Messages API is
/// stateless — every request resends the full transcript — so this is
/// just an ordered, role-tagged list of content blocks.
pub struct Session {
    messages: Vec<Message>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Replaces the full history in one atomic step. `turn::run_turn`
    /// builds a whole turn (the user's message, any tool round trips, the
    /// final reply) on a private copy and calls this exactly once, on
    /// success or on giving up — never incrementally as the turn
    /// progresses. A mid-turn network failure after only some of a turn's
    /// messages were pushed would otherwise leave the real session with an
    /// unpaired `tool_use` or a trailing user message with no reply, and
    /// the Messages API rejects a conversation shaped like that outright,
    /// on every future request — permanently breaking the session until
    /// the process restarts.
    pub fn commit(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::client::ContentBlock;

    #[test]
    fn starts_empty() {
        let s = Session::new();
        assert!(s.messages().is_empty());
    }

    #[test]
    fn commit_replaces_the_full_history_atomically() {
        let mut s = Session::new();
        s.commit(vec![Message {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: "hi".to_string(),
            }],
        }]);
        assert_eq!(s.messages().len(), 1);

        // A later commit fully replaces the prior one, not appends to it.
        s.commit(vec![
            Message {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "hi".to_string(),
                }],
            },
            Message {
                role: "assistant".to_string(),
                content: vec![ContentBlock::Text {
                    text: "hello".to_string(),
                }],
            },
        ]);
        assert_eq!(s.messages().len(), 2);
        assert_eq!(s.messages()[1].role, "assistant");
    }
}
