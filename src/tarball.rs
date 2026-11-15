//! Reading facts out of an uploaded package tarball (gzip'd tar).

use std::io::Read;

/// Largest README we'll store — a guard against a pathological upload.
const MAX_README_BYTES: usize = 512 * 1024;

/// Documentation files captured from an uploaded release.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReleaseDocs {
    pub readme: Option<String>,
    pub changelog: Option<String>,
}

/// Extract release documentation from a `.tar.gz`. It prefers shallow files so
/// the package-root README and changelog win over nested copies. Best-effort:
/// malformed archives simply produce no documentation.
pub fn extract_release_docs(tarball: &[u8]) -> ReleaseDocs {
    let gz = flate2::read::GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(gz);
    let Ok(entries) = archive.entries() else {
        return ReleaseDocs::default();
    };
    let mut readme: Option<(usize, String)> = None;
    let mut changelog: Option<(usize, String)> = None;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let Ok(path) = entry.path() else { continue };
        let path = path.into_owned();
        let Some(name) = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
        else {
            continue;
        };
        let target = if matches!(name.as_str(), "readme.md" | "readme" | "readme.markdown") {
            &mut readme
        } else if matches!(
            name.as_str(),
            "changelog.md" | "changelog" | "changelog.markdown"
        ) {
            &mut changelog
        } else {
            continue;
        };
        let depth = path.components().count();
        if target.as_ref().is_some_and(|(d, _)| *d <= depth) {
            continue;
        }
        let mut buf = Vec::new();
        if entry
            .take((MAX_README_BYTES + 1) as u64)
            .read_to_end(&mut buf)
            .is_ok()
            && buf.len() <= MAX_README_BYTES
        {
            *target = Some((depth, String::from_utf8_lossy(&buf).into_owned()));
        }
    }
    ReleaseDocs {
        readme: readme.map(|(_, content)| content),
        changelog: changelog.map(|(_, content)| content),
    }
}

/// Compatibility helper for callers that only need a README.
pub fn extract_readme(tarball: &[u8]) -> Option<String> {
    extract_release_docs(tarball).readme
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    fn make_targz(files: &[(&str, &str)]) -> Vec<u8> {
        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = tar::Builder::new(enc);
        for (name, content) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, content.as_bytes())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn extracts_root_readme() {
        let tgz = make_targz(&[
            ("package.sema", "(module x)"),
            ("README.md", "# Hello\n\nworld"),
        ]);
        assert_eq!(extract_readme(&tgz).as_deref(), Some("# Hello\n\nworld"));
    }

    #[test]
    fn prefers_shallowest_readme() {
        let tgz = make_targz(&[("examples/README.md", "nested"), ("README.md", "root")]);
        assert_eq!(extract_readme(&tgz).as_deref(), Some("root"));
    }

    #[test]
    fn none_when_absent() {
        let tgz = make_targz(&[("package.sema", "(module x)")]);
        assert_eq!(extract_readme(&tgz), None);
    }

    #[test]
    fn extracts_root_changelog() {
        let tgz = make_targz(&[
            ("docs/CHANGELOG.md", "nested"),
            ("CHANGELOG.md", "# 1.0.0\n\nInitial release"),
        ]);
        assert_eq!(
            extract_release_docs(&tgz).changelog.as_deref(),
            Some("# 1.0.0\n\nInitial release")
        );
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(extract_readme(b"not a gzip stream"), None);
    }
}
