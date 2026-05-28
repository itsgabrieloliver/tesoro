mod frontmatter;
mod note;

pub use note::NoteMeta;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::WalkBuilder;

pub struct Vault {
    pub root: PathBuf,
    pub notes: Vec<NoteMeta>,
    by_basename: HashMap<String, Vec<usize>>,
    by_rel: HashMap<String, usize>,
    by_alias: HashMap<String, Vec<usize>>,
    by_tag: HashMap<String, Vec<usize>>,
    backlinks: HashMap<usize, Vec<usize>>,
}

impl Vault {
    pub fn load(root: PathBuf) -> Result<Self> {
        let mut notes = Vec::new();
        for result in WalkBuilder::new(&root).hidden(true).build() {
            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if !is_md {
                continue;
            }
            if let Ok(meta) = NoteMeta::parse(path, &root) {
                notes.push(meta);
            }
        }
        notes.sort_by(|a, b| a.rel.cmp(&b.rel));

        let mut vault = Self {
            root,
            notes,
            by_basename: HashMap::new(),
            by_rel: HashMap::new(),
            by_alias: HashMap::new(),
            by_tag: HashMap::new(),
            backlinks: HashMap::new(),
        };
        vault.reindex_maps();
        Ok(vault)
    }

    fn reindex_maps(&mut self) {
        self.by_basename.clear();
        self.by_rel.clear();
        self.by_alias.clear();
        self.by_tag.clear();
        for (i, note) in self.notes.iter().enumerate() {
            self.by_basename
                .entry(note.title.to_lowercase())
                .or_default()
                .push(i);
            self.by_rel.insert(rel_key(&note.rel), i);
            for alias in &note.aliases {
                self.by_alias
                    .entry(alias.to_lowercase())
                    .or_default()
                    .push(i);
            }
            for tag in &note.tags {
                self.by_tag.entry(tag.clone()).or_default().push(i);
            }
        }

        let mut backlinks: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..self.notes.len() {
            let targets = self.notes[i].links.clone();
            for target in targets {
                if let Some(j) = self.resolve(&target)
                    && j != i
                {
                    backlinks.entry(j).or_default().push(i);
                }
            }
        }
        for v in backlinks.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        self.backlinks = backlinks;
    }

    pub fn resolve(&self, target: &str) -> Option<usize> {
        let normalized = target.trim().replace('\\', "/");
        let normalized = normalized.strip_suffix(".md").unwrap_or(&normalized);
        let key = normalized.to_lowercase();

        if normalized.contains('/')
            && let Some(&i) = self.by_rel.get(&key)
        {
            return Some(i);
        }
        if let Some(matches) = self.by_basename.get(&key) {
            return Some(self.pick_shortest(matches));
        }
        if let Some(matches) = self.by_alias.get(&key) {
            return Some(self.pick_shortest(matches));
        }
        self.by_rel.get(&key).copied()
    }

    fn pick_shortest(&self, idxs: &[usize]) -> usize {
        idxs.iter()
            .copied()
            .min_by_key(|&i| (self.notes[i].rel.components().count(), self.notes[i].rel.clone()))
            .unwrap()
    }

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.notes.iter().position(|n| n.path.as_path() == path)
    }

    pub fn reload(&mut self, path: &Path) {
        let Ok(meta) = NoteMeta::parse(path, &self.root) else {
            return;
        };
        if let Some(i) = self.notes.iter().position(|n| n.path.as_path() == path) {
            self.notes[i] = meta;
        } else {
            self.notes.push(meta);
            self.notes.sort_by(|a, b| a.rel.cmp(&b.rel));
        }
        self.reindex_maps();
    }

    pub fn remove(&mut self, path: &Path) {
        if let Some(i) = self.notes.iter().position(|n| n.path.as_path() == path) {
            self.notes.remove(i);
            self.reindex_maps();
        }
    }

    pub fn backlinks(&self, idx: usize) -> &[usize] {
        self.backlinks.get(&idx).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn outbound(&self, idx: usize) -> Vec<usize> {
        let mut out = Vec::new();
        if let Some(note) = self.notes.get(idx) {
            for target in &note.links {
                if let Some(j) = self.resolve(target)
                    && j != idx
                    && !out.contains(&j)
                {
                    out.push(j);
                }
            }
        }
        out
    }

    pub fn tags(&self) -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = self
            .by_tag
            .iter()
            .map(|(k, ids)| (k.clone(), ids.len()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    pub fn notes_with_tag(&self, tag: &str) -> &[usize] {
        self.by_tag.get(tag).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn search(&self, query: &str) -> Vec<(usize, String)> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let needle = query.to_lowercase();
        let mut hits = Vec::new();
        for (i, note) in self.notes.iter().enumerate() {
            if let Some(snippet) = first_match_line(&note.body, &needle) {
                hits.push((i, snippet));
            }
        }
        hits
    }

    pub fn most_connected(&self) -> Vec<usize> {
        let mut scored: Vec<(usize, usize, usize)> = (0..self.notes.len())
            .map(|i| (self.backlinks(i).len(), self.outbound(i).len(), i))
            .collect();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(b.1.cmp(&a.1))
                .then_with(|| self.notes[a.2].rel.cmp(&self.notes[b.2].rel))
        });
        scored.into_iter().map(|(_, _, i)| i).collect()
    }
}

fn rel_key(rel: &Path) -> String {
    let s = rel.to_string_lossy().replace('\\', "/");
    s.strip_suffix(".md").unwrap_or(&s).to_lowercase()
}

fn first_match_line(body: &str, needle_lower: &str) -> Option<String> {
    for line in body.lines() {
        if line.to_lowercase().contains(needle_lower) {
            return Some(line.trim().chars().take(100).collect());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(rel: &str, aliases: &[&str]) -> NoteMeta {
        let rel = PathBuf::from(rel);
        let title = rel.file_stem().unwrap().to_str().unwrap().to_string();
        NoteMeta {
            path: rel.clone(),
            rel: rel.clone(),
            title,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            tags: Vec::new(),
            links: Vec::new(),
            body: String::new(),
        }
    }

    fn vault(notes: Vec<NoteMeta>) -> Vault {
        let mut v = Vault {
            root: PathBuf::from("/"),
            notes,
            by_basename: HashMap::new(),
            by_rel: HashMap::new(),
            by_alias: HashMap::new(),
            by_tag: HashMap::new(),
            backlinks: HashMap::new(),
        };
        v.reindex_maps();
        v
    }

    #[test]
    fn resolves_basename_case_insensitively() {
        let v = vault(vec![note("Foo.md", &[]), note("sub/Bar.md", &["bar-alias"])]);
        assert_eq!(v.resolve("foo"), Some(0));
        assert_eq!(v.resolve("Foo"), Some(0));
        assert_eq!(v.resolve("bar"), Some(1));
        assert_eq!(v.resolve("BAR-ALIAS"), Some(1));
        assert_eq!(v.resolve("sub/Bar"), Some(1));
        assert_eq!(v.resolve("sub/Bar.md"), Some(1));
        assert_eq!(v.resolve("missing"), None);
    }

    #[test]
    fn resolves_shortest_path_when_ambiguous() {
        let v = vault(vec![note("a/b/Note.md", &[]), note("Note.md", &[])]);
        assert_eq!(v.resolve("Note"), Some(1));
    }

    #[test]
    fn load_walks_vault_parses_frontmatter_and_resolves() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("Alpha.md"),
            "---\naliases: [A1]\ntags: [meta]\n---\n# Alpha\nlinks to [[Beta]] #inline\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/Beta.md"), "# Beta\n").unwrap();
        fs::write(root.join("notes.txt"), "ignored, not markdown\n").unwrap();

        let v = Vault::load(root.to_path_buf()).unwrap();
        assert_eq!(v.notes.len(), 2);
        assert!(v.resolve("Beta").is_some());
        assert_eq!(v.resolve("Beta"), v.resolve("sub/Beta"));
        assert!(v.resolve("A1").is_some());
        assert!(v.resolve("missing").is_none());
    }

    #[test]
    fn reload_updates_note_metadata() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("N.md");
        fs::write(&p, "# N\n").unwrap();
        let mut v = Vault::load(dir.path().to_path_buf()).unwrap();
        assert!(v.resolve("alias-x").is_none());

        fs::write(&p, "---\naliases: [alias-x]\n---\n# N\n").unwrap();
        v.reload(&p);
        assert!(v.resolve("alias-x").is_some());
        assert_eq!(v.index_of(&p), Some(0));
    }

    #[test]
    fn builds_backlinks_from_outbound_links() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("A.md"), "# A\nsee [[B]]\n").unwrap();
        fs::write(dir.path().join("B.md"), "# B\n").unwrap();
        fs::write(dir.path().join("C.md"), "# C\nlinks [[B]] too\n").unwrap();
        let v = Vault::load(dir.path().to_path_buf()).unwrap();
        let b = v.resolve("B").unwrap();
        assert_eq!(v.backlinks(b).len(), 2);
    }

    #[test]
    fn search_matches_content_case_insensitively() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("A.md"), "# A\nthe quick brown fox\n").unwrap();
        fs::write(dir.path().join("B.md"), "# B\nnothing here\n").unwrap();
        let v = Vault::load(dir.path().to_path_buf()).unwrap();
        let hits = v.search("BROWN");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1.to_lowercase().contains("brown"));
    }

    #[test]
    fn collects_tags_from_frontmatter_and_inline() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("A.md"),
            "---\ntags: [project]\n---\n# A\nbody with #inline tag\n",
        )
        .unwrap();
        fs::write(dir.path().join("B.md"), "# B\n#project again\n").unwrap();
        let v = Vault::load(dir.path().to_path_buf()).unwrap();
        assert_eq!(v.notes_with_tag("project").len(), 2);
        assert_eq!(v.notes_with_tag("inline").len(), 1);
        assert!(v.tags().iter().any(|(t, _)| t == "project"));
    }
}
