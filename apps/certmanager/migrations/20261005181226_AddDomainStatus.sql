CREATE TABLE domain_status (
	id SMALLINT NOT NULL,
	name TEXT NOT NULL,
	CONSTRAINT pk_domain_status PRIMARY KEY (id),
	CONSTRAINT uk_domain_status_name UNIQUE (name)
);

INSERT INTO domain_status (id, name) VALUES (1, 'Active'), (2, 'Retired');

CREATE TABLE domain_status_change (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_status_change_uid UUID NOT NULL,
	domain_id BIGINT NOT NULL,
	domain_status_id SMALLINT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,
	CONSTRAINT pk_domain_status_change PRIMARY KEY (id),
	CONSTRAINT uk_domain_status_change_domain_status_change_uid UNIQUE (domain_status_change_uid),
	CONSTRAINT fk_domain_status_change_domain FOREIGN KEY (domain_id) REFERENCES domain (id),
	CONSTRAINT fk_domain_status_change_domain_status FOREIGN KEY (domain_status_id) REFERENCES domain_status (id)
);

CREATE INDEX idx_domain_status_change_domain_id_created_at ON domain_status_change (domain_id, created_at DESC);

INSERT INTO domain_status_change (domain_status_change_uid, domain_id, domain_status_id, created_at)
SELECT gen_random_uuid(), d.id, (SELECT id FROM domain_status WHERE name = 'Active'), d.created_at
FROM domain d;
