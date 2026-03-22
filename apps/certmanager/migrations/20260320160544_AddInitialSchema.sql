CREATE TABLE domain (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_uid UUID NOT NULL,
	name TEXT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_domain PRIMARY KEY (id),
	CONSTRAINT uk_domain_domain_uid UNIQUE (domain_uid),
	CONSTRAINT uk_domain_name UNIQUE (name)
);

CREATE TABLE certificate (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	certificate_uid UUID NOT NULL,
	domain_id BIGINT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,
	expires_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_certificate PRIMARY KEY (id),
	CONSTRAINT uk_certificate_certificate_uid UNIQUE (certificate_uid),
	CONSTRAINT fk_certificate_domain FOREIGN KEY (domain_id) REFERENCES domain (id)
);

-- Add an index to make it fast to find expiring certificates
CREATE INDEX idx_certificate_expires_at ON certificate (expires_at);
