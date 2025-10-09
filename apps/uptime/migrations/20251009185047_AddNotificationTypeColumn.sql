CREATE TABLE notification_type (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	name TEXT NOT NULL,

	CONSTRAINT pk_notification_type PRIMARY KEY (id),
	CONSTRAINT uk_notification_type_name UNIQUE (name)
);

INSERT INTO notification_type (name) VALUES ('Uptime'), ('CertificateExpiry');

ALTER TABLE notification
	ADD COLUMN notification_type_id BIGINT;

-- Set all existing notifications to 'Uptime' type
UPDATE notification
SET notification_type_id = (SELECT id FROM notification_type WHERE name = 'Uptime');

-- Make the column NOT NULL now that it's populated
ALTER TABLE notification
	ALTER COLUMN notification_type_id SET NOT NULL,
	ADD CONSTRAINT fk_notification_notification_type_id FOREIGN KEY (notification_type_id) REFERENCES notification_type (id);
