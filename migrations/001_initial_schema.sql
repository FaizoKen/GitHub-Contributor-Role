CREATE TABLE IF NOT EXISTS role_links (
    id          BIGSERIAL PRIMARY KEY,
    guild_id    TEXT NOT NULL,
    role_id     TEXT NOT NULL,
    api_token   TEXT NOT NULL,
    conditions  JSONB NOT NULL DEFAULT '[]',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (guild_id, role_id)
);

CREATE TABLE IF NOT EXISTS linked_accounts (
    id              BIGSERIAL PRIMARY KEY,
    discord_id      TEXT NOT NULL UNIQUE,
    github_username TEXT NOT NULL UNIQUE,
    github_id       BIGINT NOT NULL UNIQUE,
    discord_name    TEXT,
    linked_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS role_assignments (
    guild_id    TEXT NOT NULL,
    role_id     TEXT NOT NULL,
    discord_id  TEXT NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (guild_id, role_id, discord_id),
    FOREIGN KEY (guild_id, role_id) REFERENCES role_links (guild_id, role_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS oauth_states (
    state       TEXT PRIMARY KEY,
    redirect_data JSONB,
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
