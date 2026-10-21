use crate::{Error, Result};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    path::Path,
};

/// Hard preflight bounds, applied before XML, CSS, raster decoding, or rbook evaluation.
#[derive(Clone, Debug)]
pub struct Limits {
    pub archive_bytes: u64,
    pub entries: usize,
    pub entry_bytes: u64,
    pub expanded_bytes: u64,
    pub xml_nodes: u32,
    pub xml_depth: usize,
    pub image_pixels: u64,
    pub text_scalars: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            archive_bytes: 64 * 1024 * 1024,
            entries: 4096,
            entry_bytes: 16 * 1024 * 1024,
            expanded_bytes: 128 * 1024 * 1024,
            xml_nodes: 500_000,
            xml_depth: 128,
            image_pixels: 40_000_000,
            text_scalars: 4_000_000,
        }
    }
}

pub(crate) struct Archive {
    pub bytes: Vec<u8>,
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Archive {
    pub fn open(path: &Path, limits: &Limits) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > limits.archive_bytes {
            return Err(Error::Limit("compressed archive bytes"));
        }
        let mut bytes = Vec::new();
        file.take(limits.archive_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limits.archive_bytes {
            return Err(Error::Limit("compressed archive bytes"));
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(&bytes))?;
        if zip.len() > limits.entries {
            return Err(Error::Limit("ZIP entries"));
        }
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_owned();
            if name.starts_with('/')
                || name.contains('\\')
                || name.split('/').any(|s| matches!(s, ".." | "." | ""))
            {
                return Err(Error::Invalid("unsafe archive resource path".into()));
            }
            if files.contains_key(&name) {
                return Err(Error::Invalid("duplicate ZIP resource".into()));
            }
            if entry.size() > limits.entry_bytes {
                return Err(Error::Limit("expanded entry bytes"));
            }
            let declared_size = entry.size();
            let remaining = limits.expanded_bytes.saturating_sub(total);
            if declared_size > remaining {
                return Err(Error::Limit("expanded archive bytes"));
            }
            let allowed = limits.entry_bytes.min(remaining);
            let mut data = Vec::new();
            entry
                .take(allowed.saturating_add(1))
                .read_to_end(&mut data)?;
            if data.len() as u64 > allowed {
                return Err(Error::Limit("actual expanded bytes"));
            }
            if data.len() as u64 != declared_size {
                return Err(Error::Invalid(
                    "ZIP size metadata disagrees with expanded data".into(),
                ));
            }
            total = total
                .checked_add(data.len() as u64)
                .ok_or(Error::Limit("expanded archive bytes"))?;
            files.insert(name, data);
        }
        if files.get("mimetype").map(Vec::as_slice) != Some(b"application/epub+zip".as_slice()) {
            return Err(Error::Invalid("missing or incorrect EPUB mimetype".into()));
        }
        Ok(Self { bytes, files })
    }
    pub fn text(&self, path: &str) -> Result<&str> {
        let data = self
            .files
            .get(path)
            .ok_or_else(|| Error::Invalid(format!("missing publication resource: {path}")))?;
        std::str::from_utf8(data).map_err(|_| Error::Invalid("only UTF-8 XML is supported".into()))
    }
}

pub(crate) fn xml<'a>(
    text: &'a str,
    path: &str,
    limits: &Limits,
) -> Result<roxmltree::Document<'a>> {
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: limits.xml_nodes,
            ..Default::default()
        },
    )
    .map_err(|error| Error::Invalid(format!("malformed XML/HTML in {path}: {error}")))?;
    if doc.descendants().any(|n| {
        n.ancestors()
            .take(limits.xml_depth.saturating_add(2))
            .count()
            > limits.xml_depth.saturating_add(1)
    }) {
        return Err(Error::Limit("XML depth"));
    }
    Ok(doc)
}

/// Resolve local publication links. Reject external schemes and escaping the archive root.
pub(crate) fn resolve(base: &str, href: &str) -> Result<(String, Option<String>)> {
    if href.contains(':') || href.starts_with('/') || href.contains('\\') || href.contains('?') {
        return Err(Error::Invalid(
            "non-local or unsupported resource URL".into(),
        ));
    }
    let (path, fragment) = href
        .split_once('#')
        .map_or((href, None), |(p, f)| (p, Some(f)));
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .map_err(|_| Error::Invalid("invalid URL encoding".into()))?;
    let fragment = fragment
        .map(|s| {
            percent_encoding::percent_decode_str(s)
                .decode_utf8()
                .map(|v| v.into_owned())
        })
        .transpose()
        .map_err(|_| Error::Invalid("invalid fragment encoding".into()))?;
    if path.is_empty() {
        return Ok((base.to_owned(), fragment));
    }
    if path.contains(':') || path.starts_with('/') || path.contains('\\') {
        return Err(Error::Invalid("unsafe resource URL".into()));
    }
    let mut parts: Vec<&str> = base
        .rsplit_once('/')
        .map_or(Vec::new(), |(dir, _)| dir.split('/').collect());
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(Error::Invalid("resource escapes archive root".into()));
                }
            }
            _ => parts.push(part),
        }
    }
    Ok((parts.join("/"), fragment))
}

pub(crate) fn node_path(node: roxmltree::Node<'_, '_>) -> String {
    let mut parts = Vec::new();
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        parts.push(cur.prev_siblings().skip(1).count().to_string());
        cur = parent;
    }
    parts.reverse();
    parts.join("/")
}
