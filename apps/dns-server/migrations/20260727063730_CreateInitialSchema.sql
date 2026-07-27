CREATE TABLE domain_event_type (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	name TEXT NOT NULL,

	CONSTRAINT pk_domain_event_type PRIMARY KEY (id),
	CONSTRAINT uk_domain_event_type_name UNIQUE (name)
);

INSERT INTO domain_event_type (name) VALUES ('Blocked');

CREATE TABLE domain (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_uid UUID NOT NULL,
	name TEXT NOT NULL,

	CONSTRAINT pk_domain PRIMARY KEY (id),
	CONSTRAINT uk_domain_domain_uid UNIQUE (domain_uid),
	CONSTRAINT uk_domain_name UNIQUE (name),
	CONSTRAINT ck_domain_name_lowercase CHECK (name = LOWER(name))
);

CREATE TABLE domain_event (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_event_uid UUID NOT NULL,
	domain_id BIGINT NOT NULL,
	event_type_id BIGINT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_domain_event PRIMARY KEY (id),
	CONSTRAINT uk_domain_event_domain_event_uid UNIQUE (domain_event_uid),
	CONSTRAINT fk_domain_event_domain FOREIGN KEY (domain_id) REFERENCES domain (id),
	CONSTRAINT fk_domain_event_domain_event_type FOREIGN KEY (event_type_id) REFERENCES domain_event_type (id)
);
