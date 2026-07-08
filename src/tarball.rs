//! Reading facts out of an uploaded package tarball (gzip'd tar).

use std::io::Read;

/// Largest README we'll store — a guard against a pathological upload.
const MAX_README_BYTES: usize = 512 * 1024;

/// Extract a package's README from its `.tar.gz`, if present. Prefers the
/// shallowest `README(.md|.markdown)` (case-insensitive), so the package-root
/// README wins over any nested one. Returns the raw Markdown; rendering to HTML
/// is the caller's job. Best-effort: any decode error yields `None`.
pub fn extract_readme(tarball: &[u8]) -> Option<String> {
    let gz = flate2::read::GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(gz);
    let mut best: Option<(usize, String)> = None;
    for entry in archive.entries().ok()? {
        let Ok(mut entry) = entry else { continue };
        let Ok(path) = entry.path() else { continue };
        let path = path.into_owned();
        let Some(name) = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
        else {
            continue;
        };
        if name != "readme.md" && name != "readme" && name != "readme.markdown" {
            continue;
        }
        let depth = path.components().count();
        if best.as_ref().is_some_and(|(d, _)| *d <= depth) {
            continue;
        }
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_ok() && buf.len() <= MAX_README_BYTES {
            best = Some((depth, String::from_utf8_lossy(&buf).into_owned()));
        }
    }
    best.map(|(_, content)| content)
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
    fn garbage_is_none() {
        assert_eq!(extract_readme(b"not a gzip stream"), None);
    }
}
