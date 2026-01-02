-- Add the column
ALTER TABLE users
ADD COLUMN created_at timestamp with time zone;

-- Fill in the old values
UPDATE users
SET created_at = now() :: timestamp
WHERE created_at IS NULL;

-- Mark it as not-null
ALTER TABLE users
ALTER COLUMN created_at SET NOT NULL;
