// Parse-state checkpoint cache for syntax highlighting.
// Visible windows resume from the nearest snapshot instead of restarting at line 0.
// Allow unused items until Task 4 wires this into App/editor.
#![allow(dead_code)]

use ratatui::text::Span;

use crate::buffer::Language;
use crate::syntax::{HighlightSnapshot, SyntaxHighlighter};

const CHECKPOINT_INTERVAL: usize = 64;

fn overscan_lines(view_count: usize) -> usize {
    view_count.saturating_mul(3).max(64)
}

pub struct HighlightCache {
    start_line: usize,
    spans: Vec<Vec<Span<'static>>>,
    /// `None` after `invalidate` / construction so the next fill cannot match.
    keys: Option<(Language, u64)>,
    /// Snapshots of the state that *starts* highlighting `line`. Always includes 0.
    checkpoints: Vec<(usize, HighlightSnapshot)>,
    /// Snapshot at `start_line + spans.len()`, used to append when scrolling down.
    frontier: Option<(usize, HighlightSnapshot)>,
    /// Always incremented; read via `lines_parsed()` in tests.
    lines_parsed: usize,
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HighlightCache {
    pub fn new() -> Self {
        Self {
            start_line: 0,
            spans: Vec::new(),
            keys: None,
            checkpoints: Vec::new(),
            frontier: None,
            lines_parsed: 0,
        }
    }

    /// Ensure spans exist for `[view_start, view_start + view_count)`.
    /// May highlight extra overscan around that window. Cheap no-op on hit.
    #[allow(clippy::too_many_arguments)] // public API takes highlighter, lines, window, and cache key
    pub fn ensure_window(
        &mut self,
        highlighter: &SyntaxHighlighter,
        line_at: impl Fn(usize) -> String,
        n_lines: usize,
        view_start: usize,
        view_count: usize,
        language: Language,
        generation: u64,
    ) {
        if self.keys != Some((language, generation)) {
            self.invalidate();
        }

        if n_lines == 0 {
            self.spans.clear();
            self.start_line = 0;
            self.frontier = None;
            self.checkpoints.clear();
            self.keys = Some((language, generation));
            return;
        }

        let overscan = overscan_lines(view_count);
        let desired_start = view_start.saturating_sub(overscan);
        let desired_end = n_lines.min(
            view_start
                .saturating_add(view_count)
                .saturating_add(overscan),
        );

        if desired_start >= desired_end {
            self.spans.clear();
            self.start_line = desired_start.min(n_lines);
            self.frontier = None;
            self.keys = Some((language, generation));
            return;
        }

        if self.covers(desired_start, desired_end) {
            return;
        }

        if self.can_extend_down(desired_start, desired_end) {
            self.extend_down(highlighter, &line_at, desired_end, language);
        } else {
            self.fill_from_checkpoint(highlighter, &line_at, desired_start, desired_end, language);
        }

        self.trim_to_desired(desired_start, desired_end);
        self.keys = Some((language, generation));
    }

    pub fn start_line(&self) -> usize {
        self.start_line
    }

    pub fn spans(&self) -> &[Vec<Span<'static>>] {
        &self.spans
    }

    pub fn line_spans(&self, logical_line: usize) -> Option<&[Span<'static>]> {
        logical_line
            .checked_sub(self.start_line)
            .and_then(|idx| self.spans.get(idx).map(|v| v.as_slice()))
    }

    pub fn invalidate(&mut self) {
        self.start_line = 0;
        self.spans.clear();
        self.keys = None;
        self.checkpoints.clear();
        self.frontier = None;
    }

    #[cfg(test)]
    pub fn lines_parsed(&self) -> usize {
        self.lines_parsed
    }

    #[cfg(test)]
    pub fn reset_parse_counter(&mut self) {
        self.lines_parsed = 0;
    }

    fn cache_end(&self) -> usize {
        self.start_line + self.spans.len()
    }

    fn covers(&self, start: usize, end: usize) -> bool {
        end <= start || (self.start_line <= start && self.cache_end() >= end)
    }

    fn can_extend_down(&self, desired_start: usize, desired_end: usize) -> bool {
        match &self.frontier {
            Some((fline, _)) => {
                *fline == self.cache_end()
                    && desired_start >= self.start_line
                    && desired_start <= self.cache_end()
                    && desired_end > self.cache_end()
            }
            None => false,
        }
    }

    fn extend_down(
        &mut self,
        highlighter: &SyntaxHighlighter,
        line_at: &impl Fn(usize) -> String,
        desired_end: usize,
        language: Language,
    ) {
        let cache_end = self.cache_end();
        let snap = match &self.frontier {
            Some((fline, snap)) if *fline == cache_end => snap.clone(),
            _ => return,
        };
        let (new_spans, end_state) = self.walk(
            highlighter,
            line_at,
            cache_end,
            desired_end,
            cache_end,
            snap,
            language,
        );
        self.spans.extend(new_spans);
        self.frontier = Some((desired_end, end_state));
    }

    fn fill_from_checkpoint(
        &mut self,
        highlighter: &SyntaxHighlighter,
        line_at: &impl Fn(usize) -> String,
        desired_start: usize,
        desired_end: usize,
        language: Language,
    ) {
        self.seed_checkpoint_zero(highlighter, language);
        let (cp_line, cp_snap) = self.last_checkpoint_at_or_before(desired_start);
        let (new_spans, end_state) = self.walk(
            highlighter,
            line_at,
            cp_line,
            desired_end,
            desired_start,
            cp_snap,
            language,
        );
        self.spans = new_spans;
        self.start_line = desired_start;
        self.frontier = Some((desired_end, end_state));
    }

    fn seed_checkpoint_zero(&mut self, highlighter: &SyntaxHighlighter, language: Language) {
        match self.checkpoints.first() {
            Some((0, _)) => {}
            _ => {
                self.checkpoints
                    .insert(0, (0, highlighter.initial_snapshot(language)));
            }
        }
    }

    fn last_checkpoint_at_or_before(&self, line: usize) -> (usize, HighlightSnapshot) {
        self.checkpoints
            .iter()
            .rev()
            .find(|(l, _)| *l <= line)
            .map(|(l, snap)| (*l, snap.clone()))
            .expect("checkpoint 0 is always seeded before resume")
    }

    fn store_checkpoint(&mut self, line: usize, snap: HighlightSnapshot) {
        match self.checkpoints.binary_search_by_key(&line, |c| c.0) {
            Ok(i) => self.checkpoints[i].1 = snap,
            Err(i) => self.checkpoints.insert(i, (line, snap)),
        }
    }

    /// Drive syntect from absolute `from` to `to` (exclusive), collecting spans
    /// only for `[span_from, to)`. Walks in 64-line chunks so checkpoints land
    /// on `CHECKPOINT_INTERVAL` boundaries without reconstructing HighlightLines
    /// per line. Indices passed to `highlight_slice_from_state` are 0-based in
    /// the collected chunk, not absolute buffer indexes.
    #[allow(clippy::too_many_arguments)] // from/to/span_from plus highlighter state cannot be collapsed cleanly
    fn walk(
        &mut self,
        highlighter: &SyntaxHighlighter,
        line_at: &impl Fn(usize) -> String,
        from: usize,
        to: usize,
        span_from: usize,
        mut state: HighlightSnapshot,
        language: Language,
    ) -> (Vec<Vec<Span<'static>>>, HighlightSnapshot) {
        let mut out = Vec::new();
        let mut line = from;
        while line < to {
            let next_boundary = line
                .saturating_div(CHECKPOINT_INTERVAL)
                .saturating_add(1)
                .saturating_mul(CHECKPOINT_INTERVAL);
            let chunk_end = next_boundary.min(to);
            let collected: Vec<String> = (line..chunk_end).map(line_at).collect();
            let refs: Vec<&str> = collected.iter().map(String::as_str).collect();
            let rel_span_start = if span_from <= line {
                0
            } else if span_from >= chunk_end {
                refs.len()
            } else {
                span_from - line
            };
            let (spans, end_state) = highlighter.highlight_slice_from_state(
                &refs,
                rel_span_start,
                0,
                refs.len(),
                language,
                state,
            );
            self.lines_parsed += refs.len();
            out.extend(spans);
            state = end_state;
            if chunk_end > 0 && chunk_end.is_multiple_of(CHECKPOINT_INTERVAL) {
                self.store_checkpoint(chunk_end, state.clone());
            }
            line = chunk_end;
        }
        (out, state)
    }

    fn trim_to_desired(&mut self, desired_start: usize, desired_end: usize) {
        if desired_start > self.start_line && !self.spans.is_empty() {
            let drop = (desired_start - self.start_line).min(self.spans.len());
            self.spans.drain(..drop);
            self.start_line += drop;
        }
        let cache_end = self.cache_end();
        if cache_end > desired_end {
            self.spans
                .truncate(desired_end.saturating_sub(self.start_line));
            if self
                .frontier
                .as_ref()
                .is_none_or(|(fline, _)| *fline != desired_end)
            {
                self.frontier = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Language;
    use crate::syntax::SyntaxHighlighter;

    fn rust_lines() -> Vec<String> {
        let mut v = vec![
            "fn before() {}".to_string(),
            "/* opener".to_string(),
            "still comment".to_string(),
            "end */".to_string(),
            "fn after() {}".to_string(),
        ];
        v.extend((0..200).map(|i| format!("let x{i} = {i};")));
        v
    }

    fn line_at<'a>(lines: &'a [String]) -> impl Fn(usize) -> String + 'a {
        |i| lines.get(i).cloned().unwrap_or_default()
    }

    #[test]
    fn cache_hit_on_identical_window_parses_zero_lines() {
        let sh = SyntaxHighlighter::new();
        let lines = rust_lines();
        let n = lines.len();
        let mut cache = HighlightCache::new();
        cache.ensure_window(&sh, line_at(&lines), n, 0, 20, Language::Rust, 1);
        assert_eq!(cache.start_line(), 0);
        assert!(!cache.spans().is_empty());
        cache.reset_parse_counter();
        cache.ensure_window(&sh, line_at(&lines), n, 0, 20, Language::Rust, 1);
        assert_eq!(cache.lines_parsed(), 0);
    }

    #[test]
    fn scroll_down_one_line_does_not_reparse_the_viewport() {
        let sh = SyntaxHighlighter::new();
        let lines = rust_lines();
        let n = lines.len();
        let mut cache = HighlightCache::new();
        cache.ensure_window(&sh, line_at(&lines), n, 0, 20, Language::Rust, 1);
        cache.reset_parse_counter();
        cache.ensure_window(&sh, line_at(&lines), n, 1, 20, Language::Rust, 1);
        eprintln!("scroll_down_one_line lines_parsed={}", cache.lines_parsed());
        assert!(
            cache.lines_parsed() < 20,
            "scrolled one line but parsed {} lines",
            cache.lines_parsed()
        );
        assert!(cache.line_spans(1).is_some());
        assert!(cache.line_spans(20).is_some());
    }

    #[test]
    fn mid_file_spans_match_full_parse() {
        let sh = SyntaxHighlighter::new();
        let lines = rust_lines();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let full = sh.highlight(&refs.join("\n"), Language::Rust);
        let mut cache = HighlightCache::new();
        cache.ensure_window(&sh, line_at(&lines), lines.len(), 2, 2, Language::Rust, 1);
        let cached = cache.line_spans(2).expect("line 2 cached");
        assert_eq!(
            cached.first().and_then(|s| s.style.fg),
            full[2].first().and_then(|s| s.style.fg)
        );
    }

    #[test]
    fn generation_change_invalidates() {
        let sh = SyntaxHighlighter::new();
        let lines = rust_lines();
        let n = lines.len();
        let mut cache = HighlightCache::new();
        cache.ensure_window(&sh, line_at(&lines), n, 0, 10, Language::Rust, 1);
        cache.reset_parse_counter();
        cache.ensure_window(&sh, line_at(&lines), n, 0, 10, Language::Rust, 2);
        assert!(cache.lines_parsed() > 0);
    }

    #[test]
    fn jump_past_checkpoint_matches_full_parse() {
        let sh = SyntaxHighlighter::new();
        let lines = rust_lines();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let full = sh.highlight(&refs.join("\n"), Language::Rust);
        let mut cache = HighlightCache::new();
        cache.ensure_window(&sh, line_at(&lines), lines.len(), 0, 20, Language::Rust, 1);
        cache.reset_parse_counter();
        // After the first fill, overscan covers [0, 84). view_start=150 yields
        // desired_start=86, past cache_end, so we must resume from a checkpoint.
        cache.ensure_window(
            &sh,
            line_at(&lines),
            lines.len(),
            150,
            20,
            Language::Rust,
            1,
        );
        let cached = cache.line_spans(150).expect("line 150 cached");
        assert_eq!(
            cached.first().and_then(|s| s.style.fg),
            full[150].first().and_then(|s| s.style.fg)
        );
        assert!(cache.lines_parsed() > 0);
        assert!(
            cache.lines_parsed() < lines.len(),
            "checkpoint resume parsed {} of {} lines",
            cache.lines_parsed(),
            lines.len()
        );
    }
}
