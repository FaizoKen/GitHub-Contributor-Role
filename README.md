# GitHub Contributor Role

Lightweight Rust backend that tracks [GitHub](https://github.com) contribution activity and syncs qualifying users to [RoleLogic](https://rolelogic.faizo.net) for automatic Discord role assignment. Designed as a RoleLogic plugin with multi-guild support.

## How it works

1. **Registers** guild/role pairs via the RoleLogic plugin API
2. **Links accounts** — users verify via Discord OAuth, then link their GitHub account via GitHub OAuth
3. **Fetches** contribution data per configured repository (commits, PRs, merged PRs, issues) using the GitHub API
4. **Evaluates** admin-configured conditions (e.g. "commits >= 10 on owner/repo")
5. **Syncs** qualifying users to RoleLogic's Role Link API automatically

RoleLogic then assigns/removes Discord roles based on the synced list.

## Condition fields

| Field | Description |
| ----- | ----------- |
| `commits` | Total commits to the configured repository |
| `pullRequests` | Total pull requests opened |
| `mergedPRs` | Total pull requests merged |
| `issues` | Total issues opened |

Operators: `>=`, `<=`, `=`

## Setup

```bash
cp .env.example .env
# Edit .env with your values
```

### Environment Variables

| Variable | Required | Default | Description |
| -------- | -------- | ------- | ----------- |
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `DISCORD_CLIENT_ID` | Yes | — | Discord OAuth app client ID |
| `DISCORD_CLIENT_SECRET` | Yes | — | Discord OAuth app client secret |
| `SESSION_SECRET` | Yes | — | Random string for HMAC session signing |
| `GITHUB_CLIENT_ID` | Yes | — | GitHub OAuth app client ID |
| `GITHUB_CLIENT_SECRET` | Yes | — | GitHub OAuth app client secret |
| `GITHUB_TOKEN` | Yes | — | GitHub PAT for server-side API calls (5,000 req/hr) |
| `BASE_URL` | Yes | — | Public URL (e.g. `https://github-roles.example.com`) |
| `LISTEN_ADDR` | No | `0.0.0.0:8080` | Bind address |
| `GITHUB_MAX_REQUESTS_PER_HOUR` | No | `4500` | Rate limit for GitHub API calls |
| `RUST_LOG` | No | `github_roles=info` | Log level |

### Credentials

- **Discord OAuth App**: [Discord Developer Portal](https://discord.com/developers/applications) — callback URL: `{BASE_URL}/verify/callback`
- **GitHub OAuth App**: [GitHub Developer Settings](https://github.com/settings/applications/new) — callback URL: `{BASE_URL}/verify/callback`
- **GitHub Token**: [Personal Access Token](https://github.com/settings/tokens/new) — classic token, no scopes needed (public repo read access)

## Run

### Docker (recommended)

```bash
docker compose up -d
```

### From source

```bash
cargo run              # development
cargo build --release  # production
```

## Endpoints

| Method | Path | Description |
| ------ | ---- | ----------- |
| `POST` | `/register` | Register a guild/role pair |
| `GET` | `/config` | Get plugin configuration schema |
| `POST` | `/config` | Update plugin configuration |
| `DELETE` | `/config` | Delete a registration |
| `GET` | `/verify` | Verification page (Discord + GitHub linking) |
| `GET` | `/verify/login` | Start Discord OAuth flow |
| `GET` | `/verify/callback` | OAuth callback handler |
| `GET` | `/verify/status` | Check link status |
| `POST` | `/verify/unlink` | Unlink GitHub account |
| `GET` | `/health` | Health check |

## Usage

1. In the RoleLogic dashboard, create a Role Link and set the **Custom Plugin URL** to your GitHub Contributor Role instance's public URL
2. RoleLogic will automatically register the guild/role pair
3. Open the plugin config in RoleLogic — configure the target repository (`owner/repo`) and conditions (e.g. "commits >= 5")
4. Share the verification link (`{BASE_URL}/verify`) with your server members
5. Members link their Discord + GitHub accounts, and roles are assigned automatically based on their contributions

## API Reference

- [RoleLogic Role Link API](https://docs-rolelogic.faizo.net/reference/role-link-api)
- [GitHub REST API — Repositories](https://docs.github.com/en/rest/repos)
- [GitHub REST API — Pull Requests](https://docs.github.com/en/rest/pulls)

## License

[MIT](LICENSE)
