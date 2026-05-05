ALTER TABLE IF EXISTS files DROP CONSTRAINT IF EXISTS files_directory_id_fkey;
DROP INDEX IF EXISTS idx_files_directory_id;
ALTER TABLE IF EXISTS files DROP COLUMN IF EXISTS directory_id;

DROP TABLE IF EXISTS directories;

CREATE TABLE IF NOT EXISTS changes (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_id      UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    old_path        TEXT,
    new_path        TEXT,
    old_start_line  INTEGER NOT NULL,
    old_end_line    INTEGER NOT NULL,
    new_start_line  INTEGER NOT NULL,
    new_end_line    INTEGER NOT NULL,
    metrics         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_changes_version_id ON changes(version_id);
