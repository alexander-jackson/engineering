CREATE TABLE retainer_seating (
    id          BIGINT GENERATED ALWAYS AS IDENTITY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id)
);

CREATE INDEX idx_retainer_seating_occurred_at ON retainer_seating (occurred_at DESC);
