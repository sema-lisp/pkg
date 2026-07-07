use std::env;

/// The first non-empty value among `keys` in the environment, if any.
fn env_any(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| env::var(k).ok().filter(|v| !v.is_empty()))
}

/// The compiled-in placeholder for `OAUTH_TOKEN_KEY`. Deploying with this value
/// unchanged would encrypt every stored GitHub token under a publicly known
/// key, so [`Config::check_production_secrets`] refuses to boot when it is used
/// while GitHub OAuth is enabled.
pub const DEFAULT_OAUTH_TOKEN_KEY: &str = "change-me-32-bytes-in-production!";

pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub blob_dir: String,
    // S3-compatible blob storage (Cloudflare R2, MinIO, S3). When `blob_s3_bucket`
    // is set, tarballs go to object storage instead of `blob_dir` — decoupling
    // durability from the compute node for stateless / multi-instance deploys.
    pub blob_s3_bucket: Option<String>,
    pub blob_s3_endpoint: Option<String>,
    pub blob_s3_region: Option<String>,
    pub blob_s3_access_key_id: Option<String>,
    pub blob_s3_secret_access_key: Option<String>,
    pub base_url: String,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub oauth_token_key: String,
    pub max_tarball_bytes: usize,
    pub max_dependencies: usize,
    // IP-keyed request rate limiting. Enabled by default; the global limit
    // guards the API surface and a stricter fixed limit guards auth endpoints
    // (see `ratelimit`). Disable only behind a trusted gateway that limits for us.
    pub rate_limit_enabled: bool,
    pub rate_limit_rps: u32,
    pub rate_limit_burst: u32,
    // The install/download hot path (package metadata + tarball fetch) gets its
    // own generous tier: resolving one project pulls many packages in a burst
    // from a single IP, so a strict shared limit would 429 legitimate installs.
    pub rate_limit_read_rps: u32,
    pub rate_limit_read_burst: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "0.0.0.0".into(),
            port: 3000,
            database_url: "sqlite://data/registry.db?mode=rwc".into(),
            blob_dir: "data/blobs".into(),
            blob_s3_bucket: None,
            blob_s3_endpoint: None,
            blob_s3_region: None,
            blob_s3_access_key_id: None,
            blob_s3_secret_access_key: None,
            base_url: "http://localhost:3000".into(),
            github_client_id: None,
            github_client_secret: None,
            oauth_token_key: DEFAULT_OAUTH_TOKEN_KEY.into(),
            max_tarball_bytes: 50 * 1024 * 1024, // 50 MB
            max_dependencies: 64,
            rate_limit_enabled: true,
            rate_limit_rps: 20,
            rate_limit_burst: 40,
            rate_limit_read_rps: 100,
            rate_limit_read_burst: 500,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/registry.db?mode=rwc".into()),
            blob_dir: env::var("BLOB_DIR").unwrap_or_else(|_| "data/blobs".into()),
            // Fall back to the standard AWS SDK / Fly Tigris variable names, so
            // `fly storage create` (which injects BUCKET_NAME, AWS_ENDPOINT_URL_S3,
            // AWS_REGION, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY) works with no
            // extra config.
            blob_s3_bucket: env_any(&["BLOB_S3_BUCKET", "BUCKET_NAME"]),
            blob_s3_endpoint: env_any(&["BLOB_S3_ENDPOINT", "AWS_ENDPOINT_URL_S3"]),
            blob_s3_region: env_any(&["BLOB_S3_REGION", "AWS_REGION"]),
            blob_s3_access_key_id: env_any(&["BLOB_S3_ACCESS_KEY_ID", "AWS_ACCESS_KEY_ID"]),
            blob_s3_secret_access_key: env_any(&[
                "BLOB_S3_SECRET_ACCESS_KEY",
                "AWS_SECRET_ACCESS_KEY",
            ]),
            base_url: env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into()),
            github_client_id: env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").ok(),
            oauth_token_key: env::var("OAUTH_TOKEN_KEY")
                .unwrap_or_else(|_| DEFAULT_OAUTH_TOKEN_KEY.into()),
            max_tarball_bytes: env::var("MAX_TARBALL_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50 * 1024 * 1024), // 50 MB
            max_dependencies: env::var("MAX_DEPENDENCIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(64),
            rate_limit_enabled: env::var("RATE_LIMIT_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            rate_limit_rps: env::var("RATE_LIMIT_RPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v| v > 0)
                .unwrap_or(20),
            rate_limit_burst: env::var("RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v| v > 0)
                .unwrap_or(40),
            rate_limit_read_rps: env::var("RATE_LIMIT_READ_RPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v| v > 0)
                .unwrap_or(100),
            rate_limit_read_burst: env::var("RATE_LIMIT_READ_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v| v > 0)
                .unwrap_or(500),
        }
    }

    /// Whether GitHub OAuth (and thus encrypted token storage) is enabled.
    pub fn github_enabled(&self) -> bool {
        self.github_client_id.is_some() && self.github_client_secret.is_some()
    }

    /// Fail-closed check for secrets that must be set before a live deploy.
    /// Returns an error (rather than silently running insecurely) when GitHub
    /// OAuth is enabled but `OAUTH_TOKEN_KEY` is still the compiled-in default.
    pub fn check_production_secrets(&self) -> Result<(), String> {
        if self.github_enabled() && self.oauth_token_key == DEFAULT_OAUTH_TOKEN_KEY {
            return Err(
                "OAUTH_TOKEN_KEY is set to the insecure default; set a unique 32-byte \
                 OAUTH_TOKEN_KEY before enabling GitHub OAuth (stored tokens are \
                 encrypted with it)"
                    .into(),
            );
        }
        Ok(())
    }
}
