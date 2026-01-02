-- Adds the sets and RPE columns to each exercise
ALTER TABLE exercises
ADD COLUMN sets INTEGER DEFAULT 1,
ADD COLUMN rpe REAL;
