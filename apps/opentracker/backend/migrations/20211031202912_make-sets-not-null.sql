-- Add migration script here
ALTER TABLE exercises
ALTER COLUMN sets SET NOT NULL;

ALTER TABLE exercises
ALTER COLUMN sets DROP DEFAULT;
