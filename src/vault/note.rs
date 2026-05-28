use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::frontmatter;

pub struct NoteMeta {
    pub path: PathBuf,
    pub rel: PathBuf,
    pub title: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub body: String,
}

impl NoteMeta {
    pub fn parse(path: &Path, root: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let (front, body) = frontmatter::split(&raw);
        let aliases = front
            .as_ref()
            .map(|v| frontmatter::string_or_list(v, "aliases"))
            .unwrap_or_default();

        let mut tagset: BTreeSet<String> = BTreeSet::new();
        if let Some(v) = &front {
            for t in frontmatter::string_or_list(v, "tags") {
                tagset.insert(t);
            }
        }
        for t in crate::markdown::tag_names(&body) {
            tagset.insert(t);
        }
        let tags: Vec<String> = tagset.into_iter().collect();

        let links = crate::markdown::wikilink_targets(&body);
        let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        Ok(Self {
            path: path.to_path_buf(),
            rel,
            title,
            aliases,
            tags,
            links,
            body,
        })
    }
}
