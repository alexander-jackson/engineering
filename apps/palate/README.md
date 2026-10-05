# Palate

`palate` sends a set of recipe ideas every week, to slowly explore different
cuisines and techniques.

Once a week (day and time come from the `schedule` configuration, in UTC) it:

1. starts a new conversation, stored in PostgreSQL (`conversation` and
   `conversation_message`), containing a system prompt and a request for ideas
2. sends the conversation through [OpenRouter][openrouter] to the configured
   model, mixing classics, forgotten dishes, new cuisines and new techniques,
   mostly suited to meal prep
3. stores the response and sends a Telegram message linking to the conversation

The web frontend lists past conversations, shows each one and lets you send
follow-up messages. The full history is replayed to the model on every follow-up,
and the page waits for the reply.

There is no application level authentication, access is restricted by mTLS in
`f2`, like the other web applications.

## Configuration

See `local-config.yaml`. The OpenRouter API key and Telegram bot token are
secrets, and `telegram.chat_id` should be quoted as channel identifiers are
often negative numbers.

## Development

```bash
# needs a local PostgreSQL, see `.env` for the connection used by sqlx
sqlx database create && sqlx migrate run

just run
```

Queries are checked at compile time, so run `cargo sqlx prepare` after changing
them and commit the `.sqlx` directory.

[openrouter]: https://openrouter.ai
