CREATE TABLE retainer_event_type (
    id   BIGINT GENERATED ALWAYS AS IDENTITY,
    name TEXT NOT NULL,

    CONSTRAINT pk_retainer_event_type PRIMARY KEY (id),
    CONSTRAINT uk_retainer_event_type_name UNIQUE (name)
);

INSERT INTO retainer_event_type (name) VALUES ('Inserted'), ('Removed');

CREATE TABLE retainer_event (
    id            BIGINT GENERATED ALWAYS AS IDENTITY,
    event_type_id BIGINT NOT NULL,
    occurred_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT pk_retainer_event PRIMARY KEY (id),
    CONSTRAINT fk_retainer_event_event_type_id FOREIGN KEY (event_type_id) REFERENCES retainer_event_type (id)
);

CREATE INDEX ON retainer_event (occurred_at DESC);
