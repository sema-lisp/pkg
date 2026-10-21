use crate::{crypto, dal, db::Db};
use sea_orm::TransactionTrait;

const MAX_RELEASE_DOCUMENT_BYTES: usize = 512 * 1024;

pub fn validate_sema_version_req(requirement: Option<&str>) -> Result<Option<String>, String> {
    let Some(requirement) = requirement.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if requirement.len() > 128 {
        return Err("sema_version_req must be at most 128 characters".to_string());
    }
    semver::VersionReq::parse(requirement)
        .map_err(|error| format!("Invalid sema_version_req {requirement:?}: {error}"))?;
    Ok(Some(requirement.to_string()))
}

/// Fetch the decrypted GitHub access token for a user.
pub async fn get_github_token(db: &Db, user_id: i64, token_key: &str) -> Option<String> {
    let row = crate::dal::oauth::find_active(db, user_id).await.ok()??;

    crypto::decrypt(&row.access_token_enc, token_key)
}

/// Mark a user's GitHub connection as revoked (e.g. after a 401).
pub async fn mark_token_revoked(db: &Db, user_id: i64) {
    let _ = crate::dal::oauth::mark_revoked(db, user_id).await;
}

/// Validate that a GitHub repo exists and contains sema.toml. Returns the parsed manifest.
pub async fn validate_repo(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<RepoManifest, String> {
    let resp = client
        .get(format!("https://api.github.com/repos/{owner}/{repo}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "sema-pkg")
        .send()
        .await
        .map_err(|e| format!("Failed to reach GitHub: {e}"))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("GitHub token is invalid or revoked".into());
    }
    if !resp.status().is_success() {
        return Err(format!(
            "Repository {owner}/{repo} not found or not accessible"
        ));
    }

    let toml_resp = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{repo}/contents/sema.toml"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "sema-pkg")
        .header("Accept", "application/vnd.github.raw+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch sema.toml: {e}"))?;

    if !toml_resp.status().is_success() {
        return Err(format!("No sema.toml found in {owner}/{repo}"));
    }

    let toml_content = toml_resp
        .text()
        .await
        .map_err(|e| format!("Failed to read sema.toml: {e}"))?;
    parse_manifest(&toml_content)
}

#[derive(Debug, Clone)]
pub struct RepoManifest {
    pub name: String,
    pub description: String,
    pub repository_url: Option<String>,
    pub sema_version_req: Option<String>,
}

fn parse_manifest(content: &str) -> Result<RepoManifest, String> {
    let doc: toml::Value =
        toml::from_str(content).map_err(|e| format!("Invalid sema.toml: {e}"))?;
    let pkg = doc
        .get("package")
        .ok_or("sema.toml missing [package] section")?;
    let pkg = match pkg {
        toml::Value::Table(t) => t,
        _ => return Err("sema.toml [package] must be a table".into()),
    };
    let name = pkg
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or("sema.toml [package] missing 'name'")?;
    let description = pkg
        .get("description")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();
    let repository_url = pkg
        .get("repository")
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let sema_version_req = match pkg.get("sema_version_req") {
        None => None,
        Some(value) => {
            let requirement = value.as_str().ok_or("sema_version_req must be a string")?;
            validate_sema_version_req(Some(requirement))?
        }
    };
    Ok(RepoManifest {
        name: name.to_string(),
        description,
        repository_url,
        sema_version_req,
    })
}

/// List semver tags from a GitHub repo. Strips leading 'v' prefix.
/// Returns (tag_name, semver_version) pairs sorted newest-first.
pub async fn list_semver_tags(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<(String, semver::Version)>, String> {
    let mut tags = Vec::new();
    let mut page = 1u32;

    loop {
        let resp = client
            .get(format!(
                "https://api.github.com/repos/{owner}/{repo}/tags?per_page=100&page={page}"
            ))
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "sema-pkg")
            .send()
            .await
            .map_err(|e| format!("Failed to list tags: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Failed to list tags ({})", resp.status()));
        }

        let items: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| format!("Invalid response: {e}"))?;
        if items.is_empty() {
            break;
        }

        for item in &items {
            if let Some(tag_name) = item.get("name").and_then(|n| n.as_str()) {
                let version_str = tag_name.strip_prefix('v').unwrap_or(tag_name);
                if let Ok(ver) = semver::Version::parse(version_str) {
                    tags.push((tag_name.to_string(), ver));
                }
            }
        }

        if items.len() < 100 {
            break;
        }
        page += 1;
    }

    tags.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(tags)
}

/// Everything needed to read one GitHub repository.
///
/// Grouping these keeps the owner, repo, and tag from being transposed at a call site
/// where they are all `&str`.
pub struct RepoAccess<'a> {
    pub client: &'a reqwest::Client,
    pub token: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
}

#[derive(Debug, Default)]
struct TagMetadata {
    sema_version_req: Option<String>,
    dependencies: Vec<(String, String)>,
}

/// Fetch a text file from one tag without downloading the repository archive.
/// GitHub resolves the tag at request time; we save the returned text on the
/// new release row and never refresh it after that release exists.
async fn fetch_tag_file(
    access: &RepoAccess<'_>,
    tag_name: &str,
    path: &str,
) -> Result<Option<String>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/{path}",
        access.owner, access.repo
    );
    let response = access
        .client
        .get(url)
        .query(&[("ref", tag_name)])
        .header("Authorization", format!("Bearer {}", access.token))
        .header("User-Agent", "sema-pkg")
        .header("Accept", "application/vnd.github.raw+json")
        .send()
        .await
        .map_err(|error| format!("Failed to fetch {path} at {tag_name}: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch {path} at {tag_name}: GitHub returned {}",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RELEASE_DOCUMENT_BYTES as u64)
    {
        return Err(format!(
            "{path} at {tag_name} exceeds the {} byte capture limit",
            MAX_RELEASE_DOCUMENT_BYTES
        ));
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Failed to read {path} at {tag_name}: {error}"))?
    {
        if bytes.len() + chunk.len() > MAX_RELEASE_DOCUMENT_BYTES {
            return Err(format!(
                "{path} at {tag_name} exceeds the {} byte capture limit",
                MAX_RELEASE_DOCUMENT_BYTES
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("{path} at {tag_name} is not UTF-8: {error}"))
}

async fn fetch_tag_document(
    access: &RepoAccess<'_>,
    tag_name: &str,
    paths: &[&str],
) -> Result<Option<String>, String> {
    // A missing filename is normal. Any other fetch failure aborts the tag
    // import so a retry can capture the release documentation atomically.
    for path in paths {
        if let Some(content) = fetch_tag_file(access, tag_name, path).await? {
            return Ok(Some(content));
        }
    }
    Ok(None)
}

fn tag_metadata(
    content: &str,
    tag_name: &str,
    max_dependencies: usize,
) -> Result<TagMetadata, String> {
    let document = toml::from_str::<toml::Value>(content)
        .map_err(|error| format!("{tag_name}: invalid sema.toml: {error}"))?;
    let sema_version_req = match document
        .get("package")
        .and_then(|package| package.get("sema_version_req"))
    {
        None => None,
        Some(value) => {
            let requirement = value
                .as_str()
                .ok_or_else(|| format!("{tag_name}: sema_version_req must be a string"))?;
            validate_sema_version_req(Some(requirement))
                .map_err(|error| format!("{tag_name}: {error}"))?
        }
    };
    let mut dependencies = Vec::new();
    if let Some(deps) = document.get("deps").and_then(toml::Value::as_table) {
        if deps.len() > max_dependencies {
            return Err(format!(
                "{tag_name}: too many dependencies (max {max_dependencies})"
            ));
        }
        for (name, value) in deps {
            if name.contains('/') {
                return Err(format!(
                    "{tag_name}: git dependency {name:?} cannot be represented by the registry"
                ));
            }
            crate::api::packages::validate_package_name(name)
                .map_err(|error| format!("{tag_name}: invalid dependency {name:?}: {error}"))?;
            let version_req = value
                .as_str()
                .ok_or_else(|| format!("{tag_name}: dependency {name:?} must be a string"))?;
            semver::VersionReq::parse(version_req).map_err(|error| {
                format!(
                    "{tag_name}: invalid version requirement {version_req:?} for {name:?}: {error}"
                )
            })?;
            dependencies.push((name.clone(), version_req.to_string()));
        }
    }
    Ok(TagMetadata {
        sema_version_req,
        dependencies,
    })
}

async fn fetch_tag_metadata(
    access: &RepoAccess<'_>,
    tag_name: &str,
    max_dependencies: usize,
) -> Result<TagMetadata, String> {
    let Some(content) = fetch_tag_file(access, tag_name, "sema.toml").await? else {
        return Ok(TagMetadata::default());
    };
    tag_metadata(&content, tag_name, max_dependencies)
}

/// Sync a single tag: store metadata and GitHub tarball URL (no blob download).
/// Returns Ok(true) if version was created, Ok(false) if it already existed.
pub async fn sync_tag(
    db: &Db,
    access: &RepoAccess<'_>,
    tag_name: &str,
    version: &semver::Version,
    package_id: i64,
    max_dependencies: usize,
) -> Result<bool, String> {
    let version_str = version.to_string();

    // Check if version already exists
    let exists = crate::dal::versions::exists(db, package_id, &version_str)
        .await
        .unwrap_or(false);

    if exists {
        return Ok(false);
    }

    // Read every supported metadata field at the tag, not from the default
    // branch. Once inserted these fields are never refreshed, so an upstream
    // edit cannot rewrite an older release page.
    let metadata = fetch_tag_metadata(access, tag_name, max_dependencies).await?;
    let readme_raw = fetch_tag_document(
        access,
        tag_name,
        &[
            "README.md",
            "README.markdown",
            "README",
            "readme.md",
            "readme.markdown",
            "readme",
        ],
    )
    .await?;
    let changelog_raw = fetch_tag_document(
        access,
        tag_name,
        &[
            "CHANGELOG.md",
            "CHANGELOG.markdown",
            "CHANGELOG",
            "changelog.md",
            "changelog.markdown",
            "changelog",
        ],
    )
    .await?;
    let docs = crate::dal::versions::ReleaseDocuments {
        readme_html: readme_raw.as_deref().map(render_readme),
        changelog_html: changelog_raw.as_deref().map(render_readme),
        readme_raw,
        changelog_raw,
    };

    let (owner, repo) = (access.owner, access.repo);
    let tarball_url = format!("https://api.github.com/repos/{owner}/{repo}/tarball/{tag_name}");

    let txn = db
        .begin()
        .await
        .map_err(|error| format!("Failed to begin version transaction: {error}"))?;
    let release = crate::dal::versions::create_github_version(
        &txn,
        package_id,
        &version_str,
        tarball_url,
        metadata.sema_version_req,
        docs,
    )
    .await
    .map_err(|e| format!("Failed to insert version: {e}"))?;

    for (name, version_req) in metadata.dependencies {
        dal::deps::insert(&txn, release.id, &name, &version_req)
            .await
            .map_err(|e| format!("Failed to insert dependency: {e}"))?;
    }
    txn.commit()
        .await
        .map_err(|error| format!("Failed to commit version transaction: {error}"))?;

    let _ = crate::dal::sync_log::record_ok(db, package_id, tag_name).await;

    Ok(true)
}

/// The syntect syntax set: syntect's defaults plus the canonical Sema grammar
/// (the same `.sublime-syntax` the editors use, whose scopes mirror the website's
/// TextMate grammar), so ` ```sema ` fences highlight with the real Sema grammar.
fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    use std::sync::OnceLock;
    use syntect::parsing::{SyntaxDefinition, SyntaxSet};
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(|| {
        let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
        match SyntaxDefinition::load_from_str(
            include_str!("../syntaxes/Sema.sublime-syntax"),
            true,
            Some("Sema"),
        ) {
            Ok(def) => builder.add(def),
            Err(e) => tracing::error!("failed to load Sema syntax: {e}"),
        }
        builder.build()
    })
}

/// Parse a `#rrggbb` string into a syntect Color (opaque).
fn hex_color(s: &str) -> syntect::highlighting::Color {
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    syntect::highlighting::Color {
        r: byte(1),
        g: byte(3),
        b: byte(5),
        a: 0xFF,
    }
}

/// The `sema-dark` theme — a direct port of the website's code theme
/// (`website/.vitepress/sema-code-theme.json`), mapping the same scopes the
/// grammar emits, so README code looks identical to the docs.
fn sema_theme() -> syntect::highlighting::Theme {
    use std::str::FromStr;
    use syntect::highlighting::{
        FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
    };
    let item = |scope: &str, fg: &str, italic: bool| ThemeItem {
        scope: ScopeSelectors::from_str(scope).unwrap_or_default(),
        style: StyleModifier {
            foreground: Some(hex_color(fg)),
            background: None,
            font_style: italic.then_some(FontStyle::ITALIC),
        },
    };
    Theme {
        name: Some("sema-dark".to_string()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(hex_color("#e9e3d6")),
            background: Some(hex_color("#181512")),
            ..ThemeSettings::default()
        },
        scopes: vec![
            item("comment", "#6b6354", true),
            item("string, constant.character", "#a8c47a", false),
            item(
                "constant.numeric, constant.language.boolean, constant.language.nil",
                "#d19a66",
                false,
            ),
            item("constant.other.keyword", "#7aacb8", false),
            item(
                "keyword.control, keyword.control.definition, keyword.operator.threading",
                "#c8a855",
                false,
            ),
            item(
                "punctuation, keyword.operator.quote, keyword.operator.quasiquote, keyword.operator.unquote, keyword.operator.unquote-splicing",
                "#6a6258",
                false,
            ),
        ],
    }
}

/// Highlight stored source with the same grammar and palette as README code.
/// The returned fragments escape source text before adding trusted markup.
pub fn highlight_source(source: &str) -> Vec<String> {
    use syntect::{
        easy::HighlightLines,
        html::{styled_line_to_highlighted_html, IncludeBackground},
        util::LinesWithEndings,
    };
    let set = syntax_set();
    let syntax = set
        .find_syntax_by_name("Sema")
        .unwrap_or_else(|| set.find_syntax_plain_text());
    let mut theme = sema_theme();
    // Source viewers need readable punctuation and comments across long files.
    for scope in &mut theme.scopes {
        if matches!(scope.style.foreground, Some(color) if color == hex_color("#6b6354") || color == hex_color("#6a6258"))
        {
            scope.style.foreground = Some(hex_color("#a39a89"));
        }
    }
    let mut highlighter = HighlightLines::new(syntax, &theme);
    LinesWithEndings::from(source)
        .map(|line| {
            highlighter
                .highlight_line(line, set)
                .ok()
                .and_then(|ranges| {
                    styled_line_to_highlighted_html(&ranges, IncludeBackground::No).ok()
                })
                .unwrap_or_else(|| crate::syndication::xml::escape(line))
        })
        .collect()
}

/// Render a Markdown README to HTML using comrak (GitHub Flavored Markdown) with
/// syntect syntax highlighting — the real Sema grammar + the docs' color theme.
pub fn render_readme(markdown: &str) -> String {
    use comrak::plugins::syntect::SyntectAdapterBuilder;
    use comrak::{markdown_to_html_with_plugins, Options, Plugins};
    use syntect::highlighting::ThemeSet;

    let mut options = Options::default();
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.header_ids = Some(String::new());
    options.render.unsafe_ = false;

    // ThemeSet isn't Clone; build one holding just our theme (README rendering
    // only happens on publish/backfill, so the cost is irrelevant).
    let mut themes = ThemeSet::default();
    themes.themes.insert("sema-dark".to_string(), sema_theme());

    let adapter = SyntectAdapterBuilder::new()
        .theme("sema-dark")
        .theme_set(themes)
        .syntax_set(syntax_set().clone())
        .build();
    let mut plugins = Plugins::default();
    plugins.render.codefence_syntax_highlighter = Some(&adapter);
    let html = markdown_to_html_with_plugins(markdown, &options, &plugins);
    strip_pre_background(&html)
}

/// Drop syntect's theme `background-color` from `<pre>` so the site's own code
/// background (`--bg-code`) shows through — keeping only the token colors, which
/// avoids a two-tone look where code blocks differ from the page.
fn strip_pre_background(html: &str) -> String {
    let mut out = html.to_string();
    while let Some(start) = out.find("background-color:#") {
        match out[start..].find(';') {
            Some(semi) => out.replace_range(start..start + semi + 1, ""),
            None => break,
        }
    }
    out.replace(" style=\"\"", "")
}

/// Register a webhook on a GitHub repository.
pub async fn register_webhook(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    webhook_url: &str,
    webhook_secret: &str,
) -> Result<(), String> {
    let body = serde_json::json!({
        "name": "web",
        "active": true,
        "events": ["push"],
        "config": {
            "url": webhook_url,
            "content_type": "json",
            "secret": webhook_secret,
            "insecure_ssl": "0"
        }
    });

    let resp = client
        .post(format!("https://api.github.com/repos/{owner}/{repo}/hooks"))
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "sema-pkg")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to register webhook: {e}"))?;

    if resp.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let errors = body.get("errors").and_then(|e| e.as_array());
        if let Some(errors) = errors {
            let already_exists = errors.iter().any(|e| {
                e.get("message")
                    .and_then(|m| m.as_str())
                    .map(|m| m.contains("already exists"))
                    .unwrap_or(false)
            });
            if already_exists {
                return Ok(());
            }
        }
        return Err(format!("Failed to register webhook: {}", body));
    }

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!("Failed to register webhook ({status})"));
    }

    Ok(())
}

/// Parse an "owner/repo" string from a GitHub URL.
/// Accepts: "github.com/owner/repo", "https://github.com/owner/repo", "https://github.com/owner/repo.git", "owner/repo"
pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let url = url.strip_suffix(".git").unwrap_or(url);
    let url = url.strip_prefix("https://").unwrap_or(url);
    let url = url.strip_prefix("http://").unwrap_or(url);
    let url = url.strip_prefix("github.com/").unwrap_or(url);

    let parts: Vec<&str> = url.splitn(3, '/').collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

/// Generate a random webhook secret.
pub fn generate_webhook_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod readme_tests {
    use super::*;

    #[test]
    fn highlights_common_and_sema_fences() {
        let md = "```bash\nsema pkg add x\n```\n\n```json\n{\"a\":1}\n```\n\n```sema\n(graphql/client \"u\")\n```\n";
        let html = render_readme(md);
        // syntect highlighting emits inline color styles
        assert!(
            html.contains("style=\"color"),
            "expected syntect color spans, got:\n{html}"
        );
        // The sema block must be highlighted by the real Sema grammar:
        // find the last <pre> (the sema one) and confirm it has color spans.
        let last_pre = html.rfind("<pre").map(|i| &html[i..]).unwrap_or("");
        assert!(
            last_pre.contains("style=\"color"),
            "sema code block not highlighted:\n{last_pre}"
        );
        // The theme background is stripped so the site's --bg-code shows through.
        assert!(
            !html.contains("background-color"),
            "syntect theme background should be stripped:\n{html}"
        );
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    #[test]
    fn manifest_rejects_invalid_sema_version_requirement() {
        let error = parse_manifest(
            r#"
            [package]
            name = "policies"
            sema_version_req = "not a requirement"
            "#,
        )
        .unwrap_err();
        assert!(error.contains("Invalid sema_version_req"));
    }

    #[test]
    fn manifest_normalizes_valid_sema_version_requirement() {
        let manifest = parse_manifest(
            r#"
            [package]
            name = "policies"
            sema_version_req = "  >=1.34.0  "
            "#,
        )
        .unwrap();
        assert_eq!(manifest.sema_version_req.as_deref(), Some(">=1.34.0"));
    }

    #[test]
    fn manifest_rejects_non_string_sema_version_requirement() {
        let error = parse_manifest(
            r#"
            [package]
            name = "policies"
            sema_version_req = 34
            "#,
        )
        .unwrap_err();
        assert_eq!(error, "sema_version_req must be a string");
    }

    #[test]
    fn tag_manifest_reads_the_requirement_at_that_tag() {
        let content = "[package]\nname = \"policies\"\nsema_version_req = \"  >=1.34.0  \"\n";
        assert_eq!(
            tag_metadata(content, "v1.0.0", 64)
                .unwrap()
                .sema_version_req,
            Some(">=1.34.0".to_string())
        );
    }

    #[test]
    fn tag_manifest_without_a_requirement_is_absent_not_an_error() {
        // A tag older than the field, or older than sema.toml itself, must still import.
        assert_eq!(
            tag_metadata("[package]\nname = \"policies\"\n", "v0.1.0", 64)
                .unwrap()
                .sema_version_req,
            None
        );
        assert_eq!(
            tag_metadata("", "v0.1.0", 64).unwrap().sema_version_req,
            None
        );
        let error = tag_metadata("not : valid : toml", "v0.1.0", 64).unwrap_err();
        assert!(error.starts_with("v0.1.0: invalid sema.toml:"), "{error}");
    }

    #[test]
    fn tag_manifest_with_a_bad_requirement_names_the_tag() {
        let error = tag_metadata(
            "[package]\nsema_version_req = \"not a requirement\"\n",
            "v1.2.3",
            64,
        )
        .unwrap_err();
        assert!(error.starts_with("v1.2.3: "), "{error}");
        assert!(error.contains("Invalid sema_version_req"), "{error}");

        let error = tag_metadata("[package]\nsema_version_req = 34\n", "v1.2.3", 64).unwrap_err();
        assert_eq!(error, "v1.2.3: sema_version_req must be a string");
    }

    #[test]
    fn tag_manifest_preserves_supported_dependencies() {
        let metadata = tag_metadata(
            "[deps]\nforms = \"^1.2\"\npolicy = \">=2.0, <3.0\"\n",
            "v1.2.3",
            2,
        )
        .unwrap();
        assert_eq!(
            metadata.dependencies,
            vec![
                ("forms".to_string(), "^1.2".to_string()),
                ("policy".to_string(), ">=2.0, <3.0".to_string()),
            ]
        );
    }

    #[test]
    fn tag_manifest_rejects_unrepresentable_or_excess_dependencies() {
        let error = tag_metadata("[deps]\n\"org/forms\" = \"1.0\"\n", "v1.2.3", 10).unwrap_err();
        assert!(error.contains("git dependency"), "{error}");

        let error = tag_metadata("[deps]\na = \"1.0\"\nb = \"1.0\"\n", "v1.2.3", 1).unwrap_err();
        assert_eq!(error, "v1.2.3: too many dependencies (max 1)");
    }

    #[test]
    fn validator_treats_blank_as_absent_and_limits_length() {
        assert_eq!(validate_sema_version_req(Some("  ")).unwrap(), None);
        let requirement = format!(">={}", "1".repeat(128));
        let error = validate_sema_version_req(Some(&requirement)).unwrap_err();
        assert!(error.contains("at most 128 characters"));
    }
}

#[cfg(test)]
mod theme_probe {
    use super::*;
    #[test]
    fn sema_uses_website_colors() {
        let html = render_readme("```sema\n(define (f x) (if x 1 nil))\n```\n");
        // gold keyword.control (#c8a855) and green string/nil etc from the theme
        assert!(
            html.to_lowercase().contains("c8a855"),
            "expected gold keyword color:\n{html}"
        );
    }
}
