//! Static API declarations from a release archive. Package code is never run.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Component, Path},
    rc::Rc,
};

use sema_core::{SpanMap, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PARSER_VERSION: &str = "sema-reader 1.37.0";
const MAX_EXPANDED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 512 * 1024;
const MAX_FILES: usize = 4096;
const MAX_EXPORTS: usize = 512;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiDocument {
    pub schema_version: u32,
    pub parser_version: String,
    pub archive_sha256: String,
    pub status: Status,
    pub message: String,
    pub source: Option<Source>,
    pub entries: Vec<ApiEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ready,
    Partial,
    Unsupported,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    pub sha256: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiEntry {
    pub anchor: String,
    pub module: String,
    pub name: String,
    pub parameters: Option<Vec<String>>,
    pub docstring: Option<String>,
    pub line: Option<usize>,
    pub end_line: Option<usize>,
    pub unresolved_reason: Option<String>,
}

impl ApiEntry {
    pub fn signature(&self) -> Option<String> {
        self.parameters.as_ref().map(|parameters| {
            let suffix = if parameters.is_empty() {
                String::new()
            } else {
                format!(" {}", parameters.join(" "))
            };
            format!("({}{suffix})", self.name)
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum CaptureError {
    #[error("The archive cannot be read within the API documentation limits.")]
    Archive(#[from] std::io::Error),
    #[error("{0}")]
    Unsupported(&'static str),
}

/// Capture a bounded, source-backed document, including a diagnostic for
/// unsupported packages. A documentation failure does not reject a release.
pub fn capture(archive: &[u8]) -> ApiDocument {
    let mut document = ApiDocument {
        schema_version: 1,
        parser_version: PARSER_VERSION.into(),
        archive_sha256: hex::encode(Sha256::digest(archive)),
        status: Status::Unsupported,
        message: String::new(),
        source: None,
        entries: Vec::new(),
    };
    match source_from_archive(archive) {
        Ok(source) => {
            // The reader interns symbols in thread-local storage. A fresh,
            // bounded-stack thread releases those allocations after each archive.
            let declarations = std::thread::scope(|scope| {
                std::thread::Builder::new()
                    .name("api-doc-parser".into())
                    .stack_size(8 * 1024 * 1024)
                    .spawn_scoped(scope, || declarations(&source.text))
                    .map_err(|_| "The API parser worker could not start.".to_string())
                    .and_then(|worker| {
                        worker
                            .join()
                            .map_err(|_| "The API parser worker failed.".to_string())
                    })
                    .and_then(|result| result)
            });
            match declarations {
                Ok(entries) => {
                    document.status = if entries.iter().any(|entry| entry.parameters.is_none()) {
                        Status::Partial
                    } else {
                        Status::Ready
                    };
                    document.entries = entries;
                }
                Err(error) => document.message = error,
            }
            document.source = Some(source);
        }
        Err(error) => document.message = error.to_string(),
    }
    document
}

fn archive_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => {
                let part = part.to_str()?;
                if part.contains('\\') {
                    return None;
                }
                parts.push(part);
            }
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn source_from_archive(bytes: &[u8]) -> Result<Source, CaptureError> {
    let decoder = flate2::read::GzDecoder::new(bytes).take(MAX_EXPANDED_BYTES);
    let mut archive = tar::Archive::new(decoder);
    let mut files = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for (index, entry) in archive.entries()?.enumerate() {
        if index >= MAX_FILES {
            return Err(CaptureError::Unsupported(
                "The archive contains too many files for API documentation.",
            ));
        }
        let mut entry = entry?;
        let entry_path = entry.path()?;
        // `tar -C package .` emits a root directory header before its files.
        if entry.header().entry_type().is_dir()
            && entry_path
                .components()
                .all(|part| part == Component::CurDir)
        {
            continue;
        }
        let path = archive_path(&entry_path).ok_or(CaptureError::Unsupported(
            "The archive contains an unsafe source path.",
        ))?;
        if !paths.insert(path.clone()) {
            return Err(CaptureError::Unsupported(
                "The archive contains duplicate paths.",
            ));
        }
        if !entry.header().entry_type().is_file() {
            continue;
        }
        if !(path.ends_with(".sema") || path == "sema.toml" || path.ends_with("/sema.toml")) {
            continue;
        }
        if entry.size() > MAX_SOURCE_BYTES {
            return Err(CaptureError::Unsupported(
                "A source file exceeds the 512 KiB API documentation limit.",
            ));
        }
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        files.insert(path, text);
    }
    if archive.into_inner().limit() == 0 {
        return Err(CaptureError::Unsupported(
            "The expanded archive exceeds the 16 MiB API documentation limit.",
        ));
    }
    let manifests: Vec<_> = files
        .keys()
        .filter(|path| path.as_str() == "sema.toml" || path.ends_with("/sema.toml"))
        .collect();
    let min_depth = manifests.iter().map(|path| path.split('/').count()).min();
    let candidates: Vec<_> = manifests
        .into_iter()
        .filter(|path| Some(path.split('/').count()) == min_depth)
        .collect();
    let [manifest_path] = candidates.as_slice() else {
        return Err(CaptureError::Unsupported(
            "API documentation needs one unambiguous sema.toml manifest.",
        ));
    };
    let manifest: toml::Value = toml::from_str(&files[*manifest_path])
        .map_err(|_| CaptureError::Unsupported("The package manifest is not valid TOML."))?;
    let entrypoint = manifest
        .get("package")
        .and_then(|package| package.get("entrypoint"))
        .and_then(toml::Value::as_str)
        .ok_or(CaptureError::Unsupported(
            "Declare [package].entrypoint in sema.toml to generate an API reference.",
        ))?;
    let relative = archive_path(Path::new(entrypoint)).ok_or(CaptureError::Unsupported(
        "The declared entrypoint must be a relative path inside the package.",
    ))?;
    let parent = Path::new(manifest_path).parent().unwrap_or(Path::new(""));
    let path = archive_path(&parent.join(relative)).ok_or(CaptureError::Unsupported(
        "The declared entrypoint path is invalid.",
    ))?;
    let text = files.remove(&path).ok_or(CaptureError::Unsupported(
        "The declared .sema entrypoint was not found as a regular file in the archive.",
    ))?;
    Ok(Source {
        path,
        sha256: hex::encode(Sha256::digest(text.as_bytes())),
        text,
    })
}

fn form(value: &Value, name: &str) -> bool {
    value
        .as_list()
        .and_then(|items| items.first())
        .and_then(Value::as_symbol)
        .as_deref()
        == Some(name)
}

fn source_lines(value: &Value, spans: &SpanMap) -> (Option<usize>, Option<usize>) {
    let span = value
        .as_list_rc()
        .and_then(|list| spans.get(&(Rc::as_ptr(&list) as usize)).copied());
    (span.map(|span| span.line), span.map(|span| span.end_line))
}

fn declarations(text: &str) -> Result<Vec<ApiEntry>, String> {
    let (ast, spans) = sema_reader::read_many_with_spans(text)
        .map_err(|_| "The entrypoint could not be parsed by sema-reader 1.37.0.".to_string())?;
    let modules: Vec<_> = ast.iter().filter(|value| form(value, "module")).collect();
    if modules.is_empty() {
        return Err("No literal top-level module declaration was found. API extraction does not execute package code.".into());
    }
    let mut result = Vec::new();
    let mut module_names = BTreeSet::new();
    for module in modules {
        let Some(items) = module.as_list() else {
            continue;
        };
        let name = items
            .get(1)
            .and_then(Value::as_symbol)
            .ok_or("The module name is not a literal symbol.")?;
        if !module_names.insert(name.clone()) {
            return Err("Repeated module declarations cannot be documented unambiguously.".into());
        }
        let mut exports = BTreeSet::new();
        let body = items.get(2..).unwrap_or_default();
        for export in body.iter().filter(|value| form(value, "export")) {
            for value in export.as_list().unwrap_or_default().iter().skip(1) {
                exports.insert(value.as_symbol().ok_or("An export declaration contains computed names. API extraction needs literal symbols.")?);
                if exports.len() + result.len() > MAX_EXPORTS {
                    return Err(
                        "The package exceeds the 512-export API documentation limit.".into(),
                    );
                }
            }
        }
        for export in exports {
            let definitions: Vec<_> = body
                .iter()
                .filter(|value| {
                    if !["define", "def", "defn", "defun", "defmacro"]
                        .iter()
                        .any(|name| form(value, name))
                    {
                        return false;
                    }
                    let binding = value.as_list().and_then(|items| items.get(1));
                    binding
                        .and_then(Value::as_symbol)
                        .or_else(|| {
                            binding
                                .and_then(Value::as_list)
                                .and_then(|items| items.first())
                                .and_then(Value::as_symbol)
                        })
                        .as_deref()
                        == Some(export.as_str())
                })
                .collect();
            let rebound = body.iter().any(|value| {
                form(value, "set!")
                    && value
                        .as_list()
                        .and_then(|items| items.get(1))
                        .and_then(Value::as_symbol)
                        .as_deref()
                        == Some(export.as_str())
            });
            let mut entry = ApiEntry {
                anchor: format!("fn-{}", hex::encode(format!("{name}/{export}"))), module: name.clone(), name: export.clone(),
                parameters: None, docstring: None, line: None, end_line: None,
                unresolved_reason: Some("This export has no single direct function declaration. See the source for its definition.".into()),
            };
            if definitions.len() == 1 && !rebound {
                let definition = definitions[0];
                (entry.line, entry.end_line) = source_lines(definition, &spans);
                let direct_function = form(definition, "define") || form(definition, "def");
                let definition = definition.as_list().unwrap_or_default();
                if let Some(signature) = definition
                    .get(1)
                    .and_then(Value::as_list)
                    .filter(|_| direct_function)
                {
                    let parameters: Option<Vec<_>> =
                        signature.iter().skip(1).map(Value::as_symbol).collect();
                    if definition.len() >= 3 && parameters.is_some() {
                        entry.parameters = parameters;
                        entry.unresolved_reason = None;
                        if definition.len() >= 4 {
                            entry.docstring = definition[2]
                                .as_str()
                                .map(str::trim)
                                .filter(|text| !text.is_empty())
                                .map(str::to_string);
                        }
                    }
                }
            } else if rebound || definitions.len() > 1 {
                entry.unresolved_reason = Some("This name is defined more than once or rebound. A single signature cannot be established from its declarations.".into());
            }
            result.push(entry);
        }
    }
    if result.is_empty() {
        return Err("No literal exports were found in the entrypoint's top-level modules.".into());
    }
    Ok(result)
}

/// Populate missing documents from verified stored uploads in bounded batches.
/// GitHub redirects and existing documents are left untouched.
pub async fn backfill(
    db: &crate::db::Db,
    blobs: &crate::blob::BlobStore,
) -> Result<(u64, u64), sea_orm::DbErr> {
    let (mut generated, mut skipped, mut after_id) = (0, 0, 0);
    loop {
        let targets = crate::dal::versions::document_backfill_targets(db, after_id, 100).await?;
        if targets.is_empty() {
            break;
        }
        for target in targets {
            after_id = target.id;
            if target.tarball_url.is_some()
                || target.blob_key.is_empty()
                || crate::dal::api_docs::find(db, target.id).await?.is_some()
            {
                skipped += 1;
                continue;
            }
            let Some(bytes) = blobs.read(&target.blob_key).await else {
                skipped += 1;
                continue;
            };
            if hex::encode(Sha256::digest(&bytes)) != target.checksum_sha256 {
                skipped += 1;
                continue;
            }
            let document = tokio::task::spawn_blocking(move || capture(&bytes))
                .await
                .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
            crate::dal::api_docs::insert(db, target.id, &document).await?;
            generated += 1;
        }
    }
    Ok((generated, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        let mut root = tar::Header::new_gnu();
        root.set_entry_type(tar::EntryType::Directory);
        root.set_mode(0o755);
        root.set_size(0);
        root.set_cksum();
        tar.append_data(&mut root, "./", std::io::empty()).unwrap();
        for (path, source) in files {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            header.set_size(source.len() as u64);
            header.set_cksum();
            tar.append_data(&mut header, path, source.as_bytes())
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }
    fn package(source: &str) -> ApiDocument {
        capture(&archive(&[
            ("sema.toml", "[package]\nentrypoint='package.sema'"),
            ("package.sema", source),
        ]))
    }
    #[test]
    fn published_retry_fixture_has_nine_real_declarations() {
        let document = capture(include_bytes!("../tests/fixtures/sema-retry-0.1.0.tar.gz"));
        assert_eq!(
            document.archive_sha256,
            "ea1f177349994e1cfc0684dfc796063ac389a583448f6b1a574c9b4595dcf82e"
        );
        assert_eq!(document.status, Status::Ready, "{}", document.message);
        assert_eq!(document.entries.len(), 9);
        let retry = document
            .entries
            .iter()
            .find(|entry| entry.name == "retry/with-retry")
            .unwrap();
        assert_eq!(
            retry.signature().as_deref(),
            Some("(retry/with-retry thunk . opts)")
        );
        assert_eq!(retry.line, Some(73));
        assert!(retry
            .docstring
            .as_ref()
            .unwrap()
            .starts_with("Run thunk, retrying failures"));
    }
    #[test]
    fn nested_fixture_documents_entrypoint_without_following_imports() {
        let document = capture(&archive(&[
            (
                "sema.toml",
                include_str!("../tests/fixtures/nested-api-demo/sema.toml"),
            ),
            (
                "src/api.sema",
                include_str!("../tests/fixtures/nested-api-demo/src/api.sema"),
            ),
            (
                "src/internal/scale.sema",
                include_str!("../tests/fixtures/nested-api-demo/src/internal/scale.sema"),
            ),
            (
                "tests/api.sema",
                include_str!("../tests/fixtures/nested-api-demo/tests/api.sema"),
            ),
        ]));
        assert_eq!(document.status, Status::Partial, "{}", document.message);
        assert_eq!(document.source.unwrap().path, "src/api.sema");
        assert_eq!(document.entries.len(), 3);
        assert_eq!(
            document.entries[0].signature().as_deref(),
            Some("(demo/clamp value minimum maximum)")
        );
        assert_eq!(
            document.entries[1].signature().as_deref(),
            Some("(demo/scale value factor)")
        );
        assert_eq!(document.entries[2].name, "demo/triple");
        assert!(document.entries[2].signature().is_none());
    }
    #[test]
    fn docstrings_need_a_following_body_and_exports_must_be_literal() {
        let document = package("(module test (export f g) (define (f x) \"A docstring\" x) (define (g) \"A return value\"))");
        assert_eq!(
            document.entries[0].docstring.as_deref(),
            Some("A docstring")
        );
        assert_eq!(document.entries[1].docstring, None);
        assert_eq!(
            package("(module test (export (names)) (define (f) 1))").status,
            Status::Unsupported
        );
    }
    #[test]
    fn ambiguous_and_indirect_exports_have_no_guessed_signature() {
        for source in [
            "(module test (export f) (define (f x) x) (define (f x y) y))",
            "(module test (export f) (define (f x) x) (set! f identity))",
            "(module test (export f) (define f identity))",
            "(module test (export f) (if #t (define (f) 1)))",
            "(module test (export f) (define (f) 1) (defun f (x) x))",
            "(module test (export f) (defmacro (f x) x))",
        ] {
            let document = package(source);
            assert_eq!(document.status, Status::Partial, "{source}");
            assert!(document.entries[0].parameters.is_none(), "{source}");
        }
    }
    #[test]
    fn archive_paths_duplicates_and_size_are_bounded() {
        let unsafe_manifest = capture(&archive(&[
            ("sema.toml", "[package]\nentrypoint='../escape.sema'"),
            ("package.sema", "(module a)"),
        ]));
        assert_eq!(unsafe_manifest.status, Status::Unsupported);
        let duplicate = capture(&archive(&[("sema.toml", "a=1"), ("sema.toml", "a=2")]));
        assert!(duplicate.message.contains("duplicate"));
        let oversized = " ".repeat(MAX_SOURCE_BYTES as usize + 1);
        assert_eq!(package(&oversized).status, Status::Unsupported);
        let wrapped = capture(&archive(&[
            ("pkg/sema.toml", "[package]\nentrypoint='src/api.sema'"),
            (
                "pkg/src/api.sema",
                "(module test (export f) (define (f) 1))",
            ),
        ]));
        assert_eq!(wrapped.status, Status::Ready);
        assert_eq!(wrapped.source.unwrap().path, "pkg/src/api.sema");
    }
    #[test]
    fn malformed_and_deep_source_are_diagnostics_not_execution() {
        assert_eq!(package("(module incomplete").status, Status::Unsupported);
        assert_eq!(package(&"(".repeat(500)).status, Status::Unsupported);
        let document =
            package("(module test (export f) (define (f) (shell \"never run package code\")))");
        assert_eq!(document.status, Status::Ready);
    }
}
