ALTER TABLE diffs ADD COLUMN IF NOT EXISTS old_commit_hash TEXT;
ALTER TABLE diffs ADD COLUMN IF NOT EXISTS new_commit_hash TEXT;

UPDATE diffs d
SET new_commit_hash = v.commit_hash
FROM versions v
WHERE d.version_id = v.id
  AND (d.new_commit_hash IS NULL OR d.new_commit_hash = '');

WITH ordered_versions AS (
    SELECT
        v.id AS version_id,
        LAG(v.commit_hash) OVER (PARTITION BY v.codebase_id ORDER BY v.created_at, v.id) AS previous_commit_hash
    FROM versions v
)
UPDATE diffs d
SET old_commit_hash = COALESCE(
    CASE
        WHEN d.commit_range ~ '^(root|[0-9a-f]{7,40})\\.\\.[0-9a-f]{7,40}$'
            THEN NULLIF(split_part(d.commit_range, '..', 1), 'root')
        ELSE NULL
    END,
    ov.previous_commit_hash
)
FROM ordered_versions ov
WHERE d.version_id = ov.version_id
  AND d.old_commit_hash IS NULL;

ALTER TABLE diffs ALTER COLUMN new_commit_hash SET NOT NULL;
ALTER TABLE diffs DROP COLUMN IF EXISTS commit_range;
