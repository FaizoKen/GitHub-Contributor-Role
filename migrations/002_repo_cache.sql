CREATE TABLE IF NOT EXISTS repo_cache (
    repo_full_name  TEXT PRIMARY KEY,
    fetched_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    next_fetch_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    fetch_failures  INTEGER NOT NULL DEFAULT 0,
    contributor_count INTEGER NOT NULL DEFAULT 0,
    pr_count        INTEGER NOT NULL DEFAULT 0,
    issue_count     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS repo_contributors (
    repo_full_name  TEXT NOT NULL,
    github_username TEXT NOT NULL,
    commits         INTEGER NOT NULL DEFAULT 0,
    pull_requests   INTEGER NOT NULL DEFAULT 0,
    merged_prs      INTEGER NOT NULL DEFAULT 0,
    issues          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (repo_full_name, github_username)
);
