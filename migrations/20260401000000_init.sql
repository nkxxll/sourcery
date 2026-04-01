CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- A git repository being analyzed
CREATE TABLE IF NOT EXISTS repositories (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    url         TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A single commit in a repository
CREATE TABLE IF NOT EXISTS commits (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repository_id   UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    oid             TEXT NOT NULL,              -- git SHA hex
    message         TEXT NOT NULL DEFAULT '',
    author_name     TEXT NOT NULL DEFAULT '',
    author_email    TEXT NOT NULL DEFAULT '',
    committed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (repository_id, oid)
);

CREATE INDEX IF NOT EXISTS idx_commits_repository_id ON commits(repository_id);

-- Per-file metrics collected at each commit
CREATE TABLE IF NOT EXISTS file_metrics (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    commit_id   UUID NOT NULL REFERENCES commits(id) ON DELETE CASCADE,
    file_path   TEXT NOT NULL,
    lines       INTEGER NOT NULL DEFAULT 0,
    size_bytes  BIGINT  NOT NULL DEFAULT 0,
    language    TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_file_metrics_commit_id ON file_metrics(commit_id);
CREATE INDEX IF NOT EXISTS idx_file_metrics_file_path ON file_metrics(file_path);
