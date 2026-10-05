use chrono::{DateTime, Utc};
use pulldown_cmark::{Event, Parser, html};
use serde::Serialize;

use crate::persistence::{Conversation, ConversationSummary, Message, Role};
use crate::uid::ConversationUid;

#[derive(Serialize)]
pub struct IndexContext {
    conversations: Vec<ConversationItem>,
}

#[derive(Serialize)]
struct ConversationItem {
    conversation_uid: ConversationUid,
    created_at: String,
}

impl From<Vec<ConversationSummary>> for IndexContext {
    fn from(conversations: Vec<ConversationSummary>) -> Self {
        let conversations = conversations
            .into_iter()
            .map(|c| ConversationItem {
                conversation_uid: c.conversation_uid,
                created_at: format_timestamp(c.created_at),
            })
            .collect();

        Self { conversations }
    }
}

#[derive(Serialize)]
pub struct ConversationContext {
    conversation_uid: ConversationUid,
    created_at: String,
    messages: Vec<MessageContext>,
}

#[derive(Serialize)]
struct MessageContext {
    role: &'static str,
    created_at: String,
    html: String,
}

impl From<Conversation> for ConversationContext {
    fn from(conversation: Conversation) -> Self {
        Self {
            conversation_uid: conversation.conversation_uid,
            created_at: format_timestamp(conversation.created_at),
            messages: conversation.messages.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Message> for MessageContext {
    fn from(message: Message) -> Self {
        let role = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        Self {
            role,
            created_at: format_timestamp(message.created_at),
            html: render_markdown(&message.content),
        }
    }
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.format("%a %d %b %Y, %H:%M UTC").to_string()
}

/// Renders markdown to HTML, treating any raw HTML in the source as plain text.
fn render_markdown(content: &str) -> String {
    let events = Parser::new(content).map(|event| match event {
        Event::Html(html) | Event::InlineHtml(html) => Event::Text(html),
        other => other,
    });

    let mut rendered = String::new();
    html::push_html(&mut rendered, events);

    rendered
}

#[cfg(test)]
mod tests {
    use crate::templates::render_markdown;

    #[test]
    fn renders_markdown() {
        let rendered = render_markdown("# Title\n\n- one\n- two");

        assert!(rendered.contains("<h1>Title</h1>"));
        assert!(rendered.contains("<li>one</li>"));
    }

    #[test]
    fn raw_html_is_escaped() {
        let rendered = render_markdown("hello <script>alert(1)</script>\n\n<div>block</div>");

        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains("<div>"));
        assert!(rendered.contains("&lt;script&gt;"));
    }
}
