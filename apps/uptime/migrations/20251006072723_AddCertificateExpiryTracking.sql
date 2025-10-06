CREATE TABLE certificate_check (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	certificate_check_uid UUID NOT NULL,
	origin_id BIGINT NOT NULL,
	expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
	checked_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_certificate_check PRIMARY KEY (id),
	CONSTRAINT uk_certificate_check_certificate_check_uid UNIQUE (certificate_check_uid),
	CONSTRAINT fk_certificate_check_origin_id FOREIGN KEY (origin_id) REFERENCES origin (id)
);

CREATE INDEX idx_certificate_check_origin_checked_at ON certificate_check (origin_id, checked_at DESC);
