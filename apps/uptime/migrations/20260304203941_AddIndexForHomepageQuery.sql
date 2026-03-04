CREATE INDEX idx_query_origin_id_queried_at_desc ON query (origin_id, queried_at DESC);
CREATE INDEX idx_query_failure_origin_id_queried_at_desc ON query_failure (origin_id, queried_at DESC);
