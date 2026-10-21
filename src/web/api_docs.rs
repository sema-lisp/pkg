use crate::{
    api_docs::{ApiDocument, ApiEntry},
    dal, AppState,
};
use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

#[derive(Clone)]
struct EntryView {
    entry: ApiEntry,
    definition_html: String,
}

#[derive(Template)]
#[template(path = "api_reference.html")]
struct ReferenceTemplate {
    username: Option<String>,
    is_admin: bool,
    base_url: String,
    name: String,
    version: String,
    description: String,
    release_version: Option<String>,
    api_section: bool,
    source_label: Option<String>,
    source_view: bool,
    index_entries: Vec<ApiEntry>,
    entries: Vec<EntryView>,
    message: String,
    source_path: String,
    source_url: String,
    parser_version: String,
    archive_sha256: String,
    yanked: bool,
}

#[derive(Template)]
#[template(path = "api_reference_print.html")]
struct PrintTemplate {
    name: String,
    version: String,
    entries: Vec<ApiEntry>,
    message: String,
    source_path: String,
    source_url: String,
    parser_version: String,
    archive_sha256: String,
    yanked: bool,
}

struct SourceLine {
    number: usize,
    html: String,
}

#[derive(Template)]
#[template(path = "api_source.html")]
struct SourceTemplate {
    username: Option<String>,
    is_admin: bool,
    base_url: String,
    name: String,
    version: String,
    release_version: Option<String>,
    api_section: bool,
    source_label: Option<String>,
    source_view: bool,
    index_entries: Vec<ApiEntry>,
    description: String,
    source_url: String,
    source_path: String,
    source_sha256: String,
    source_sha256_short: String,
    lines: Vec<SourceLine>,
}

struct Release {
    package: crate::entity::package::Model,
    version: crate::dal::versions::ReleaseCandidate,
    document: Option<ApiDocument>,
}

async fn release(state: &AppState, name: &str, version: &str) -> Result<Release, StatusCode> {
    if !state.config.api_docs_enabled {
        return Err(StatusCode::NOT_FOUND);
    }
    let package = dal::packages::find_by_name(&state.db, name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let version = dal::versions::list_candidates_for_package(&state.db, package.id)
        .await
        .into_iter()
        .find(|candidate| candidate.version == version)
        .ok_or(StatusCode::NOT_FOUND)?;
    let document = dal::api_docs::find(&state.db, version.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Release {
        package,
        version,
        document,
    })
}

fn message(release: &Release) -> String {
    match &release.document {
        Some(document) => document.message.clone(),
        None if release.version.tarball_url.is_some() => "API documentation is unavailable for this GitHub release because the registry has no stored source archive. Read the release README or visit its repository.".into(),
        None => "API documentation has not been generated for this release. Its README is still available on the package page.".into(),
    }
}

fn source_url(name: &str, version: &str, path: &str) -> String {
    let path = path
        .split('/')
        .map(crate::syndication::url::seg)
        .collect::<Vec<_>>()
        .join("/");
    format!("/packages/{name}/{version}/source/{path}")
}

/// Remove the enclosing module's indentation for display. The stored source
/// and the complete source viewer retain the original bytes and line numbers.
fn definition_source(text: &str, start: usize, end: usize) -> String {
    if start == 0 || end < start {
        return String::new();
    }
    let lines: Vec<_> = text.lines().skip(start - 1).take(end - start + 1).collect();
    let indent = lines.first().map_or(0, |line| {
        line.bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count()
    });
    lines
        .iter()
        .map(|line| {
            let trim = line
                .bytes()
                .take(indent)
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count();
            &line[trim..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn reference(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((name, version)): Path<(String, String)>,
) -> Response {
    let release = match release(&state, &name, &version).await {
        Ok(release) => release,
        Err(status) => return status.into_response(),
    };
    let message = message(&release);
    let session = super::get_session_info(&state, &headers).await;
    let source = release
        .document
        .as_ref()
        .and_then(|document| document.source.as_ref());
    let entries = release
        .document
        .as_ref()
        .map(|document| {
            document
                .entries
                .iter()
                .map(|entry| {
                    let definition_html = match (source, entry.line, entry.end_line) {
                        (Some(source), Some(start), Some(end)) => {
                            crate::github_sync::highlight_source(&definition_source(
                                &source.text,
                                start,
                                end,
                            ))
                            .join("")
                        }
                        _ => String::new(),
                    };
                    EntryView {
                        entry: entry.clone(),
                        definition_html,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    super::render(ReferenceTemplate {
        username: session.username,
        is_admin: session.is_admin,
        base_url: state.config.site_url().into(),
        release_version: Some(version.clone()),
        api_section: true,
        source_label: None,
        source_view: false,
        index_entries: release
            .document
            .as_ref()
            .map(|document| document.entries.clone())
            .unwrap_or_default(),
        description: release.package.description,
        message,
        entries,
        source_path: source.map(|source| source.path.clone()).unwrap_or_default(),
        source_url: source
            .map(|source| source_url(&name, &version, &source.path))
            .unwrap_or_default(),
        parser_version: release
            .document
            .as_ref()
            .map(|document| document.parser_version.clone())
            .unwrap_or_default(),
        archive_sha256: release
            .document
            .as_ref()
            .map(|document| document.archive_sha256.clone())
            .unwrap_or_default(),
        yanked: release.version.yanked != 0,
        name,
        version,
    })
    .into_response()
}

pub async fn print(
    State(state): State<Arc<AppState>>,
    Path((name, version)): Path<(String, String)>,
) -> Response {
    let release = match release(&state, &name, &version).await {
        Ok(release) => release,
        Err(status) => return status.into_response(),
    };
    let message = message(&release);
    let source = release
        .document
        .as_ref()
        .and_then(|document| document.source.as_ref());
    let template = PrintTemplate {
        entries: release
            .document
            .as_ref()
            .map(|document| document.entries.clone())
            .unwrap_or_default(),
        source_path: source.map(|source| source.path.clone()).unwrap_or_default(),
        source_url: source
            .map(|source| {
                format!(
                    "{}{}",
                    state.config.site_url(),
                    source_url(&name, &version, &source.path)
                )
            })
            .unwrap_or_default(),
        parser_version: release
            .document
            .as_ref()
            .map(|document| document.parser_version.clone())
            .unwrap_or_default(),
        archive_sha256: release
            .document
            .as_ref()
            .map(|document| document.archive_sha256.clone())
            .unwrap_or_default(),
        yanked: release.version.yanked != 0,
        name,
        version,
        message,
    };
    let mut response = super::render(template).into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'",
        ),
    );
    response
}

pub async fn source(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((name, version, path)): Path<(String, String, String)>,
) -> Response {
    let release = match release(&state, &name, &version).await {
        Ok(release) => release,
        Err(status) => return status.into_response(),
    };
    let Some(source) = release
        .document
        .as_ref()
        .and_then(|document| document.source.as_ref())
        .filter(|source| source.path == path)
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let session = super::get_session_info(&state, &headers).await;
    let lines = crate::github_sync::highlight_source(&source.text)
        .into_iter()
        .enumerate()
        .map(|(index, html)| SourceLine {
            number: index + 1,
            html,
        })
        .collect();
    super::render(SourceTemplate {
        username: session.username,
        is_admin: session.is_admin,
        base_url: state.config.site_url().into(),
        release_version: Some(version.clone()),
        api_section: true,
        source_label: Some(source.path.clone()),
        source_view: true,
        index_entries: release
            .document
            .as_ref()
            .map(|document| document.entries.clone())
            .unwrap_or_default(),
        description: release.package.description,
        source_url: source_url(&name, &version, &source.path),
        name,
        version,
        source_path: source.path.clone(),
        source_sha256_short: source.sha256.chars().take(12).collect(),
        source_sha256: source.sha256.clone(),
        lines,
    })
    .into_response()
}

fn markdown_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fence(value: &str) -> String {
    let length = value
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or_default()
        .max(2)
        + 1;
    "`".repeat(length)
}

pub async fn markdown(
    State(state): State<Arc<AppState>>,
    Path((name, version)): Path<(String, String)>,
) -> Response {
    let release = match release(&state, &name, &version).await {
        Ok(release) => release,
        Err(status) => return status.into_response(),
    };
    let mut output = format!("# {name} {version} — API reference\n\nSource declarations extracted without executing package code. Signatures describe declarations, not inferred runtime behavior.\n\n");
    if release.version.yanked != 0 {
        output.push_str("This release is yanked.\n\n");
    }
    let message = message(&release);
    if !message.is_empty() {
        output.push_str(&format!("{}\n\n", markdown_text(&message)));
    }
    if let Some(document) = release.document {
        for entry in document.entries {
            output.push_str(&format!("## {}\n\n", markdown_text(&entry.name)));
            if let Some(signature) = entry.signature() {
                let fence = fence(&signature);
                output.push_str(&format!("{fence}sema\n{signature}\n{fence}\n\n"));
            }
            if let Some(docstring) = entry.docstring {
                output.push_str(&format!("{}\n\n", markdown_text(&docstring)));
            }
            if let Some(reason) = entry.unresolved_reason {
                output.push_str(&format!("{}\n\n", markdown_text(&reason)));
            }
            if let (Some(source), Some(line)) = (&document.source, entry.line) {
                output.push_str(&format!(
                    "[Source line {line}]({}{}#L{line})\n\n",
                    state.config.site_url(),
                    source_url(&name, &version, &source.path)
                ));
            }
        }
        output.push_str(&format!(
            "---\n\nParser: {}\n\nArchive SHA-256: `{}`\n",
            document.parser_version, document.archive_sha256
        ));
    }
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        output,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::definition_source;

    #[test]
    fn definition_display_removes_module_indent_without_flattening_body() {
        let source =
            "(module example\n  (define (f x)\n    \"Description\nContinuation\"\n    (+ x 1)))\n";
        assert_eq!(
            definition_source(source, 2, 5),
            "(define (f x)\n  \"Description\nContinuation\"\n  (+ x 1)))"
        );
        assert_eq!(definition_source(source, 0, 5), "");
        assert_eq!(definition_source(source, 5, 2), "");
    }
}
