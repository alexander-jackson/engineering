CREATE TABLE conversation (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  conversation_uid UUID NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,

  CONSTRAINT pk_conversation PRIMARY KEY (id),
  CONSTRAINT uk_conversation_conversation_uid UNIQUE (conversation_uid)
);

CREATE INDEX idx_conversation_created_at_desc ON conversation (created_at DESC);

CREATE TABLE conversation_message_role (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  name TEXT NOT NULL,

  CONSTRAINT pk_conversation_message_role PRIMARY KEY (id),
  CONSTRAINT uk_conversation_message_role_name UNIQUE (name)
);

INSERT INTO conversation_message_role (name) VALUES ('system'), ('user'), ('assistant');

CREATE TABLE conversation_message (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  conversation_id BIGINT NOT NULL,
  conversation_message_role_id BIGINT NOT NULL,
  content TEXT NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,

  CONSTRAINT pk_conversation_message PRIMARY KEY (id),
  CONSTRAINT fk_conversation_message_conversation_id
    FOREIGN KEY (conversation_id) REFERENCES conversation (id),
  CONSTRAINT fk_conversation_message_conversation_message_role_id
    FOREIGN KEY (conversation_message_role_id) REFERENCES conversation_message_role (id)
);

CREATE INDEX idx_conversation_message_conversation_id
  ON conversation_message (conversation_id, id);
