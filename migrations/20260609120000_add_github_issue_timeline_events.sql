CREATE TABLE IF NOT EXISTS github_issue_timeline_events (
    id                  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo                TEXT NOT NULL,
    issue_number        INTEGER NOT NULL,
    issue_title         TEXT NOT NULL DEFAULT '',
    issue_state         TEXT NOT NULL DEFAULT '',
    event               TEXT NOT NULL,
    event_created_at    TIMESTAMPTZ NOT NULL,
    actor               TEXT NOT NULL DEFAULT '',
    commit_id           TEXT NOT NULL DEFAULT '',
    label               TEXT NOT NULL DEFAULT '',
    assignee            TEXT NOT NULL DEFAULT '',
    milestone           TEXT NOT NULL DEFAULT '',
    source              TEXT NOT NULL DEFAULT '',
    body                TEXT NOT NULL DEFAULT '',
    html_url            TEXT NOT NULL DEFAULT '',
    raw_event           JSONB NOT NULL DEFAULT '{}',
    imported_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (repo, issue_number, event, event_created_at, actor, html_url)
);

CREATE INDEX IF NOT EXISTS idx_github_issue_timeline_events_repo_created
    ON github_issue_timeline_events(repo, event_created_at);

CREATE INDEX IF NOT EXISTS idx_github_issue_timeline_events_issue
    ON github_issue_timeline_events(repo, issue_number, event_created_at);

CREATE INDEX IF NOT EXISTS idx_github_issue_timeline_events_event
    ON github_issue_timeline_events(repo, event, event_created_at);
