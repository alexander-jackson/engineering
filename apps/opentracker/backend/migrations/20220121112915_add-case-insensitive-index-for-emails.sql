CREATE UNIQUE INDEX users_email_idx_unique
ON users(lower(email));
