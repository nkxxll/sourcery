CREATE TABLE IF NOT EXISTS diffs (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_id      UUID NOT NULL UNIQUE REFERENCES versions(id) ON DELETE CASCADE,
    commit_range    TEXT NOT NULL,
    files_changed   INTEGER NOT NULL,
    insertions      INTEGER NOT NULL,
    deletions       INTEGER NOT NULL,
    changed_lines   INTEGER NOT NULL,
    summary         TEXT,
    metrics         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_diffs_version_id ON diffs(version_id);

INSERT INTO diffs (version_id, commit_range, files_changed, insertions, deletions, changed_lines, summary, metrics)
SELECT
    v.id,
    COALESCE(v.diff, ''),
    COALESCE((v.metrics ->> 'files_changed')::INTEGER, 0),
    COALESCE((v.metrics ->> 'insertions')::INTEGER, 0),
    COALESCE((v.metrics ->> 'deletions')::INTEGER, 0),
    COALESCE((v.metrics ->> 'total_line_changes')::INTEGER, 0),
    v.diff,
    COALESCE(v.metrics, '{}'::JSONB)
FROM versions v
WHERE NOT EXISTS (
    SELECT 1 FROM diffs d WHERE d.version_id = v.id
);

ALTER TABLE changes ADD COLUMN IF NOT EXISTS diff_id UUID REFERENCES diffs(id) ON DELETE CASCADE;

UPDATE changes c
SET diff_id = d.id
FROM diffs d
WHERE c.diff_id IS NULL
  AND c.version_id = d.version_id;

ALTER TABLE changes ALTER COLUMN diff_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_changes_diff_id ON changes(diff_id);

DROP INDEX IF EXISTS idx_changes_version_id;
ALTER TABLE changes DROP COLUMN IF EXISTS version_id;
