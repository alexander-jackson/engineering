CREATE TABLE item_content (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  item_id BIGINT NOT NULL,
  content TEXT NOT NULL,
  recorded_at TIMESTAMP NOT NULL,

  CONSTRAINT pk_item_content PRIMARY KEY (id),
  CONSTRAINT fk_item_content_item FOREIGN KEY (item_id) REFERENCES item (id)
);

CREATE INDEX idx_item_content_item_id_recorded_at ON item_content (item_id, recorded_at DESC, id DESC);

-- Keep the original text of every existing item as its first version
INSERT INTO item_content (item_id, content, recorded_at)
SELECT id, content, created_at
FROM item;

ALTER TABLE item
DROP COLUMN content;
