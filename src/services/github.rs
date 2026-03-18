use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{Quota, RateLimiter};

use crate::error::GitHubError;

#[derive(Debug, Clone)]
pub struct ContributorStats {
    pub commits: i32,
    pub pull_requests: i32,
    pub merged_prs: i32,
    pub issues: i32,
}

pub struct RepoData {
    pub contributors: HashMap<String, ContributorStats>,
}

pub struct GitHubClient {
    http: reqwest::Client,
    rate_limiter: Arc<
        RateLimiter<
            governor::state::NotKeyed,
            governor::state::InMemoryState,
            governor::clock::DefaultClock,
        >,
    >,
}

impl GitHubClient {
    pub fn new(token: &str) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("GitHubContributorRoles/1.0")
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                        .expect("Invalid token"),
                );
                headers.insert(
                    reqwest::header::ACCEPT,
                    reqwest::header::HeaderValue::from_static("application/vnd.github+json"),
                );
                headers
            })
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");

        // 1 request per second (conservative, leaves headroom under 5000/hr)
        let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Self { http, rate_limiter }
    }

    pub async fn wait_for_permit(&self) {
        self.rate_limiter.until_ready().await;
    }

    /// Fetch all contributor data for a repository.
    /// Makes ~15-25 API calls per repo (contributors + PRs + issues).
    pub async fn fetch_repo_data(&self, repo_full_name: &str) -> Result<RepoData, GitHubError> {
        let mut contributors: HashMap<String, ContributorStats> = HashMap::new();

        // 1. Fetch contributors (commit counts) — max 5 pages (500 contributors)
        self.fetch_contributors(repo_full_name, &mut contributors)
            .await?;

        // 2. Fetch PRs — max 10 pages (1000 PRs), aggregate by author
        self.fetch_pull_requests(repo_full_name, &mut contributors)
            .await?;

        // 3. Fetch issues — max 10 pages (1000 issues), aggregate by author
        self.fetch_issues(repo_full_name, &mut contributors).await?;

        Ok(RepoData { contributors })
    }

    async fn fetch_contributors(
        &self,
        repo_full_name: &str,
        contributors: &mut HashMap<String, ContributorStats>,
    ) -> Result<(), GitHubError> {
        let mut url = format!(
            "https://api.github.com/repos/{repo_full_name}/contributors?per_page=100"
        );

        for _ in 0..5 {
            self.wait_for_permit().await;
            let resp = self.http.get(&url).send().await?;

            let status = resp.status().as_u16();
            let next_url = parse_next_link(resp.headers());

            match status {
                200 => {}
                204 => return Ok(()), // empty repo
                404 => return Err(GitHubError::NotFound),
                403 => {
                    let body = resp.text().await.unwrap_or_default();
                    if body.contains("too large") {
                        return Err(GitHubError::RepoTooLarge);
                    }
                    return Err(check_rate_limit_from_body(&body));
                }
                429 => return Err(GitHubError::RateLimited),
                code => return Err(GitHubError::Server(code as u16)),
            }

            let items: Vec<serde_json::Value> = resp.json().await?;
            for item in &items {
                if let (Some(login), Some(commits)) = (
                    item["login"].as_str(),
                    item["contributions"].as_i64(),
                ) {
                    let entry = contributors
                        .entry(login.to_lowercase())
                        .or_insert_with(|| ContributorStats {
                            commits: 0,
                            pull_requests: 0,
                            merged_prs: 0,
                            issues: 0,
                        });
                    entry.commits = commits as i32;
                }
            }

            match next_url {
                Some(next) => url = next,
                None => break,
            }
        }

        Ok(())
    }

    async fn fetch_pull_requests(
        &self,
        repo_full_name: &str,
        contributors: &mut HashMap<String, ContributorStats>,
    ) -> Result<(), GitHubError> {
        let mut url = format!(
            "https://api.github.com/repos/{repo_full_name}/pulls?state=all&per_page=100&sort=created&direction=desc"
        );

        for _ in 0..10 {
            self.wait_for_permit().await;
            let resp = self.http.get(&url).send().await?;

            let status = resp.status().as_u16();
            if status != 200 {
                // Non-fatal: PRs are supplementary data
                tracing::warn!(repo_full_name, status, "Failed to fetch PRs, skipping");
                return Ok(());
            }

            let next_url = parse_next_link(resp.headers());
            let items: Vec<serde_json::Value> = resp.json().await?;

            for item in &items {
                if let Some(login) = item["user"]["login"].as_str() {
                    let entry = contributors
                        .entry(login.to_lowercase())
                        .or_insert_with(|| ContributorStats {
                            commits: 0,
                            pull_requests: 0,
                            merged_prs: 0,
                            issues: 0,
                        });
                    entry.pull_requests += 1;
                    if item["merged_at"].is_string() {
                        entry.merged_prs += 1;
                    }
                }
            }

            match next_url {
                Some(next) => url = next,
                None => break,
            }
        }

        Ok(())
    }

    async fn fetch_issues(
        &self,
        repo_full_name: &str,
        contributors: &mut HashMap<String, ContributorStats>,
    ) -> Result<(), GitHubError> {
        let mut url = format!(
            "https://api.github.com/repos/{repo_full_name}/issues?state=all&per_page=100&sort=created&direction=desc"
        );

        for _ in 0..10 {
            self.wait_for_permit().await;
            let resp = self.http.get(&url).send().await?;

            let status = resp.status().as_u16();
            if status != 200 {
                tracing::warn!(repo_full_name, status, "Failed to fetch issues, skipping");
                return Ok(());
            }

            let next_url = parse_next_link(resp.headers());
            let items: Vec<serde_json::Value> = resp.json().await?;

            for item in &items {
                // GitHub includes PRs in the issues endpoint — skip them
                if item.get("pull_request").is_some() {
                    continue;
                }

                if let Some(login) = item["user"]["login"].as_str() {
                    let entry = contributors
                        .entry(login.to_lowercase())
                        .or_insert_with(|| ContributorStats {
                            commits: 0,
                            pull_requests: 0,
                            merged_prs: 0,
                            issues: 0,
                        });
                    entry.issues += 1;
                }
            }

            match next_url {
                Some(next) => url = next,
                None => break,
            }
        }

        Ok(())
    }
}

/// Parse the `Link` header for the `rel="next"` URL.
fn parse_next_link(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let link = headers.get("link")?.to_str().ok()?;
    for part in link.split(',') {
        let part = part.trim();
        if part.contains("rel=\"next\"") {
            // Extract URL between < and >
            let start = part.find('<')? + 1;
            let end = part.find('>')?;
            return Some(part[start..end].to_string());
        }
    }
    None
}

fn check_rate_limit_from_body(body: &str) -> GitHubError {
    if body.contains("rate limit") || body.contains("API rate") {
        GitHubError::RateLimited
    } else {
        GitHubError::Forbidden
    }
}
