-- Restructures the entire database essentially
CREATE TABLE email_address (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	email_address_uid UUID NOT NULL,
	email_address TEXT NOT NULL,
	created_at TIMESTAMPTZ NOT NULL,
	verified_at TIMESTAMPTZ,
	active BOOLEAN NOT NULL
);

-- Make sure 2 users cannot have the same email address
CREATE UNIQUE INDEX idx_email_address_email_address ON email_address (lower(email_address));
-- Create an index on the UUID column
CREATE UNIQUE INDEX idx_email_address_email_address_uid ON email_address (email_address_uid);

CREATE TABLE account (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	account_uid UUID NOT NULL,
	email_address_id BIGINT NOT NULL,
	password TEXT NOT NULL,
	created_at TIMESTAMPTZ NOT NULL,

	CONSTRAINT fk_email_address_id FOREIGN KEY (email_address_id) REFERENCES email_address (id)
);

-- Create an index on the UUID column
CREATE UNIQUE INDEX idx_account_account_uid ON account (account_uid);

CREATE TABLE bodyweight (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	account_id BIGINT NOT NULL,
	bodyweight REAL NOT NULL,
	recorded DATE NOT NULL,

	CONSTRAINT fk_account_id FOREIGN KEY (account_id) REFERENCES account (id),
	-- Only one bodyweight entry per day
	CONSTRAINT uk_bodyweight_account_id_recorded UNIQUE (account_id, recorded)
);

CREATE TABLE workout (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	workout_uid UUID NOT NULL,
	account_id BIGINT NOT NULL,
	recorded DATE NOT NULL,

	CONSTRAINT fk_account_id FOREIGN KEY (account_id) REFERENCES account (id),
	-- Only one workout entry per day
	CONSTRAINT uk_workout_account_id_recorded UNIQUE (account_id, recorded)
);

-- Create an index on the UUID column
CREATE UNIQUE INDEX idx_workout_workout_uid ON workout (workout_uid);

CREATE TABLE user_preference (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	account_id BIGINT NOT NULL,
	rep_set_notation TEXT NOT NULL,

	CONSTRAINT fk_account_id FOREIGN KEY (account_id) REFERENCES account (id),
	-- Only one entry per user
	CONSTRAINT uk_user_preference_account_id UNIQUE (account_id)
);

-- Rename the existing tables for exercises
ALTER TABLE structured_exercise RENAME TO structured_exercise_old;
ALTER TABLE freeform_exercise RENAME TO freeform_exercise_old;

-- Create new versions
CREATE TABLE structured_exercise (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	workout_id BIGINT NOT NULL,
	variant exercise_variant NOT NULL,
	description TEXT NOT NULL,
	weight REAL NOT NULL,
	reps INTEGER NOT NULL,
	sets INTEGER NOT NULL,
	rpe REAL,

	CONSTRAINT fk_workout_id FOREIGN KEY (workout_id) REFERENCES workout (id)
);

CREATE TABLE freeform_exercise (
	id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	workout_id BIGINT NOT NULL,
	variant exercise_variant NOT NULL,
	description TEXT NOT NULL,
	weight REAL NOT NULL,
	reps INTEGER NOT NULL,
	sets INTEGER NOT NULL,
	rpe REAL,

	CONSTRAINT fk_workout_id FOREIGN KEY (workout_id) REFERENCES workout (id)
);

-- Move all the data into the new tables
INSERT INTO email_address (email_address_uid, email_address, created_at, active)
SELECT gen_random_uuid(), users.email, users.created_at, true
FROM users;

INSERT INTO account (account_uid, email_address_id, password, created_at)
SELECT users.id, (SELECT id FROM email_address WHERE email_address = users.email), users.password, users.created_at
FROM users;

INSERT INTO bodyweight (account_id, bodyweight, recorded)
SELECT (SELECT id FROM account WHERE account_uid = user_id), bodyweight, recorded
FROM bodyweights;

INSERT INTO workout (workout_uid, account_id, recorded)
SELECT workouts.id, (SELECT id FROM account WHERE account_uid = user_id), recorded
FROM workouts;

INSERT INTO user_preference (account_id, rep_set_notation)
SELECT (SELECT id FROM account WHERE account_uid = user_uid), rep_set_notation
FROM preferences;

INSERT INTO structured_exercise (workout_id, variant, description, weight, reps, sets, rpe)
SELECT (SELECT id FROM workout WHERE workout_uid = workout_id), variant, description, weight, reps, sets, rpe
FROM structured_exercise_old;

INSERT INTO freeform_exercise (workout_id, variant, description, weight, reps, sets, rpe)
SELECT (SELECT id FROM workout WHERE workout_uid = workout_id), variant, description, weight, reps, sets, rpe
FROM freeform_exercise_old;

-- Drop the old tables
DROP TABLE bodyweights;
DROP TABLE structured_exercise_old;
DROP TABLE freeform_exercise_old;
DROP TABLE preferences;
DROP TABLE workouts;
DROP TABLE users;
DROP TABLE exercises;
