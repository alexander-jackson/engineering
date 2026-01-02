CREATE TYPE rep_set_notation AS ENUM (
	'SetsThenReps',
	'RepsThenSets'
);

CREATE TABLE preferences (
	user_uid UUID UNIQUE NOT NULL REFERENCES users(id),
	rep_set_notation TEXT NOT NULL
);
