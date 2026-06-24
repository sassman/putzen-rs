//! `/`-style substring filter applied to the left list.

/// Less/vim-style `/` filter applied to the left list. When `Modal::FilterEdit`
/// is active the input strip is open and printable keys are routed to it; when
/// not, the filter is "applied" — rows still hidden, but the strip shows
/// the static badge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub input: String,
}

impl Filter {
    /// True when the row's absolute path contains the (case-insensitive)
    /// filter substring. Empty input matches everything.
    pub fn is_visible(&self, path: &std::path::Path) -> bool {
        if self.input.is_empty() {
            return true;
        }
        let needle = self.input.to_lowercase();
        let hay = path.to_string_lossy().to_lowercase();
        hay.contains(&needle)
    }
}
