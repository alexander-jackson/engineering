-- The content a decision was made from, so it can be audited or re-evaluated later. Decisions made
-- without fetching anything (e.g. the fetch failed) have no row here. The table is append only.
CREATE TABLE domain_decision_content (
	id BIGINT GENERATED ALWAYS AS IDENTITY,
	domain_decision_id BIGINT NOT NULL,
	-- The document exactly as received, compressed with zstd.
	raw_content BYTEA NOT NULL,
	-- The text extracted from the document, which is what the decision model was shown.
	trimmed_content TEXT NOT NULL,
	-- Whether the document was cut short by the download limit.
	truncated BOOLEAN NOT NULL,
	fetched_at TIMESTAMP WITH TIME ZONE NOT NULL,

	CONSTRAINT pk_domain_decision_content PRIMARY KEY (id),
	CONSTRAINT uk_domain_decision_content_domain_decision_id UNIQUE (domain_decision_id),
	CONSTRAINT fk_domain_decision_content_domain_decision FOREIGN KEY (domain_decision_id) REFERENCES domain_decision (id)
);

CREATE TRIGGER trg_domain_decision_content_append_only
	BEFORE UPDATE OR DELETE ON domain_decision_content
	FOR EACH ROW EXECUTE FUNCTION prevent_modification();
