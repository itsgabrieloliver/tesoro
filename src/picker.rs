use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub struct Picker {
    pub query: String,
    pub items: Vec<String>,
    pub map: Vec<usize>,
    pub matches: Vec<usize>,
    pub sel: usize,
}

impl Picker {
    pub fn new(items: Vec<String>) -> Self {
        let map = (0..items.len()).collect();
        Self::with_map(items, map)
    }

    pub fn with_map(items: Vec<String>, map: Vec<usize>) -> Self {
        let mut p = Self {
            query: String::new(),
            items,
            map,
            matches: Vec::new(),
            sel: 0,
        };
        p.refilter();
        p
    }

    pub fn refilter(&mut self) {
        if self.query.is_empty() {
            self.matches = (0..self.items.len()).collect();
        } else {
            let pattern = Pattern::parse(&self.query, CaseMatching::Ignore, Normalization::Smart);
            let mut matcher = Matcher::new(Config::DEFAULT);
            let mut buf = Vec::new();
            let mut scored: Vec<(u32, usize)> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    pattern
                        .score(Utf32Str::new(s, &mut buf), &mut matcher)
                        .map(|sc| (sc, i))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            self.matches = scored.into_iter().map(|(_, i)| i).collect();
        }
        if self.sel >= self.matches.len() {
            self.sel = self.matches.len().saturating_sub(1);
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.sel = 0;
        self.refilter();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn up(&mut self) {
        self.sel = self.sel.saturating_sub(1);
    }

    pub fn down(&mut self) {
        if !self.matches.is_empty() {
            self.sel = (self.sel + 1).min(self.matches.len() - 1);
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.matches.get(self.sel).copied()
    }

    pub fn selected_id(&self) -> Option<usize> {
        self.selected().map(|i| self.map[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_filters_and_ranks() {
        let mut p = Picker::new(vec![
            "Welcome".to_string(),
            "sub/Bar".to_string(),
            "Ideas".to_string(),
        ]);
        assert_eq!(p.matches.len(), 3);
        p.push('i');
        p.push('d');
        let top = p.selected().unwrap();
        assert_eq!(p.items[top], "Ideas");
    }

    #[test]
    fn empty_query_lists_all() {
        let mut p = Picker::new(vec!["a".into(), "b".into()]);
        p.push('a');
        p.backspace();
        assert_eq!(p.matches.len(), 2);
    }

    #[test]
    fn maps_item_to_external_id() {
        let p = Picker::with_map(vec!["x".into(), "y".into()], vec![7, 9]);
        assert_eq!(p.selected_id(), Some(7));
    }
}
