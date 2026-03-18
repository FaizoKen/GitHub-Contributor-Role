CREATE TABLE IF NOT EXISTS user_guilds (
    discord_id  TEXT NOT NULL,
    guild_id    TEXT NOT NULL,
    guild_name  TEXT,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (discord_id, guild_id)
);
