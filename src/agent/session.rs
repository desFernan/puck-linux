use super::client::{ContentBlock, Message};

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

    pub fn push_user_text(&mut self, text: &str) {
        self.messages.push(Message {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        });
    }

    pub fn push_assistant(&mut self, content: Vec<ContentBlock>) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content,
        });
    }

    pub fn push_tool_results(&mut self, content: Vec<ContentBlock>) {
        self.messages.push(Message {
            role: "user".to_string(),
            content,
        });
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

    #[test]
    fn push_user_text_appends_a_user_message() {
        let mut s = Session::new();
        s.push_user_text("hi");
        assert_eq!(s.messages().len(), 1);
        assert_eq!(s.messages()[0].role, "user");
        assert_eq!(
            s.messages()[0].content,
            vec![ContentBlock::Text {
                text: "hi".to_string()
            }]
        );
    }

    #[test]
    fn push_assistant_and_tool_results_preserve_order() {
        let mut s = Session::new();
        s.push_user_text("run ls");
        s.push_assistant(vec![ContentBlock::ToolUse {
            id: "toolu_1".to_string(),
            name: "run_shell".to_string(),
            input: serde_json::json!({"command": "ls"}),
        }]);
        s.push_tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: "toolu_1".to_string(),
            content: "file.txt".to_string(),
            is_error: None,
        }]);

        let msgs = s.messages();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[2].role, "user");
    }
}
