-- Create the initial versions of each table
CREATE TABLE users (
	id UUID PRIMARY KEY,
	email TEXT NOT NULL,
	password TEXT NOT NULL
);

CREATE TABLE bodyweights (
	user_id UUID NOT NULL,
	bodyweight REAL NOT NULL,
	recorded DATE NOT NULL,
	PRIMARY KEY (user_id, recorded)
);

CREATE TABLE workouts (
	id UUID NOT NULL,
	user_id UUID NOT NULL,
	recorded DATE NOT NULL,
	PRIMARY KEY (user_id, recorded)
);

CREATE TABLE exercises (
	workout_id UUID NOT NULL,
	name TEXT NOT NULL,
	weight REAL NOT NULL,
	reps INTEGER NOT NULL
);
