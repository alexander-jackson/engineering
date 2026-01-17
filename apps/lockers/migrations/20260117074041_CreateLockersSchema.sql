-- Bag type lookup table
CREATE TABLE bag_type (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  name TEXT NOT NULL,
  display_name TEXT NOT NULL,

  CONSTRAINT pk_bag_type PRIMARY KEY (id),
  CONSTRAINT uk_bag_type_name UNIQUE (name)
);

-- Seed bag types
INSERT INTO bag_type (name, display_name) VALUES
  ('PeakDesign30L', 'Peak Design 30L'),
  ('StubbleAndCo20L', 'Stubble & Co 20L');

-- Event type lookup table
CREATE TABLE locker_event_type (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  name TEXT NOT NULL,

  CONSTRAINT pk_locker_event_type PRIMARY KEY (id),
  CONSTRAINT uk_locker_event_type_name UNIQUE (name)
);

-- Seed event types
INSERT INTO locker_event_type (name) VALUES
  ('CheckIn'),
  ('CheckOut');

-- Immutable event log for check-ins and check-outs
CREATE TABLE locker_event (
  id BIGINT GENERATED ALWAYS AS IDENTITY,
  locker_event_uid UUID NOT NULL,
  locker_number SMALLINT NOT NULL,
  bag_type_id BIGINT NOT NULL,
  locker_event_type_id BIGINT NOT NULL,
  occurred_at TIMESTAMP NOT NULL,

  CONSTRAINT pk_locker_event PRIMARY KEY (id),
  CONSTRAINT uk_locker_event_uid UNIQUE (locker_event_uid),
  CONSTRAINT fk_locker_event_bag_type FOREIGN KEY (bag_type_id) REFERENCES bag_type (id),
  CONSTRAINT fk_locker_event_type FOREIGN KEY (locker_event_type_id) REFERENCES locker_event_type (id),
  CONSTRAINT check_locker_number_range CHECK (locker_number >= 0 AND locker_number <= 999)
);

-- Indexes for efficient queries of current state
CREATE INDEX idx_locker_event_locker_number_occurred_at ON locker_event (locker_number, occurred_at DESC);
CREATE INDEX idx_locker_event_bag_type_occurred_at ON locker_event (bag_type_id, occurred_at DESC);
