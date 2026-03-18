CREATE INDEX IF NOT EXISTS idx_repo_cache_next_fetch ON repo_cache (next_fetch_at ASC);
CREATE INDEX IF NOT EXISTS idx_repo_contributors_user ON repo_contributors (github_username);
CREATE INDEX IF NOT EXISTS idx_linked_accounts_github ON linked_accounts (github_username);
CREATE INDEX IF NOT EXISTS idx_role_assignments_discord ON role_assignments (discord_id);
CREATE INDEX IF NOT EXISTS idx_role_assignments_guild_role ON role_assignments (guild_id, role_id);
CREATE INDEX IF NOT EXISTS idx_user_guilds_guild ON user_guilds (guild_id);
