/// Buffered text replacements applied to source.
///
/// Edits are non-overlapping byte ranges. They're applied in reverse order
/// so earlier byte offsets stay valid while later edits are written.
#[derive(Debug, Default)]
pub struct EditSet {
    edits: Vec<Edit>,
}

#[derive(Debug, Clone)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

impl EditSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, start: usize, end: usize, replacement: String) {
        self.edits.push(Edit {
            start,
            end,
            replacement,
        });
    }

    pub fn apply(mut self, source: &str) -> String {
        if self.edits.is_empty() {
            return source.to_string();
        }
        // Sort by start ascending; drop any edit fully covered by an earlier one.
        self.edits.sort_by_key(|e| (e.start, e.end));
        let mut filtered: Vec<Edit> = Vec::with_capacity(self.edits.len());
        for e in self.edits {
            match filtered.last() {
                Some(prev) if prev.end > e.start => {
                    // Overlap: keep the earlier (broader) edit; drop later.
                }
                _ => filtered.push(e),
            }
        }
        let mut out = String::with_capacity(source.len());
        let mut cursor = 0usize;
        for e in filtered {
            out.push_str(&source[cursor..e.start]);
            out.push_str(&e.replacement);
            cursor = e.end;
        }
        out.push_str(&source[cursor..]);
        out
    }
}
