ALTER TABLE versions
ADD COLUMN IF NOT EXISTS sample_number INTEGER NOT NULL DEFAULT 0;

ALTER TABLE versions
ADD CONSTRAINT versions_sample_number_non_negative CHECK (sample_number >= 0);

CREATE INDEX IF NOT EXISTS idx_versions_codebase_sample_number
ON versions(codebase_id, sample_number);

CREATE UNIQUE INDEX IF NOT EXISTS idx_versions_codebase_positive_sample_number
ON versions(codebase_id, sample_number)
WHERE sample_number > 0;
