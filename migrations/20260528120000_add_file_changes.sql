CREATE TABLE IF NOT EXISTS file_changes (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    diff_id         UUID NOT NULL REFERENCES diffs(id) ON DELETE CASCADE,
    old_path        TEXT,
    new_path        TEXT,
    status          TEXT NOT NULL,
    old_blob_oid    TEXT,
    new_blob_oid    TEXT,
    metrics         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_file_changes_diff_id ON file_changes(diff_id);
CREATE INDEX IF NOT EXISTS idx_file_changes_old_path ON file_changes(old_path);
CREATE INDEX IF NOT EXISTS idx_file_changes_new_path ON file_changes(new_path);
CREATE INDEX IF NOT EXISTS idx_file_changes_status ON file_changes(status);
