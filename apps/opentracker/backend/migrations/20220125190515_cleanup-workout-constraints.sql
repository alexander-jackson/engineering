-- Remove the existing primary key constraint
ALTER TABLE workouts
DROP CONSTRAINT workouts_pkey;

-- Create a new primary key constraint on the ID
ALTER TABLE workouts
ADD CONSTRAINT pk_workouts PRIMARY KEY (id);

-- Add a unique constraint on the combination of user and recorded
ALTER TABLE workouts
ADD CONSTRAINT uk_workouts UNIQUE (user_id, recorded);
