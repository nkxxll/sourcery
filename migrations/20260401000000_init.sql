CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- A codebase being analyzed
CREATE TABLE IF NOT EXISTS codebases (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT NOT NULL UNIQUE,
    url         TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A specific version (commit) of a codebase
CREATE TABLE IF NOT EXISTS versions (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    codebase_id     UUID NOT NULL REFERENCES codebases(id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL,
    message         TEXT NOT NULL DEFAULT '',
    author_name     TEXT NOT NULL DEFAULT '',
    author_email    TEXT NOT NULL DEFAULT '',
    committed_at    TIMESTAMPTZ,
    diff            TEXT,
    metrics         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (codebase_id, commit_hash)
);

CREATE INDEX IF NOT EXISTS idx_versions_codebase_id ON versions(codebase_id);

-- A directory within a version (self-referential for nesting)
CREATE TABLE IF NOT EXISTS directories (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_id  UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    parent_id   UUID REFERENCES directories(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    metrics     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (version_id, path)
);

CREATE INDEX IF NOT EXISTS idx_directories_version_id ON directories(version_id);
CREATE INDEX IF NOT EXISTS idx_directories_path ON directories(path);

-- A file within a directory
CREATE TABLE IF NOT EXISTS files (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_id      UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    directory_id    UUID NOT NULL REFERENCES directories(id) ON DELETE CASCADE,
    path            TEXT NOT NULL,
    language        TEXT,
    metrics         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (version_id, path)
);

CREATE INDEX IF NOT EXISTS idx_files_version_id ON files(version_id);
CREATE INDEX IF NOT EXISTS idx_files_directory_id ON files(directory_id);
CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);

-- A function within a file
CREATE TABLE IF NOT EXISTS functions (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    file_id     UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    start_line  INTEGER NOT NULL,
    end_line    INTEGER NOT NULL,
    metrics     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_functions_file_id ON functions(file_id);
