-- Questions the decision model is asked about each new domain. The table is append only so that a
-- question can always be referred back to, changing one means adding a new question and ending
-- the old one.
CREATE TABLE classifier_question (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	classifier_question_uid UUID NOT NULL,
	prompt TEXT NOT NULL,
	threshold DOUBLE PRECISION NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_classifier_question PRIMARY KEY (id),
	CONSTRAINT uk_classifier_question_classifier_question_uid UNIQUE (classifier_question_uid),
	CONSTRAINT ck_classifier_question_prompt_not_blank CHECK (BTRIM(prompt) <> ''),
	CONSTRAINT ck_classifier_question_threshold_range CHECK (threshold >= 0 AND threshold <= 1)
);

CREATE TABLE classifier_question_event_type (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	name TEXT NOT NULL,

	CONSTRAINT pk_classifier_question_event_type PRIMARY KEY (id),
	CONSTRAINT uk_classifier_question_event_type_name UNIQUE (name)
);

-- Retired questions were removed by a user, redundant ones were superseded by a newer question.
INSERT INTO classifier_question_event_type (name) VALUES ('Retired'), ('Redundant');

-- A question with no events is active. Events are terminal, so a question has at most one.
CREATE TABLE classifier_question_event (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	classifier_question_event_uid UUID NOT NULL,
	classifier_question_id BIGINT NOT NULL,
	event_type_id BIGINT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_classifier_question_event PRIMARY KEY (id),
	CONSTRAINT uk_classifier_question_event_classifier_question_event_uid UNIQUE (classifier_question_event_uid),
	CONSTRAINT uk_classifier_question_event_classifier_question_id UNIQUE (classifier_question_id),
	CONSTRAINT fk_classifier_question_event_classifier_question FOREIGN KEY (classifier_question_id) REFERENCES classifier_question (id),
	CONSTRAINT fk_classifier_question_event_classifier_question_event_type FOREIGN KEY (event_type_id) REFERENCES classifier_question_event_type (id)
);

CREATE FUNCTION prevent_modification() RETURNS TRIGGER AS $$
BEGIN
	RAISE EXCEPTION '% on % is not allowed, the table is append only', TG_OP, TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_classifier_question_append_only
	BEFORE UPDATE OR DELETE ON classifier_question
	FOR EACH ROW EXECUTE FUNCTION prevent_modification();

CREATE TRIGGER trg_classifier_question_event_append_only
	BEFORE UPDATE OR DELETE ON classifier_question_event
	FOR EACH ROW EXECUTE FUNCTION prevent_modification();

-- The outcome of classifying a domain, which is only ever done once per domain.
CREATE TABLE domain_decision (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_decision_uid UUID NOT NULL,
	name TEXT NOT NULL,
	blocked BOOLEAN NOT NULL,
	decided_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_domain_decision PRIMARY KEY (id),
	CONSTRAINT uk_domain_decision_domain_decision_uid UNIQUE (domain_decision_uid),
	CONSTRAINT uk_domain_decision_name UNIQUE (name),
	CONSTRAINT ck_domain_decision_name_lowercase CHECK (name = LOWER(name))
);

-- The score each question gave to a domain, so a decision can be audited. Decisions made without
-- asking any questions (e.g. there was no content to classify) have no rows here.
CREATE TABLE domain_decision_question (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_decision_id BIGINT NOT NULL,
	classifier_question_id BIGINT NOT NULL,
	score DOUBLE PRECISION NOT NULL,

	CONSTRAINT pk_domain_decision_question PRIMARY KEY (id),
	CONSTRAINT uk_domain_decision_question_decision_question UNIQUE (domain_decision_id, classifier_question_id),
	CONSTRAINT fk_domain_decision_question_domain_decision FOREIGN KEY (domain_decision_id) REFERENCES domain_decision (id) ON DELETE CASCADE,
	CONSTRAINT fk_domain_decision_question_classifier_question FOREIGN KEY (classifier_question_id) REFERENCES classifier_question (id),
	CONSTRAINT ck_domain_decision_question_score_range CHECK (score >= 0 AND score <= 1)
);
