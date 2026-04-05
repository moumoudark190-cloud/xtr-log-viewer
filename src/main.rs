#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::{egui, App, Frame, NativeOptions};
use egui::{
    Align, Align2, Color32, FontId, Key, Layout, Rounding, ScrollArea, Sense, Stroke, Vec2,
    RichText, Button, Window, TextEdit, Checkbox, Separator, Frame as EguiFrame, Margin,
};
use std::path::PathBuf;

// ─── Configuration & Theme ───────────────────────────────────────────────────
struct Theme {
    bg_main: Color32,
    bg_panel: Color32,
    bg_header: Color32,
    bg_input: Color32,
    border: Color32,
    border_active: Color32,
    text_main: Color32,
    text_muted: Color32,
    accent: Color32,
    danger: Color32,
    warning: Color32,
    success: Color32,
    info: Color32,
}

impl Theme {
    fn dark_modern() -> Self {
        Self {
            bg_main: Color32::from_rgb(20, 22, 28),
            bg_panel: Color32::from_rgb(26, 28, 36),
            bg_header: Color32::from_rgb(32, 34, 44),
            bg_input: Color32::from_rgb(18, 20, 26),
            border: Color32::from_rgb(45, 48, 58),
            border_active: Color32::from_rgb(90, 120, 200),
            text_main: Color32::from_rgb(220, 225, 235),
            text_muted: Color32::from_rgb(130, 135, 150),
            accent: Color32::from_rgb(88, 166, 255),
            danger: Color32::from_rgb(235, 85, 85),
            warning: Color32::from_rgb(230, 190, 70),
            success: Color32::from_rgb(80, 205, 105),
            info: Color32::from_rgb(100, 180, 255),
        }
    }

    fn apply_visuals(&self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = self.bg_panel;
        visuals.window_fill = self.bg_main;
        visuals.override_text_color = Some(self.text_main);
        visuals.widgets.inactive.bg_fill = self.bg_input;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.border);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 42, 52);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, self.border_active);
        visuals.widgets.active.bg_fill = Color32::from_rgb(45, 48, 60);
        visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(88, 166, 255, 80);
        visuals.hyperlink_color = self.accent;
        visuals.extreme_bg_color = self.bg_main;
        visuals.popup_shadow = egui::epaint::Shadow { 
            offset: Vec2::new(0.0, 4.0), 
            blur: 20.0, 
            spread: 0.0,
            color: Color32::BLACK 
        };
        ctx.set_visuals(visuals);
    }
}

// ── Data Structures ────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Level { Error, Warning, Info, Debug, Trace }
impl Level {
    fn from_str(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "ERR" | "ERROR" => Self::Error,
            "WRN" | "WARN" | "WARNING" => Self::Warning,
            "INF" | "INFO" => Self::Info,
            "DBG" | "DEBUG" => Self::Debug,
            "TRC" | "TRACE" | "VERBOSE" => Self::Trace,
            _ => Self::Debug,
        }
    }
    fn label(self) -> &'static str {
        match self { Self::Error => "ERR", Self::Warning => "WRN", Self::Info => "INF", Self::Debug => "DBG", Self::Trace => "TRC" }
    }
    fn color(&self, t: &Theme) -> Color32 {
        match self { Self::Error => t.danger, Self::Warning => t.warning, Self::Info => t.success, Self::Debug => t.info, Self::Trace => t.text_muted }
    }
    fn row_bg(&self, _t: &Theme) -> Option<Color32> {
        match self {
            Self::Error => Some(Color32::from_rgba_unmultiplied(235, 85, 85, 15)),
            Self::Warning => Some(Color32::from_rgba_unmultiplied(230, 190, 70, 10)),
            _ => None,
        }
    }
    fn index(self) -> usize {
        match self { Self::Error => 0, Self::Warning => 1, Self::Info => 2, Self::Debug => 3, Self::Trace => 4 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode { Normal, Extended, Regex }
impl Default for SearchMode { fn default() -> Self { Self::Normal } }

#[derive(Debug, Clone)]
struct SearchMatch {
    row_idx: usize, line_idx: usize, line_num: usize, start_col: usize, end_col: usize,
    match_text: String, context_before: String, context_after: String, module: String, level: Level,
}

#[derive(Debug, Clone, Default)]
struct SearchState {
    find_what: String, match_case: bool, whole_word: bool, wrap_around: bool, backward: bool,
    mode: SearchMode, matches: Vec<SearchMatch>, current_match_idx: usize,
    results_panel_open: bool, results_panel_height: f32, first_search: bool,
}
impl SearchState {
    fn new() -> Self { Self { wrap_around: true, results_panel_height: 200.0, first_search: true, ..Default::default() } }
    fn expand_escapes(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('0') => result.push('\0'),
                    Some('\\') => result.push('\\'),
                    Some('x') => {
                        let mut hex = String::new();
                        for _ in 0..2 { if let Some(&c) = chars.peek() { if c.is_ascii_hexdigit() { hex.push(chars.next().unwrap()); } } }
                        if let Ok(val) = u8::from_str_radix(&hex, 16) { result.push(val as char); } else { result.push_str("\\x"); result.push_str(&hex); }
                    }
                    Some(other) => { result.push('\\'); result.push(other); }
                    None => result.push('\\'),
                }
            } else { result.push(c); }
        }
        result
    }
    fn matches_whole_word(&self, hay: &str, needle: &str) -> bool {
        let mut start = 0;
        while let Some(pos) = hay[start..].find(needle) {
            let abs = start + pos;
            let end = abs + needle.len();
            let left_ok = abs == 0 || !hay.as_bytes().get(abs.saturating_sub(1)).copied().map(|b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false);
            let right_ok = end >= hay.len() || !hay.as_bytes().get(end).copied().map(|b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false);
            if left_ok && right_ok { return true; }
            start = abs + 1;
        }
        false
    }
    fn find_all(&mut self, filtered: &[usize], all_lines: &[LogLine]) {
        self.matches.clear();
        if self.find_what.is_empty() { return; }
        let search_text = match self.mode { SearchMode::Extended => self.expand_escapes(&self.find_what), _ => self.find_what.clone() };
        let needle = if self.match_case { search_text.clone() } else { search_text.to_lowercase() };
        for (row_idx, &line_idx) in filtered.iter().enumerate() {
            let Some(line) = all_lines.get(line_idx) else { continue; };
            let hay = if self.match_case { line.raw.clone() } else { line.raw.to_lowercase() };
            let mut start = 0;
            while let Some(pos) = hay[start..].find(&needle) {
                let abs_pos = start + pos;
                let match_end = abs_pos + needle.len();
                if self.whole_word && !self.matches_whole_word(&hay, &needle) { start = abs_pos + 1; continue; }
                let before_start = abs_pos.saturating_sub(30);
                let after_end = (match_end + 30).min(line.raw.len());
                self.matches.push(SearchMatch {
                    row_idx, line_idx, line_num: line.num, start_col: abs_pos, end_col: match_end,
                    match_text: line.raw[abs_pos..match_end].to_string(),
                    context_before: if before_start < abs_pos { line.raw[before_start..abs_pos].to_string() } else { String::new() },
                    context_after: if match_end < after_end { line.raw[match_end..after_end].to_string() } else { String::new() },
                    module: line.module.clone(), level: line.level,
                });
                start = abs_pos + 1;
            }
        }
        if self.current_match_idx >= self.matches.len() { self.current_match_idx = 0; }
    }
    fn next(&mut self) -> Option<usize> {
        if self.matches.is_empty() { return None; }
        if self.first_search { self.first_search = false; self.current_match_idx = if self.backward { self.matches.len() - 1 } else { 0 }; }
        else if self.backward { if self.current_match_idx == 0 { if self.wrap_around { self.current_match_idx = self.matches.len() - 1; } } else { self.current_match_idx -= 1; } }
        else { self.current_match_idx += 1; if self.current_match_idx >= self.matches.len() { self.current_match_idx = if self.wrap_around { 0 } else { self.matches.len() - 1 }; } }
        Some(self.matches[self.current_match_idx].row_idx)
    }
    fn prev(&mut self) -> Option<usize> {
        if self.matches.is_empty() { return None; }
        if self.current_match_idx == 0 { if self.wrap_around { self.current_match_idx = self.matches.len() - 1; } } else { self.current_match_idx -= 1; }
        Some(self.matches[self.current_match_idx].row_idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavKind { Error, Warning, TestStart, TestEnd, Step, Teardown, Custom, Bookmark }
impl NavKind {
    fn color(&self, t: &Theme) -> Color32 {
        match self {
            NavKind::Error => t.danger, NavKind::Warning => t.warning, NavKind::TestStart => t.success,
            NavKind::TestEnd => t.info, NavKind::Step => Color32::from_rgb(140, 140, 220),
            NavKind::Teardown => Color32::from_rgb(180, 150, 230), NavKind::Custom => t.warning,
            NavKind::Bookmark => Color32::from_rgb(255, 140, 200),
        }
    }
    fn short_label(&self) -> &'static str {
        match self {
            NavKind::Error => "ERR", NavKind::Warning => "WRN", NavKind::TestStart => "TST▶",
            NavKind::TestEnd => "TST■", NavKind::Step => "STP", NavKind::Teardown => "TDN",
            NavKind::Custom => "★", NavKind::Bookmark => "♥",
        }
    }
}
#[derive(Debug, Clone)]
struct NavEntry { kind: NavKind, row_idx: usize, line_num: usize, label: String }

#[derive(Debug, Clone)]
struct LogLine {
    num: usize, timestamp: String, ts_ms: Option<u64>, delta_ms: Option<u64>,
    level: Level, module: String, message: String, raw: String,
}

fn parse_timestamp_ms(ts: &str) -> Option<u64> {
    let ts = ts.trim();
    let c1 = ts.find(':')?;
    let after1 = &ts[c1 + 1..];
    let c2 = after1.find(':')?;
    let h: u64 = ts[..c1].parse().ok()?;
    let m: u64 = after1[..c2].parse().ok()?;
    let sec_rest = &after1[c2 + 1..];
    let (s_str, frac) = if let Some(dot) = sec_rest.find('.') { (&sec_rest[..dot], &sec_rest[dot + 1..]) } else { (sec_rest, "") };
    let s_str = s_str.trim_end_matches(|c: char| !c.is_ascii_digit());
    let s: u64 = s_str.parse().ok()?;
    let ms: u64 = if frac.is_empty() { 0 } else { let n = frac.len().min(3); let v: u64 = frac[..n].parse().ok()?; match n { 1 => v * 100, 2 => v * 10, _ => v } };
    Some(h * 3_600_000 + m * 60_000 + s * 1_000 + ms)
}
fn format_delta(ms: u64) -> String {
    if ms < 1_000 { format!("+{}ms", ms) }
    else if ms < 10_000 { format!("+{:.2}s", ms as f64 / 1000.0) }
    else if ms < 60_000 { format!("+{:.1}s", ms as f64 / 1000.0) }
    else { let s = ms / 1_000; format!("+{}m{:02}s", s / 60, s % 60) }
}
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' { if it.peek() == Some(&'[') { it.next(); for nc in it.by_ref() { if nc.is_ascii_alphabetic() { break; } } } }
        else { out.push(c); }
    }
    out
}
fn take_bracket(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('[') { return None; }
    let s = &s[1..];
    let end = s.find(']')?;
    Some((&s[..end], s[end + 1..].trim_start()))
}
fn parse_log_line(raw: &str, num: usize) -> LogLine {
    let s = strip_ansi(raw.trim());
    let make = |ts: String, level: Level, module: String, message: String| LogLine { num, timestamp: ts, ts_ms: None, delta_ms: None, level, module, message, raw: raw.to_string() };
    if s.starts_with('[') {
        if let Some((ts_raw, rest)) = take_bracket(&s) {
            let ts = ts_raw.split_whitespace().nth(1).or_else(|| ts_raw.split_whitespace().next()).unwrap_or(ts_raw).to_string();
            if let Some((lv, rest2)) = take_bracket(rest) {
                let level = Level::from_str(lv);
                let (module, message) = if let Some((m, msg)) = take_bracket(rest2) { (m.to_string(), msg.to_string()) } else { (String::new(), rest2.to_string()) };
                return make(ts, level, module, message);
            }
        }
    }
    {
        let parts: Vec<&str> = s.splitn(3, ' ').collect();
        if parts.len() == 3 {
            let ts_cand = parts[0];
            if ts_cand.len() >= 5 && ts_cand.as_bytes().get(2) == Some(&b':') {
                if let Some((thread, rest)) = take_bracket(parts[1]) {
                    if let Some(cp) = rest.find(':') {
                        let lv = rest[..cp].trim();
                        if matches!(lv.to_uppercase().as_str(), "DEBUG"|"INFO"|"WARN"|"WARNING"|"ERROR"|"TRACE") {
                            return make(ts_cand.to_string(), Level::from_str(lv), thread.to_string(), rest[cp + 1..].trim().to_string());
                        }
                    }
                }
            }
        }
    }
    if let Some(pos) = s.find("  ") {
        let ts = s[..pos].split_whitespace().nth(1).unwrap_or("").to_string();
        let rest = s[pos..].trim();
        if let Some((module, rest2)) = take_bracket(rest) {
            if let Some((lv, msg)) = take_bracket(rest2) { return make(ts, Level::from_str(lv), module.trim().to_string(), msg.to_string()); }
        }
    }
    make(String::new(), Level::Debug, String::new(), s.to_string())
}
fn trunc(s: &str, max: usize) -> String { if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max.saturating_sub(1)]) } }

// ─── App State ───────────────────────────────────────────────────────────────
struct LogViewerApp {
    theme: Theme,
    all_lines: Vec<LogLine>, filtered: Vec<usize>, modules: Vec<String>, counts: [usize; 5],
    filter_text: String, module_filter: String, show: [bool; 5],
    row_height: f32, font_size: f32, wrap_lines: bool,
    selected: Option<usize>, detail_open: bool, status: String, drag_hover: bool,
    current_file: Option<PathBuf>,
    minimap_levels: Vec<u8>,
    scroll_to_offset: Option<f32>, current_scroll_offset: f32, scroll_area_height: f32,
    search: SearchState, find_dialog_open: bool,
    nav_open: bool, nav_entries: Vec<NavEntry>,
    nav_show_error: bool, nav_show_warning: bool, nav_show_teststart: bool,
    nav_show_testend: bool, nav_show_step: bool, nav_show_teardown: bool,
    nav_show_custom: bool, nav_show_bookmark: bool,
    nav_custom_kw: String, nav_custom_kw_buf: String,
    bookmarks: Vec<usize>,
}

impl Default for LogViewerApp {
    fn default() -> Self {
        let theme = Theme::dark_modern();
        Self {
            theme,
            all_lines: vec![], filtered: vec![], filter_text: String::new(), show: [true; 5],
            module_filter: String::new(), modules: vec![], counts: [0; 5],
            row_height: 22.0, font_size: 12.5, wrap_lines: false,
            selected: None, detail_open: false,
            status: "Ready — drop or open a log file".into(), drag_hover: false, current_file: None,
            minimap_levels: vec![], scroll_to_offset: None, current_scroll_offset: 0.0, scroll_area_height: 0.0,
            search: SearchState::new(), find_dialog_open: false,
            nav_open: false, nav_entries: vec![],
            nav_show_error: true, nav_show_warning: true, nav_show_teststart: true,
            nav_show_testend: true, nav_show_step: true, nav_show_teardown: true,
            nav_show_custom: true, nav_show_bookmark: true,
            nav_custom_kw: String::new(), nav_custom_kw_buf: String::new(),
            bookmarks: vec![],
        }
    }
}

impl LogViewerApp {
    fn load_text(&mut self, text: &str) {
        self.all_lines = text.lines().filter(|l| !l.trim().is_empty()).enumerate()
            .map(|(i, l)| { let mut ln = parse_log_line(l, i + 1); ln.ts_ms = parse_timestamp_ms(&ln.timestamp); ln })
            .collect();
        let mut prev_ms: Option<u64> = None;
        for line in &mut self.all_lines {
            line.delta_ms = match (prev_ms, line.ts_ms) { (Some(p), Some(c)) => Some(c.saturating_sub(p)), _ => None };
            if line.ts_ms.is_some() { prev_ms = line.ts_ms; }
        }
        self.counts = [0; 5];
        let mut mod_set: std::collections::BTreeSet<String> = Default::default();
        for l in &self.all_lines { self.counts[l.level.index()] += 1; if !l.module.is_empty() { mod_set.insert(l.module.clone()); } }
        self.modules = mod_set.into_iter().collect();
        self.module_filter.clear(); self.selected = None; self.detail_open = false; self.current_scroll_offset = 0.0;
        self.bookmarks.clear(); self.apply_filters();
    }
    fn load_file(&mut self, path: &PathBuf) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.load_text(&text);
                self.current_file = Some(path.clone());
                self.status = format!("{}  ·  {} lines", path.file_name().unwrap_or_default().to_string_lossy(), self.all_lines.len());
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }
    fn apply_filters(&mut self) {
        let show = self.show;
        let filter_lc = self.filter_text.to_lowercase();
        let mf = self.module_filter.clone();
        self.filtered = self.all_lines.iter().enumerate().filter(|(_, l)| {
            show[l.level.index()] && (mf.is_empty() || l.module == mf) && (filter_lc.is_empty() || l.raw.to_lowercase().contains(&filter_lc))
        }).map(|(i, _)| i).collect();
        self.minimap_levels = self.filtered.iter().map(|&i| self.all_lines[i].level.index() as u8).collect();
        self.search.find_all(&self.filtered, &self.all_lines);
        self.recompute_nav();
    }
    fn do_find_next(&mut self) {
        if self.search.matches.is_empty() { self.search.find_all(&self.filtered, &self.all_lines); }
        if self.search.matches.is_empty() { self.status = "No matches found".to_string(); return; }
        if let Some(row) = self.search.next() { self.scroll_to_offset = Some(row as f32 * self.row_height); self.selected = Some(row); self.detail_open = true; }
    }
    fn do_find_prev(&mut self) {
        if let Some(row) = self.search.prev() { self.scroll_to_offset = Some(row as f32 * self.row_height); self.selected = Some(row); self.detail_open = true; }
    }
    fn do_find_all_with_results(&mut self) {
        self.search.find_all(&self.filtered, &self.all_lines);
        if self.search.matches.is_empty() { self.status = "No matches found".to_string(); }
        else {
            self.search.results_panel_open = true;
            self.status = format!("{} matches", self.search.matches.len());
            if let Some(mat) = self.search.matches.first() { self.scroll_to_offset = Some(mat.row_idx as f32 * self.row_height); self.selected = Some(mat.row_idx); }
        }
    }
    fn toggle_bookmark(&mut self, row_idx: usize) {
        if let Some(pos) = self.bookmarks.iter().position(|&r| r == row_idx) { self.bookmarks.remove(pos); }
        else { self.bookmarks.push(row_idx); self.bookmarks.sort_unstable(); }
        self.recompute_nav();
    }
    fn is_bookmarked(&self, row_idx: usize) -> bool { self.bookmarks.binary_search(&row_idx).is_ok() }
    fn recompute_nav(&mut self) {
        self.nav_entries.clear();
        let kw_lc = self.nav_custom_kw.to_lowercase();
        for (row_idx, &line_idx) in self.filtered.iter().enumerate() {
            let line = &self.all_lines[line_idx];
            let raw_lc = line.raw.to_lowercase();
            if self.is_bookmarked(row_idx) { self.nav_entries.push(NavEntry { kind: NavKind::Bookmark, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); }
            if matches!(line.level, Level::Error) { self.nav_entries.push(NavEntry { kind: NavKind::Error, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue; }
            if matches!(line.level, Level::Warning) { self.nav_entries.push(NavEntry { kind: NavKind::Warning, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue; }
            if raw_lc.contains("test serie started") || raw_lc.contains("test started:") || raw_lc.contains("test case started") || raw_lc.contains("testcase start") { self.nav_entries.push(NavEntry { kind: NavKind::TestStart, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue; }
            if raw_lc.contains("test case status") || raw_lc.contains("test serie ended") || raw_lc.contains("testcase end") || raw_lc.contains("test result:") || (raw_lc.contains("result") && (raw_lc.contains("passed") || raw_lc.contains("failed"))) { self.nav_entries.push(NavEntry { kind: NavKind::TestEnd, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue; }
            if raw_lc.contains("] step ") || raw_lc.contains("[step]") || (raw_lc.contains("step ") && matches!(line.level, Level::Info)) { self.nav_entries.push(NavEntry { kind: NavKind::Step, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue; }
            if raw_lc.contains("teardown") || raw_lc.contains("tear down") || raw_lc.contains("cleanup") || raw_lc.contains("---teardown---") { self.nav_entries.push(NavEntry { kind: NavKind::Teardown, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue; }
            if !kw_lc.is_empty() && raw_lc.contains(kw_lc.as_str()) { self.nav_entries.push(NavEntry { kind: NavKind::Custom, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); }
        }
    }
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Log files", &["log", "txt"]).add_filter("All files", &["*"]).pick_file() { self.load_file(&path); }
    }
    fn clear_file(&mut self) { *self = LogViewerApp::default(); }
    fn export_filtered(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Log files", &["log", "txt"]).set_file_name("filtered.log").save_file() {
            let content: String = self.filtered.iter().filter_map(|&i| self.all_lines.get(i)).map(|l| l.raw.as_str()).collect::<Vec<_>>().join("\n");
            match std::fs::write(&path, content) { Ok(_) => self.status = format!("Exported {} lines", self.filtered.len()), Err(e) => self.status = format!("Export failed: {e}"), }
        }
    }
    fn export_search_results(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Text files", &["txt"]).add_filter("CSV files", &["csv"]).set_file_name("search_results.txt").save_file() {
            let is_csv = path.extension().map(|e| e == "csv").unwrap_or(false);
            let content = if is_csv {
                let mut lines = vec!["Line,Level,Module,Match,Context".to_string()];
                for mat in &self.search.matches { lines.push(format!("{},{},{},\"{}\",\"{}{}{}\"", mat.line_num, mat.level.label(), mat.module, mat.match_text.replace('"', "\"\""), mat.context_before.replace('"', "\"\""), mat.match_text.replace('"', "\"\""), mat.context_after.replace('"', "\"\""))); }
                lines.join("\n")
            } else {
                let mut lines = vec![format!("Search Results: \"{}\"", self.search.find_what), format!("Total matches: {}", self.search.matches.len()), String::new(), "─".repeat(80), String::new()];
                for mat in &self.search.matches { lines.push(format!("Line {:>5} │ {:>4} │ {:>12} │ {}{}{}", mat.line_num, mat.level.label(), if mat.module.len() > 12 { &mat.module[..12] } else { &mat.module }, mat.context_before, mat.match_text, mat.context_after)); }
                lines.join("\n")
            };
            match std::fs::write(&path, content) { Ok(_) => self.status = format!("Exported {} matches", self.search.matches.len()), Err(e) => self.status = format!("Export failed: {e}"), }
        }
    }
}

// ─── UI Helpers ──────────────────────────────────────────────────────────────
fn icon_btn(ui: &mut egui::Ui, icon: &str, tooltip: &str, active: bool, theme: &Theme) -> bool {
    let color = if active { theme.accent } else { theme.text_muted };
    let stroke = if active { Stroke::new(1.0, theme.border_active) } else { Stroke::new(1.0, theme.border) };
    let fill = if active { Color32::from_rgba_unmultiplied(88, 166, 255, 20) } else { Color32::TRANSPARENT };
    
    ui.add(Button::new(RichText::new(icon).font(FontId::proportional(14.0)).color(color))
        .fill(fill).stroke(stroke).rounding(Rounding::same(6.0))
        .min_size(Vec2::new(32.0, 28.0)))
        .on_hover_text(tooltip)
        .clicked()
}

fn level_badge(ui: &mut egui::Ui, label: &str, count: usize, active: bool, color: Color32, theme: &Theme) -> bool {
    let (fg, bg, stroke) = if active {
        (color, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30), Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120)))
    } else {
        (theme.text_muted, theme.bg_input, Stroke::new(1.0, theme.border))
    };
    
    let btn = Button::new(
        RichText::new(format!("{} {}", label, count)).color(fg).font(FontId::monospace(11.0)).strong()
    ).fill(bg).stroke(stroke).rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 26.0));
    
    ui.add(btn).clicked()
}

// ─── Main Update Loop ────────────────────────────────────────────────────────
impl App for LogViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.theme.apply_visuals(ctx);
        
        if let Some(ref path) = self.current_file {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{} — XTR Log Viewer", name)));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("XTR Log Viewer".to_string()));
        }

        ctx.input(|i| {
            self.drag_hover = !i.raw.hovered_files.is_empty();
            for d in &i.raw.dropped_files {
                if let Some(p) = d.path.clone() { self.load_file(&p); }
                else if let Some(b) = &d.bytes { if let Ok(t) = std::str::from_utf8(b) { self.load_text(t); } }
            }
            if i.key_pressed(Key::O) && i.modifiers.ctrl { self.open_file_dialog(); }
            if i.key_pressed(Key::F) && i.modifiers.ctrl { self.find_dialog_open = true; }
            if i.key_pressed(Key::N) && i.modifiers.ctrl { self.nav_open = !self.nav_open; }
            if i.key_pressed(Key::B) && i.modifiers.ctrl { if let Some(sel) = self.selected { self.toggle_bookmark(sel); } }
            if i.key_pressed(Key::W) && i.modifiers.ctrl { self.wrap_lines = !self.wrap_lines; }
            if i.key_pressed(Key::Escape) {
                if self.search.results_panel_open { self.search.results_panel_open = false; }
                else if self.find_dialog_open { self.find_dialog_open = false; }
                else if !self.filter_text.is_empty() { self.filter_text.clear(); self.apply_filters(); }
                else { self.selected = None; self.detail_open = false; }
            }
            if i.key_pressed(Key::F3) { if i.modifiers.shift { self.do_find_prev(); } else { self.do_find_next(); } }
        });

        // ── Top Bar ─────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("top_bar")
            .exact_height(56.0)
            .frame(EguiFrame::none().fill(self.theme.bg_header).stroke(Stroke::new(1.0, self.theme.border)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.add_space(12.0);
                        ui.menu_button(RichText::new("File").font(FontId::proportional(12.0)), |ui| {
                            ui.set_min_width(180.0);
                            if ui.button("📁 Open...").clicked() { self.open_file_dialog(); ui.close_menu(); }
                            if ui.button("🔄 Reload").clicked() { if let Some(p) = self.current_file.clone() { self.load_file(&p); } ui.close_menu(); }
                            ui.separator();
                            if ui.button("💾 Export Filtered").clicked() { self.export_filtered(); ui.close_menu(); }
                            if ui.button("🗑 Clear").clicked() { self.clear_file(); ui.close_menu(); }
                        });
                        ui.menu_button(RichText::new("Help").font(FontId::proportional(12.0)), |ui| {
                            ui.label("Ctrl+O: Open"); ui.label("Ctrl+F: Find"); ui.label("Ctrl+N: Navigation");
                        });
                        ui.add_space(20.0);
                        ui.label(RichText::new(&self.status).font(FontId::monospace(10.0)).color(self.theme.text_muted));
                    });
                    ui.add(Separator::default().spacing(6.0));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.add_space(12.0);

                        // Search Input
                        let search_resp = ui.add_sized([240.0, 28.0], TextEdit::singleline(&mut self.filter_text)
                            .hint_text(RichText::new("Filter logs...").italics().color(self.theme.text_muted))
                            .font(FontId::monospace(12.0))
                            .desired_width(240.0));
                        
                        if search_resp.changed() { self.apply_filters(); }
                        if search_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) { self.do_find_next(); }
                        
                        // Clear Button inside Search
                        if !self.filter_text.is_empty() {
                            let clear_rect = egui::Rect::from_center_size(
                                egui::pos2(search_resp.rect.right() - 14.0, search_resp.rect.center().y), 
                                Vec2::splat(16.0)
                            );
                            if ui.interact(clear_rect, ui.id().with("clear_search"), Sense::click()).clicked() {
                                self.filter_text.clear(); self.apply_filters();
                            }
                            ui.painter().text(clear_rect.center(), Align2::CENTER_CENTER, "✕", FontId::proportional(10.0), self.theme.text_muted);
                        }

                        ui.add(Separator::default().vertical().spacing(8.0));

                        // Module Filter - FIXED VERSION
                        if !self.modules.is_empty() {
                            let label = if self.module_filter.is_empty() { "All Modules" } else { &self.module_filter };
                            let current_filter = self.module_filter.clone();
                            
                            let mut selected_module: Option<Option<String>> = None;
                            
                            egui::ComboBox::from_id_source("mod_cb")
                                .selected_text(RichText::new(label).font(FontId::proportional(11.0)))
                                .width(140.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(current_filter.is_empty(), "All Modules").clicked() { 
                                        selected_module = Some(None);
                                    }
                                    for m in &self.modules {
                                        if ui.selectable_label(&current_filter == m, m).clicked() { 
                                            selected_module = Some(Some(m.clone()));
                                        }
                                    }
                                });
                            
                            if let Some(selection) = selected_module {
                                match selection {
                                    None => self.module_filter.clear(),
                                    Some(m) => self.module_filter = m,
                                }
                                self.apply_filters();
                            }
                        }

                        ui.add(Separator::default().vertical().spacing(8.0));

                        // Level Toggles
                        let defs = [(0, "ERR", self.theme.danger), (1, "WRN", self.theme.warning), (2, "INF", self.theme.success), (3, "DBG", self.theme.info), (4, "TRC", self.theme.text_muted)];
                        let mut changed = false;
                        for (idx, lbl, col) in defs {
                            if level_badge(ui, lbl, self.counts[idx], self.show[idx], col, &self.theme) {
                                self.show[idx] = !self.show[idx]; changed = true;
                            }
                        }
                        if changed { self.apply_filters(); }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            
                            if icon_btn(ui, "A-", "Smaller Font", false, &self.theme) { self.font_size = (self.font_size - 1.0).max(9.0); self.row_height = self.font_size + 9.0; }
                            if icon_btn(ui, "A+", "Larger Font", false, &self.theme) { self.font_size = (self.font_size + 1.0).min(20.0); self.row_height = self.font_size + 9.0; }
                            
                            ui.add(Separator::default().vertical().spacing(6.0));

                            if icon_btn(ui, if self.wrap_lines { "↩" } else { "→" }, "Toggle Line Wrap", self.wrap_lines, &self.theme) { self.wrap_lines = !self.wrap_lines; }
                            
                            let nav_count = self.nav_entries.len();
                            let nav_label = if nav_count > 0 { format!("Nav ({})", nav_count) } else { "Nav".to_string() };
                            if ui.add(Button::new(RichText::new(nav_label).font(FontId::proportional(11.0)))
                                .fill(if self.nav_open { Color32::from_rgba_unmultiplied(88, 166, 255, 20) } else { Color32::TRANSPARENT })
                                .stroke(Stroke::new(1.0, if self.nav_open { self.theme.border_active } else { self.theme.border }))
                                .rounding(Rounding::same(6.0))
                                .min_size(Vec2::new(60.0, 28.0)))
                                .on_hover_text("Toggle Navigation Panel").clicked() {
                                self.nav_open = !self.nav_open;
                            }

                            if icon_btn(ui, "🔍", "Find Dialog (Ctrl+F)", self.find_dialog_open, &self.theme) { self.find_dialog_open = true; }
                            
                            if self.all_lines.is_empty() {
                                if ui.add(Button::new(RichText::new("📁 Open File").strong().color(self.theme.bg_main)).fill(self.theme.accent).stroke(Stroke::NONE).rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 28.0))).clicked() { self.open_file_dialog(); }
                            }
                        });
                    });
                });
            });

        // ── Find Dialog ────────────────────────────────────────────────────
        if self.find_dialog_open {
            let screen = ctx.screen_rect();
            let dialog_w = 480.0;
            let dialog_h = 280.0;
            let pos = egui::pos2((screen.width() - dialog_w) / 2.0, 100.0);
            
            Window::new("")
                .id(egui::Id::new("find_dlg"))
                .fixed_pos(pos)
                .fixed_size([dialog_w, dialog_h])
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .frame(EguiFrame::none()
                    .fill(self.theme.bg_panel)
                    .stroke(Stroke::new(1.0, self.theme.border))
                    .rounding(Rounding::same(10.0))
                    .shadow(egui::epaint::Shadow { 
                        offset: Vec2::new(0.0, 4.0), 
                        blur: 20.0, 
                        spread: 0.0,
                        color: Color32::BLACK 
                    }))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⌕ Find").strong().size(14.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("✕").clicked() { self.find_dialog_open = false; }
                            });
                        });
                        ui.add(Separator::default().spacing(10.0));

                        let te = ui.add(TextEdit::singleline(&mut self.search.find_what)
                            .hint_text("Search...").font(FontId::monospace(13.0)).desired_width(f32::INFINITY));
                        if te.changed() { self.search.first_search = true; self.search.find_all(&self.filtered, &self.all_lines); }
                        if te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) { self.do_find_next(); }
                        
                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            let mut any_changed = false;
                            
                            if ui.add(Checkbox::new(&mut self.search.match_case, "Aa")).on_hover_text("Match Case").clicked() { any_changed = true; }
                            ui.add_space(8.0);
                            if ui.add(Checkbox::new(&mut self.search.whole_word, "\\b")).on_hover_text("Whole Word").clicked() { any_changed = true; }
                            ui.add_space(8.0);
                            if ui.add(Checkbox::new(&mut self.search.wrap_around, "↻")).on_hover_text("Wrap Around").clicked() { any_changed = true; }
                            ui.add_space(8.0);
                            if ui.add(Checkbox::new(&mut self.search.backward, "←")).on_hover_text("Backward").clicked() { any_changed = true; }

                            if any_changed { self.search.first_search = true; self.search.find_all(&self.filtered, &self.all_lines); }
                        });

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            if ui.add(Button::new("▶ Next").fill(self.theme.accent).stroke(Stroke::NONE).rounding(Rounding::same(5.0)).min_size(Vec2::new(100.0, 30.0))).clicked() { self.do_find_next(); }
                            if ui.add_enabled(!self.search.matches.is_empty(), Button::new("◀ Prev").fill(self.theme.bg_input).stroke(Stroke::new(1.0, self.theme.border)).rounding(Rounding::same(5.0)).min_size(Vec2::new(100.0, 30.0))).clicked() { self.do_find_prev(); }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Find All").clicked() { self.do_find_all_with_results(); }
                            });
                        });
                        
                        if !self.search.find_what.is_empty() && self.search.matches.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new("No matches found").color(self.theme.danger).small());
                        } else if !self.search.matches.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("{} / {} matches", self.search.current_match_idx + 1, self.search.matches.len())).font(FontId::monospace(10.0)));
                        }
                    });
                });
        }

        // ─ Minimap ────────────────────────────────────────────────────────
        if !self.all_lines.is_empty() {
            let n_filt = self.filtered.len();
            let row_h = self.row_height;
            let scroll_off = self.current_scroll_offset;
            let viewport_h = self.scroll_area_height;
            let ml = self.minimap_levels.clone();
            let mut jump_to_offset: Option<f32> = None;
            
            const MM: [Color32; 5] = [
                Color32::from_rgb(235, 85, 85), Color32::from_rgb(230, 190, 70),
                Color32::from_rgb(80, 205, 105), Color32::from_rgb(100, 180, 255),
                Color32::from_rgb(130, 135, 150),
            ];

            egui::SidePanel::right("minimap_panel")
                .exact_width(34.0)
                .resizable(false)
                .frame(EguiFrame::none().fill(self.theme.bg_main))
                .show(ctx, |ui| {
                    let avail = ui.available_rect_before_wrap();
                    let (resp, painter) = ui.allocate_painter(avail.size(), Sense::click_and_drag());
                    let r = resp.rect;
                    
                    if n_filt == 0 { return; }
                    
                    let (bx0, bx1, by0, ah) = (r.min.x + 4.0, r.max.x - 4.0, r.min.y, r.height());
                    
                    for py in 0..ah as usize {
                        let i0 = ((py as f32 * n_filt as f32 / ah) as usize).min(n_filt - 1);
                        let i1 = (((py + 1) as f32 * n_filt as f32 / ah) as usize).min(n_filt - 1).max(i0);
                        let bucket = (i1 - i0 + 1) as f32;
                        let mut counts = [0u16; 5];
                        for i in i0..=i1 { counts[ml[i] as usize] += 1; }
                        let dom = (0..5).find(|&l| counts[l] as f32 / bucket >= 0.20)
                            .unwrap_or_else(|| counts.iter().enumerate().max_by(|(ia, &ca), (ib, &cb)| ca.cmp(&cb).then(ib.cmp(ia))).map(|(i, _)| i).unwrap_or(4));
                        
                        let y0 = by0 + py as f32;
                        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(bx0, y0), egui::pos2(bx1, y0 + 1.5)), Rounding::ZERO, MM[dom]);
                    }

                    let total_h = n_filt as f32 * row_h;
                    if total_h > 0.0 && viewport_h > 0.0 {
                        let vt = (scroll_off / total_h).clamp(0.0, 1.0);
                        let vb = ((scroll_off + viewport_h) / total_h).clamp(0.0, 1.0);
                        let wy0 = (by0 + vt * ah).min(r.max.y - 4.0);
                        let wy1 = (by0 + vb * ah).clamp(wy0 + 4.0, r.max.y);
                        painter.rect(egui::Rect::from_min_max(egui::pos2(r.min.x + 1.0, wy0), egui::pos2(r.max.x - 1.0, wy1)), Rounding::same(2.0), Color32::from_rgba_unmultiplied(255, 255, 255, 20), Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)));
                    }

                    if resp.dragged() || resp.clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let frac = ((pos.y - by0) / ah).clamp(0.0, 1.0);
                            let tr = ((frac * n_filt as f32) as usize).min(n_filt.saturating_sub(1));
                            let mut toff = tr as f32 * row_h;
                            if total_h > viewport_h { toff = toff.min(total_h - viewport_h); } else { toff = 0.0; }
                            jump_to_offset = Some(toff);
                        }
                    }
                });
            if let Some(off) = jump_to_offset { self.scroll_to_offset = Some(off); }
        }

        // ── Navigation Panel ────────────────────────────────────────────────
        if self.nav_open && !self.all_lines.is_empty() {
            let mut jump: Option<usize> = None;
            let visible_data: Vec<(NavKind, usize, usize, String)> = self.nav_entries.iter()
                .filter(|e| match e.kind {
                    NavKind::Error => self.nav_show_error, NavKind::Warning => self.nav_show_warning,
                    NavKind::TestStart => self.nav_show_teststart, NavKind::TestEnd => self.nav_show_testend,
                    NavKind::Step => self.nav_show_step, NavKind::Teardown => self.nav_show_teardown,
                    NavKind::Custom => self.nav_show_custom, NavKind::Bookmark => self.nav_show_bookmark,
                }).map(|e| (e.kind, e.row_idx, e.line_num, e.label.clone())).collect();

            egui::SidePanel::right("nav_panel")
                .default_width(240.0).width_range(200.0..=350.0).resizable(true)
                .frame(EguiFrame::none().fill(self.theme.bg_panel).stroke(Stroke::new(1.0, self.theme.border)))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                        
                        EguiFrame::group(&ui.style()).fill(self.theme.bg_header).inner_margin(Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("NAVIGATION").strong().small().color(self.theme.text_muted));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{}", visible_data.len())).small().color(self.theme.text_muted));
                                });
                            });
                        });

                        EguiFrame::none().inner_margin(Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                            ui.horizontal_wrapped(|ui| {
                                macro_rules! cb { 
                                    ($field:expr, $label:expr, $kind:expr) => {{
                                        let c = $kind.color(&self.theme);
                                        let resp = ui.add(Checkbox::new($field, ""));
                                        if resp.clicked() { 
                                            self.recompute_nav(); 
                                        }
                                        ui.label(RichText::new($label).color(c).small());
                                    }}
                                }
                                
                                cb!(&mut self.nav_show_error, "ERR", NavKind::Error);
                                cb!(&mut self.nav_show_warning, "WRN", NavKind::Warning);
                                cb!(&mut self.nav_show_teststart, "Start", NavKind::TestStart);
                                cb!(&mut self.nav_show_testend, "End", NavKind::TestEnd);
                                cb!(&mut self.nav_show_step, "Step", NavKind::Step);
                                cb!(&mut self.nav_show_teardown, "Down", NavKind::Teardown);
                                cb!(&mut self.nav_show_custom, "★", NavKind::Custom);
                                cb!(&mut self.nav_show_bookmark, "♥", NavKind::Bookmark);
                            });
                            ui.add_space(6.0);
                            let te_resp = ui.text_edit_singleline(&mut self.nav_custom_kw_buf);
                            if te_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) { 
                                self.nav_custom_kw = self.nav_custom_kw_buf.clone(); 
                                self.recompute_nav(); 
                            }
                            if ui.button("Update Custom").clicked() { 
                                self.nav_custom_kw = self.nav_custom_kw_buf.clone(); 
                                self.recompute_nav(); 
                            }
                        });

                        ui.add(Separator::default().spacing(0.0));

                        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            if visible_data.is_empty() {
                                ui.centered_and_justified(|ui| ui.label(RichText::new("No items").color(self.theme.text_muted).small()));
                            } else {
                                for (kind, row_idx, line_num, label) in &visible_data {
                                    let is_sel = self.selected == Some(*row_idx);
                                    let c = kind.color(&self.theme);
                                    let bg = if is_sel { Color32::from_rgba_unmultiplied(88, 166, 255, 25) } else { Color32::TRANSPARENT };
                                    
                                    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
                                    if ui.is_rect_visible(rect) {
                                        ui.painter().rect_filled(rect, Rounding::ZERO, bg);
                                        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(3.0, 32.0)), Rounding::ZERO, c);
                                        
                                        let x = rect.min.x + 10.0;
                                        let y = rect.center().y;
                                        ui.painter().text(egui::pos2(x, y - 6.0), Align2::LEFT_BOTTOM, kind.short_label(), FontId::monospace(9.0), c);
                                        ui.painter().text(egui::pos2(x + 30.0, y - 6.0), Align2::LEFT_BOTTOM, format!("Line {}", line_num), FontId::monospace(9.0), self.theme.text_muted);
                                        ui.painter().text(egui::pos2(x, y + 4.0), Align2::LEFT_TOP, label.as_str(), FontId::proportional(11.0), self.theme.text_main);
                                    }
                                    if response.clicked() { jump = Some(*row_idx); }
                                    if response.double_clicked() { jump = Some(*row_idx); self.nav_open = false; }
                                }
                            }
                        });
                    });
                    
                    if let Some(row) = jump {
                        self.scroll_to_offset = Some(row as f32 * self.row_height);
                        self.selected = Some(row); self.detail_open = true;
                    }
                });
        }

        // ── Main Log Area ───────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(EguiFrame::none().fill(self.theme.bg_main))
            .show(ctx, |ui| {
                if self.all_lines.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("📄").size(64.0));
                            ui.add_space(16.0);
                            ui.label(RichText::new("Drop a log file here").size(20.0));
                            ui.add_space(24.0);
                            if ui.add(Button::new("Open File").fill(self.theme.accent).stroke(Stroke::NONE).rounding(Rounding::same(8.0)).min_size(Vec2::new(120.0, 36.0))).clicked() { self.open_file_dialog(); }
                        });
                    });
                    return;
                }
                if self.filtered.is_empty() {
                    ui.centered_and_justified(|ui| ui.label(RichText::new("No lines match filters").color(self.theme.text_muted)));
                    return;
                }

                // Column Headers
                let hdr_h = 24.0;
                let (hdr_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), hdr_h), Sense::hover());
                let p = ui.painter();
                let y = hdr_rect.center().y;
                let x0 = hdr_rect.min.x;
                let fid = FontId::monospace(10.0);
                let col = self.theme.text_muted;
                
                const COL_LN: f32 = 50.0;
                const COL_TS: f32 = 110.0;
                const COL_DT: f32 = 70.0;
                const COL_LV: f32 = 50.0;
                const COL_MOD: f32 = 160.0;

                p.text(egui::pos2(x0 + COL_LN - 5.0, y), Align2::RIGHT_CENTER, "#", fid.clone(), col);
                p.text(egui::pos2(x0 + COL_LN + 5.0, y), Align2::LEFT_CENTER, "TIME", fid.clone(), col);
                p.text(egui::pos2(x0 + COL_LN + COL_TS + 5.0, y), Align2::LEFT_CENTER, "Δ", fid.clone(), col);
                p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + 5.0, y), Align2::LEFT_CENTER, "LVL", fid.clone(), col);
                p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + COL_LV + 5.0, y), Align2::LEFT_CENTER, "MODULE", fid.clone(), col);
                p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + COL_LV + COL_MOD + 5.0, y), Align2::LEFT_CENTER, "MESSAGE", fid.clone(), col);
                
                ui.add(Separator::default().spacing(0.0));

                let row_h = self.row_height;
                let font_sz = self.font_size;
                let n = self.filtered.len();
                let visible_height = ui.available_height();
                
                let mut sa = ScrollArea::vertical().id_source("log_scroll").auto_shrink(false).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                if let Some(off) = self.scroll_to_offset.take() { sa = sa.scroll_offset(Vec2::new(0.0, off)); }
                
                let out = sa.show_rows(ui, row_h, n, |ui, row_range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for row_idx in row_range {
                        let line_idx = match self.filtered.get(row_idx) { Some(&i) => i, None => continue };
                        let line = match self.all_lines.get(line_idx) { Some(l) => l, None => continue };
                        
                        let is_sel = self.selected == Some(row_idx);
                        let is_find_match = self.search.matches.iter().any(|m| m.row_idx == row_idx);
                        let is_current_find = is_find_match && self.search.matches.get(self.search.current_match_idx).map(|m| m.row_idx) == Some(row_idx);
                        let is_bookmarked = self.is_bookmarked(row_idx);
                        
                        let (row_rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        if !ui.is_rect_visible(row_rect) { continue; }
                        
                        let bg = if is_sel { Color32::from_rgba_unmultiplied(88, 166, 255, 40) }
                            else if is_current_find { Color32::from_rgba_unmultiplied(255, 213, 79, 40) }
                            else if is_find_match { Color32::from_rgba_unmultiplied(255, 213, 79, 15) }
                            else if resp.hovered() { Color32::from_rgba_unmultiplied(255, 255, 255, 5) }
                            else if let Some(c) = line.level.row_bg(&self.theme) { c }
                            else { Color32::TRANSPARENT };
                        
                        if bg != Color32::TRANSPARENT { ui.painter().rect_filled(row_rect, Rounding::ZERO, bg); }
                        
                        if is_bookmarked { ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_h)), Rounding::ZERO, Color32::from_rgb(255, 140, 200)); }
                        if matches!(line.level, Level::Error | Level::Warning) {
                            let x_off = if is_bookmarked { 3.0 } else { 0.0 };
                            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(row_rect.min.x + x_off, row_rect.min.y), Vec2::new(2.0, row_h)), Rounding::ZERO, line.level.color(&self.theme));
                        }

                        let p = ui.painter();
                        let y = row_rect.center().y;
                        let fid = FontId::monospace(font_sz);
                        let fsm = FontId::monospace((font_sz - 1.0).max(8.0));
                        let fxs = FontId::monospace((font_sz - 2.0).max(7.5));
                        
                        let mut x = row_rect.min.x + if is_bookmarked { 6.0 } else { 4.0 };
                        
                        p.text(egui::pos2(x + COL_LN - 5.0, y), Align2::RIGHT_CENTER, line.num.to_string(), fxs.clone(), self.theme.text_muted);
                        x += COL_LN;
                        
                        let ts = if line.timestamp.len() > 12 { &line.timestamp[..12] } else { &line.timestamp };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, ts, fsm.clone(), Color32::from_rgb(150, 205, 255));
                        x += COL_TS;
                        
                        if let Some(dms) = line.delta_ms { if dms > 0 {
                            let dc = if dms >= 1000 { Color32::from_rgb(255, 200, 80) } else if dms >= 100 { Color32::from_rgb(175, 175, 195) } else { Color32::from_rgb(110, 118, 138) };
                            p.text(egui::pos2(x, y), Align2::LEFT_CENTER, format_delta(dms), fxs.clone(), dc);
                        }}
                        x += COL_DT;
                        
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, line.level.label(), fsm.clone(), line.level.color(&self.theme));
                        x += COL_LV;
                        
                        let md = if line.module.len() > 22 { &line.module[..22] } else { &line.module };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, md, fsm.clone(), Color32::from_rgb(170, 178, 195));
                        x += COL_MOD;
                        
                        let msg = &line.message;
                        let msg_col = match line.level {
                            Level::Error => Color32::from_rgb(255, 175, 165),
                            Level::Warning => Color32::from_rgb(255, 218, 148),
                            _ => Color32::from_rgb(205, 210, 222),
                        };
                        let available_width = row_rect.max.x - x - 8.0;
                        let max_chars = (available_width / (font_sz * 0.6)) as usize;
                        let msg_disp = if msg.len() > max_chars.max(40) { format!("{}…", &msg[..max_chars.max(40).saturating_sub(1)]) } else { msg.clone() };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, msg_disp, fid.clone(), msg_col);
                        
                        if resp.clicked() {
                            if is_sel { self.detail_open = !self.detail_open; }
                            else { self.selected = Some(row_idx); self.detail_open = true; }
                        }
                        if resp.double_clicked() { self.toggle_bookmark(row_idx); }
                    }
                });
                self.scroll_area_height = visible_height;
                self.current_scroll_offset = out.state.offset.y;
            });

        // ── Detail Panel ────────────────────────────────────────────────────
        if self.detail_open {
            let sel: Option<LogLine> = self.selected.and_then(|r| self.filtered.get(r).copied()).and_then(|li| self.all_lines.get(li)).cloned();
            if let Some(line) = sel {
                egui::TopBottomPanel::bottom("detail_panel")
                    .resizable(true).default_height(160.0).min_height(80.0)
                    .frame(EguiFrame::none().fill(self.theme.bg_panel).stroke(Stroke::new(1.0, self.theme.border)).inner_margin(Margin::symmetric(16.0, 12.0)))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("LINE DETAIL").strong().small().color(self.theme.text_muted));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if ui.button("✕").clicked() { self.detail_open = false; }
                                let is_bm = self.is_bookmarked(self.selected.unwrap_or(0));
                                let bm_text = if is_bm { "♥ Bookmarked" } else { "♡ Bookmark" };
                                let bm_col = if is_bm { Color32::from_rgb(255, 140, 200) } else { self.theme.text_muted };
                                if ui.add(Button::new(RichText::new(bm_text).color(bm_col).small()).fill(self.theme.bg_input).stroke(Stroke::new(1.0, self.theme.border)).rounding(Rounding::same(5.0))).clicked() { if let Some(sel) = self.selected { self.toggle_bookmark(sel); } }
                                if ui.button("📋 Copy").clicked() { ui.output_mut(|o| o.copied_text = line.raw.clone()); }
                            });
                        });
                        ui.add(Separator::default().spacing(8.0));
                        
                        egui::Grid::new("detail_grid").num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
                            let lbl = |s: &str| RichText::new(s).color(self.theme.text_muted).font(FontId::monospace(10.0));
                            let val = |s: String| RichText::new(s).color(self.theme.text_main).font(FontId::monospace(11.0));
                            ui.label(lbl("LINE")); ui.label(val(line.num.to_string())); ui.end_row();
                            ui.label(lbl("LEVEL")); ui.label(RichText::new(line.level.label()).color(line.level.color(&self.theme)).strong().font(FontId::monospace(11.0))); ui.end_row();
                            ui.label(lbl("TIME")); ui.label(val(line.timestamp.clone())); ui.end_row();
                            ui.label(lbl("Δ TIME")); ui.label(val(line.delta_ms.map(format_delta).unwrap_or_else(|| "—".into()))); ui.end_row();
                            ui.label(lbl("MODULE")); ui.label(val(line.module.clone())); ui.end_row();
                        });
                        
                        ui.add_space(8.0);
                        ui.label(RichText::new("MESSAGE").small().color(self.theme.text_muted));
                        ScrollArea::vertical().max_height(60.0).show(ui, |ui| {
                            ui.label(RichText::new(&line.message).font(FontId::monospace(12.0)).color(self.theme.text_main));
                        });
                    });
            }
        }
        
        // ─ Search Results Panel ────────────────────────────────────────────
        if self.search.results_panel_open && !self.search.matches.is_empty() {
            let mut close_panel = false;
            let mut jump_to: Option<usize> = None;
            
            egui::TopBottomPanel::bottom("results_panel")
                .resizable(true).default_height(self.search.results_panel_height).height_range(100.0..=400.0)
                .frame(EguiFrame::none().fill(self.theme.bg_panel).stroke(Stroke::new(1.0, self.theme.border)))
                .show(ctx, |ui| {
                    self.search.results_panel_height = ui.available_height();
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🔍 Search Results").strong());
                            ui.label(RichText::new(format!("({} matches)", self.search.matches.len())).color(self.theme.text_muted).small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("✕").clicked() { close_panel = true; }
                                if ui.button("Copy All").clicked() {
                                    let text = self.search.matches.iter().map(|m| format!("Line {}: {}", m.line_num, m.match_text)).collect::<Vec<_>>().join("\n");
                                    ui.output_mut(|o| o.copied_text = text);
                                }
                                if ui.button("Export").clicked() { self.export_search_results(); }
                            });
                        });
                        ui.add(Separator::default().spacing(0.0));
                        
                        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            for (idx, mat) in self.search.matches.iter().enumerate() {
                                let is_current = idx == self.search.current_match_idx;
                                let row_h = 28.0;
                                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                                if !ui.is_rect_visible(rect) { continue; }
                                
                                let bg = if is_current { Color32::from_rgba_unmultiplied(88, 166, 255, 30) } else if idx % 2 == 1 { Color32::from_rgba_unmultiplied(255, 255, 255, 3) } else { Color32::TRANSPARENT };
                                if bg != Color32::TRANSPARENT { ui.painter().rect_filled(rect, Rounding::ZERO, bg); }
                                if is_current { ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(3.0, row_h)), Rounding::ZERO, self.theme.accent); }
                                
                                let y = rect.center().y;
                                let mut x = rect.min.x + 12.0;
                                ui.painter().text(egui::pos2(x, y), Align2::LEFT_CENTER, mat.line_num.to_string(), FontId::monospace(10.0), self.theme.text_muted);
                                x += 50.0;
                                ui.painter().text(egui::pos2(x, y), Align2::LEFT_CENTER, mat.level.label(), FontId::monospace(9.0), mat.level.color(&self.theme));
                                x += 40.0;
                                ui.painter().text(egui::pos2(x, y), Align2::LEFT_CENTER, &mat.module, FontId::monospace(10.0), Color32::from_rgb(130, 140, 160));
                                x += 120.0;
                                
                                let ctx_text = format!("...{}{}{}...", mat.context_before, mat.match_text, mat.context_after);
                                ui.painter().text(egui::pos2(x, y), Align2::LEFT_CENTER, &ctx_text, FontId::monospace(10.0), self.theme.text_main);
                                
                                if response.clicked() { self.search.current_match_idx = idx; jump_to = Some(mat.row_idx); }
                                if response.double_clicked() { self.search.current_match_idx = idx; jump_to = Some(mat.row_idx); close_panel = true; }
                            }
                        });
                    });
                    
                    if close_panel { self.search.results_panel_open = false; }
                    if let Some(row) = jump_to { self.scroll_to_offset = Some(row as f32 * self.row_height); self.selected = Some(row); self.detail_open = true; }
                });
        }
        
        // ── Status Bar ──────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(24.0)
            .frame(EguiFrame::none().fill(self.theme.bg_header).stroke(Stroke::new(1.0, self.theme.border)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let mk = |n: usize, s: &str, c: Color32| RichText::new(format!("{} {}", n, s)).color(c).font(FontId::monospace(9.5));
                    ui.label(mk(self.counts[0], "errors", self.theme.danger));
                    ui.label(mk(self.counts[1], "warnings", self.theme.warning));
                    ui.label(mk(self.counts[2], "info", self.theme.success));
                    ui.label(mk(self.counts[3], "debug", self.theme.info));
                    ui.label(mk(self.counts[4], "trace", self.theme.text_muted));
                    
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} / {} lines", self.filtered.len(), self.all_lines.len())).color(self.theme.text_muted).font(FontId::monospace(9.5)));
                    });
                });
            });
    }
}

fn main() -> eframe::Result<()> {
    let opts = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("XTR Log Viewer")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([800.0, 400.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("XTR Log Viewer", opts, Box::new(|_cc| Box::new(LogViewerApp::default())))
}
