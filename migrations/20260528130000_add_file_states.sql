CREATE TABLE IF NOT EXISTS file_states (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    codebase_id UUID NOT NULL REFERENCES codebases(id) ON DELETE CASCADE,
    version_id  UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    file_id     UUID REFERENCES files(id) ON DELETE SET NULL,
    status      TEXT NOT NULL,
    exists      BOOLEAN NOT NULL,
    source_path TEXT,
    metrics     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (version_id, path)
);

CREATE INDEX IF NOT EXISTS idx_file_states_codebase_id ON file_states(codebase_id);
CREATE INDEX IF NOT EXISTS idx_file_states_version_id ON file_states(version_id);
CREATE INDEX IF NOT EXISTS idx_file_states_path ON file_states(path);
CREATE INDEX IF NOT EXISTS idx_file_states_file_id ON file_states(file_id);

INSERT INTO file_states (codebase_id, version_id, path, file_id, status, exists, metrics)
SELECT v.codebase_id, f.version_id, f.path, f.id, 'analyzed', TRUE, f.metrics
FROM files f
JOIN versions v ON v.id = f.version_id
ON CONFLICT (version_id, path) DO NOTHING;

INSERT INTO file_states (codebase_id, version_id, path, file_id, status, exists, metrics)
SELECT v.codebase_id, d.version_id, fc.old_path, NULL, fc.status, FALSE, '{}'::JSONB
FROM file_changes fc
JOIN diffs d ON d.id = fc.diff_id
JOIN versions v ON v.id = d.version_id
WHERE fc.old_path IS NOT NULL
  AND (
      fc.status IN ('deleted', 'renamed')
      OR fc.new_path IS NULL
      OR fc.new_path <> fc.old_path
  )
ON CONFLICT (version_id, path) DO UPDATE SET
    file_id = EXCLUDED.file_id,
    status = EXCLUDED.status,
    exists = EXCLUDED.exists,
    metrics = EXCLUDED.metrics;
