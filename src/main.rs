#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, App, Frame, NativeOptions};
use egui::{
    Align, Align2, Color32, FontId, Key, Layout, Rounding, ScrollArea, Sense, Stroke, Vec2,
    RichText, Button, Window, TextEdit,
};
use std::path::PathBuf;

// ─── Level ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Level { Error, Warning, Info, Debug, Trace }

impl Level {
    fn from_str(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "ERR"|"ERROR"           => Self::Error,
            "WRN"|"WARN"|"WARNING"  => Self::Warning,
            "INF"|"INFO"            => Self::Info,
            "DBG"|"DEBUG"           => Self::Debug,
            "TRC"|"TRACE"|"VERBOSE" => Self::Trace,
            _                       => Self::Debug,
        }
    }
    fn label(self) -> &'static str {
        match self { Self::Error=>"ERR", Self::Warning=>"WRN", Self::Info=>"INF",
                     Self::Debug=>"DBG", Self::Trace=>"TRC" }
    }
    fn color(self) -> Color32 {
        match self {
            Self::Error   => Color32::from_rgb(255, 110, 100),
            Self::Warning => Color32::from_rgb(240, 190,  70),
            Self::Info    => Color32::from_rgb( 80, 205, 105),
            Self::Debug   => Color32::from_rgb(100, 180, 255),
            Self::Trace   => Color32::from_gray(120),
        }
    }
    fn row_bg(self) -> Option<Color32> {
        match self {
            Self::Error   => Some(Color32::from_rgba_unmultiplied(200,  50, 40, 22)),
            Self::Warning => Some(Color32::from_rgba_unmultiplied(200, 150, 30, 18)),
            _             => None,
        }
    }
    fn index(self) -> usize {
        match self { Self::Error=>0, Self::Warning=>1, Self::Info=>2,
                     Self::Debug=>3, Self::Trace=>4 }
    }
}

// ─── Theme / Colors ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Colors {
    bg_base:      Color32,
    bg_panel:     Color32,
    bg_input:     Color32,
    bg_row_hover: Color32,
    bg_row_sel:   Color32,
    border:       Color32,
    border_hl:    Color32,
    text:         Color32,
    muted:        Color32,
    faint:        Color32,
    accent:       Color32,
    match_hl:     Color32,
    ts_color:     Color32,
}

impl Colors {
    fn dark() -> Self {
        Self {
            bg_base:      Color32::from_rgb(13,  17,  23),
            bg_panel:     Color32::from_rgb(18,  22,  30),
            bg_input:     Color32::from_rgb(10,  13,  20),
            bg_row_hover: Color32::from_rgba_premultiplied(255,255,255,10),
            bg_row_sel:   Color32::from_rgba_premultiplied(88,166,255,45),
            border:       Color32::from_rgb(36,  42,  52),
            border_hl:    Color32::from_rgb(58,  68,  84),
            text:         Color32::from_rgb(215, 222, 232),
            muted:        Color32::from_rgb(140, 148, 160),
            faint:        Color32::from_rgb(82,  88,  100),
            accent:       Color32::from_rgb(88,  166, 255),
            match_hl:     Color32::from_rgb(255, 213,  79),
            ts_color:     Color32::from_rgb(150, 205, 255),
        }
    }
    fn light() -> Self {
        Self {
            bg_base:      Color32::from_rgb(248, 250, 255),
            bg_panel:     Color32::from_rgb(235, 240, 250),
            bg_input:     Color32::from_rgb(255, 255, 255),
            bg_row_hover: Color32::from_rgba_premultiplied(0, 0, 0, 12),
            bg_row_sel:   Color32::from_rgba_premultiplied(30,100,220,40),
            border:       Color32::from_rgb(200, 210, 225),
            border_hl:    Color32::from_rgb(150, 170, 200),
            text:         Color32::from_rgb(20,  30,  50),
            muted:        Color32::from_rgb(90,  100, 125),
            faint:        Color32::from_rgb(155, 165, 185),
            accent:       Color32::from_rgb(30,  110, 220),
            match_hl:     Color32::from_rgb(200, 140,   0),
            ts_color:     Color32::from_rgb(30,  90,  190),
        }
    }

    fn visuals(&self, dark: bool) -> egui::Visuals {
        if dark {
            let mut v = egui::Visuals::dark();
            v.panel_fill          = self.bg_panel;
            v.window_fill         = self.bg_base;
            v.override_text_color = Some(self.text);
            v.widgets.inactive.bg_fill   = Color32::from_rgb(24, 30, 40);
            v.widgets.inactive.bg_stroke = Stroke::new(0.5, self.border);
            v.widgets.hovered.bg_fill    = Color32::from_rgb(34, 42, 54);
            v.widgets.hovered.bg_stroke  = Stroke::new(0.5, self.border_hl);
            v.widgets.active.bg_fill     = Color32::from_rgb(50, 60, 76);
            v.selection.bg_fill          = Color32::from_rgba_unmultiplied(88, 166, 255, 65);
            v
        } else {
            let mut v = egui::Visuals::light();
            v.panel_fill          = self.bg_panel;
            v.window_fill         = self.bg_base;
            v.override_text_color = Some(self.text);
            v.widgets.inactive.bg_fill   = Color32::from_rgb(225, 232, 245);
            v.widgets.inactive.bg_stroke = Stroke::new(0.5, self.border);
            v.widgets.hovered.bg_fill    = Color32::from_rgb(210, 220, 238);
            v.widgets.hovered.bg_stroke  = Stroke::new(0.5, self.border_hl);
            v.widgets.active.bg_fill     = Color32::from_rgb(190, 205, 230);
            v.selection.bg_fill          = Color32::from_rgba_unmultiplied(30, 110, 220, 65);
            v
        }
    }
}

// ─── Search ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SearchMode { #[default] Normal, Extended, Regex }

#[derive(Debug, Clone)]
struct SearchMatch {
    row_idx: usize, line_num: usize,
    match_text: String, context_before: String, context_after: String,
    module: String, level: Level,
}

#[derive(Debug, Clone)]
struct SearchState {
    find_what: String,
    match_case: bool, whole_word: bool, wrap_around: bool, backward: bool,
    mode: SearchMode,
    matches: Vec<SearchMatch>, current_match_idx: usize,
    results_panel_open: bool, results_panel_height: f32, first_search: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            find_what: String::new(),
            match_case: false, whole_word: false, wrap_around: true, backward: false,
            mode: SearchMode::Normal,
            matches: vec![], current_match_idx: 0,
            results_panel_open: false, results_panel_height: 200.0, first_search: true,
        }
    }
}

impl SearchState {
    fn expand_escapes(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n')  => result.push('\n'),
                    Some('r')  => result.push('\r'),
                    Some('t')  => result.push('\t'),
                    Some('0')  => result.push('\0'),
                    Some('\\') => result.push('\\'),
                    Some('x')  => {
                        let mut hex = String::new();
                        for _ in 0..2 {
                            if let Some(&c) = chars.peek() {
                                if c.is_ascii_hexdigit() { hex.push(chars.next().unwrap()); }
                            }
                        }
                        if let Ok(val) = u8::from_str_radix(&hex, 16) { result.push(val as char); }
                        else { result.push_str("\\x"); result.push_str(&hex); }
                    }
                    Some(other) => { result.push('\\'); result.push(other); }
                    None        => result.push('\\'),
                }
            } else { result.push(c); }
        }
        result
    }

    fn matches_whole_word(&self, hay: &str, needle: &str) -> bool {
        let mut start = 0;
        while let Some(pos) = hay[start..].find(needle) {
            let abs = start + pos; let end = abs + needle.len();
            let left_ok  = abs == 0 || !hay.as_bytes().get(abs.saturating_sub(1)).copied()
                .map(|b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false);
            let right_ok = end >= hay.len() || !hay.as_bytes().get(end).copied()
                .map(|b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false);
            if left_ok && right_ok { return true; }
            start = abs + 1;
        }
        false
    }

    fn find_all(&mut self, filtered: &[usize], all_lines: &[LogLine]) {
        self.matches.clear();
        if self.find_what.is_empty() { return; }
        let search_text = match self.mode {
            SearchMode::Extended => self.expand_escapes(&self.find_what),
            _                    => self.find_what.clone(),
        };
        let needle = if self.match_case { search_text.clone() } else { search_text.to_lowercase() };

        for (row_idx, &line_idx) in filtered.iter().enumerate() {
            let Some(line) = all_lines.get(line_idx) else { continue };
            let hay = if self.match_case { line.raw.clone() } else { line.raw.to_lowercase() };
            let mut start = 0;
            while let Some(pos) = hay[start..].find(&needle) {
                let abs_pos = start + pos; let match_end = abs_pos + needle.len();
                if self.whole_word && !self.matches_whole_word(&hay, &needle) {
                    start = abs_pos + 1; continue;
                }
                let before_start = abs_pos.saturating_sub(30);
                let after_end    = (match_end + 30).min(line.raw.len());
                self.matches.push(SearchMatch {
                    row_idx, line_num: line.num,
                    match_text:     line.raw[abs_pos..match_end].to_string(),
                    context_before: if before_start < abs_pos { line.raw[before_start..abs_pos].to_string() } else { String::new() },
                    context_after:  if match_end < after_end  { line.raw[match_end..after_end].to_string()  } else { String::new() },
                    module: line.module.clone(), level: line.level,
                });
                start = abs_pos + 1;
            }
        }
        if self.current_match_idx >= self.matches.len() { self.current_match_idx = 0; }
    }

    fn next(&mut self) -> Option<usize> {
        if self.matches.is_empty() { return None; }
        if self.first_search {
            self.first_search = false;
            self.current_match_idx = if self.backward { self.matches.len() - 1 } else { 0 };
        } else if self.backward {
            if self.current_match_idx == 0 {
                if self.wrap_around { self.current_match_idx = self.matches.len() - 1; }
            } else { self.current_match_idx -= 1; }
        } else {
            self.current_match_idx += 1;
            if self.current_match_idx >= self.matches.len() {
                self.current_match_idx = if self.wrap_around { 0 } else { self.matches.len() - 1 };
            }
        }
        Some(self.matches[self.current_match_idx].row_idx)
    }

    fn prev(&mut self) -> Option<usize> {
        if self.matches.is_empty() { return None; }
        if self.current_match_idx == 0 {
            if self.wrap_around { self.current_match_idx = self.matches.len().saturating_sub(1); }
        } else { self.current_match_idx -= 1; }
        Some(self.matches[self.current_match_idx].row_idx)
    }
}

// ─── NavEntry ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavKind { Error, Warning, TestStart, TestEnd, Step, Teardown, Custom, Bookmark }

impl NavKind {
    fn color(self) -> Color32 {
        match self {
            NavKind::Error     => Color32::from_rgb(255, 110, 100),
            NavKind::Warning   => Color32::from_rgb(240, 190,  70),
            NavKind::TestStart => Color32::from_rgb( 80, 205, 105),
            NavKind::TestEnd   => Color32::from_rgb(120, 200, 255),
            NavKind::Step      => Color32::from_rgb(140, 140, 220),
            NavKind::Teardown  => Color32::from_rgb(180, 150, 230),
            NavKind::Custom    => Color32::from_rgb(255, 200,  80),
            NavKind::Bookmark  => Color32::from_rgb(255, 140, 200),
        }
    }
    fn short_label(self) -> &'static str {
        match self {
            NavKind::Error=>"ERR", NavKind::Warning=>"WRN", NavKind::TestStart=>"TST▶",
            NavKind::TestEnd=>"TST■", NavKind::Step=>"STP", NavKind::Teardown=>"TDN",
            NavKind::Custom=>"★", NavKind::Bookmark=>"♥",
        }
    }
}

#[derive(Debug, Clone)]
struct NavEntry { kind: NavKind, row_idx: usize, line_num: usize, label: String }

// ─── LogLine ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LogLine {
    num: usize, timestamp: String, ts_ms: Option<u64>, delta_ms: Option<u64>,
    level: Level, module: String, message: String, raw: String,
}

fn parse_timestamp_ms(ts: &str) -> Option<u64> {
    let ts = ts.trim();
    let c1 = ts.find(':')?; let after1 = &ts[c1+1..]; let c2 = after1.find(':')?;
    let h: u64 = ts[..c1].parse().ok()?; let m: u64 = after1[..c2].parse().ok()?;
    let sec_rest = &after1[c2+1..];
    let (s_str, frac) = if let Some(dot) = sec_rest.find('.') { (&sec_rest[..dot], &sec_rest[dot+1..]) } else { (sec_rest, "") };
    let s_str = s_str.trim_end_matches(|c: char| !c.is_ascii_digit());
    let s: u64 = s_str.parse().ok()?;
    let ms: u64 = if frac.is_empty() { 0 } else {
        let n = frac.len().min(3); let v: u64 = frac[..n].parse().ok()?;
        match n { 1 => v*100, 2 => v*10, _ => v }
    };
    Some(h*3_600_000 + m*60_000 + s*1_000 + ms)
}

fn format_delta(ms: u64) -> String {
    if      ms < 1_000  { format!("+{}ms",           ms) }
    else if ms < 10_000 { format!("+{:.2}s", ms as f64/1000.0) }
    else if ms < 60_000 { format!("+{:.1}s", ms as f64/1000.0) }
    else { let s = ms/1_000; format!("+{}m{:02}s", s/60, s%60) }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len()); let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') { it.next(); for nc in it.by_ref() { if nc.is_ascii_alphabetic() { break; } } }
        } else { out.push(c); }
    }
    out
}

fn take_bracket(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('[') { return None; }
    let s = &s[1..]; let end = s.find(']')?;
    Some((&s[..end], s[end+1..].trim_start()))
}

fn parse_log_line(raw: &str, num: usize) -> LogLine {
    let s = strip_ansi(raw.trim());
    let make = |ts: String, level: Level, module: String, message: String| LogLine {
        num, timestamp: ts, ts_ms: None, delta_ms: None, level, module, message, raw: raw.to_string(),
    };
    if s.starts_with('[') {
        if let Some((ts_raw, rest)) = take_bracket(&s) {
            let ts = ts_raw.split_whitespace().nth(1)
                .or_else(|| ts_raw.split_whitespace().next()).unwrap_or(ts_raw).to_string();
            if let Some((lv, rest2)) = take_bracket(rest) {
                let level = Level::from_str(lv);
                let (module, message) = if let Some((m, msg)) = take_bracket(rest2)
                    { (m.to_string(), msg.to_string()) } else { (String::new(), rest2.to_string()) };
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
                            return make(ts_cand.to_string(), Level::from_str(lv),
                                thread.to_string(), rest[cp+1..].trim().to_string());
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
            if let Some((lv, msg)) = take_bracket(rest2) {
                return make(ts, Level::from_str(lv), module.trim().to_string(), msg.to_string());
            }
        }
    }
    make(String::new(), Level::Debug, String::new(), s.to_string())
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max.saturating_sub(1)]) }
}

// ─── Column widths ────────────────────────────────────────────────────────────

const COL_LN:  f32 = 54.0;
const COL_TS:  f32 = 96.0;
const COL_DT:  f32 = 76.0;
const COL_LV:  f32 = 46.0;
const COL_MOD: f32 = 180.0;

// ─── App struct ───────────────────────────────────────────────────────────────

struct LogViewerApp {
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
    dark_mode: bool,
}

impl Default for LogViewerApp {
    fn default() -> Self {
        Self {
            all_lines: vec![], filtered: vec![],
            filter_text: String::new(), show: [true; 5],
            module_filter: String::new(), modules: vec![],
            counts: [0; 5], row_height: 20.0, font_size: 12.0, wrap_lines: false,
            selected: None, detail_open: false,
            status: "Ready — drop or open a log file".into(),
            drag_hover: false, current_file: None,
            minimap_levels: vec![],
            scroll_to_offset: None, current_scroll_offset: 0.0, scroll_area_height: 0.0,
            search: SearchState::default(), find_dialog_open: false,
            nav_open: false, nav_entries: vec![],
            nav_show_error: true, nav_show_warning: true, nav_show_teststart: true,
            nav_show_testend: true, nav_show_step: true, nav_show_teardown: true,
            nav_show_custom: true, nav_show_bookmark: true,
            nav_custom_kw: String::new(), nav_custom_kw_buf: String::new(),
            bookmarks: vec![],
            dark_mode: true,
        }
    }
}

// ─── LogViewerApp methods ─────────────────────────────────────────────────────

impl LogViewerApp {
    fn load_text(&mut self, text: &str) {
        self.all_lines = text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| { let mut ln = parse_log_line(l, i+1); ln.ts_ms = parse_timestamp_ms(&ln.timestamp); ln })
            .collect();
        let mut prev_ms: Option<u64> = None;
        for line in &mut self.all_lines {
            line.delta_ms = match (prev_ms, line.ts_ms) {
                (Some(p), Some(c)) => Some(c.saturating_sub(p)), _ => None
            };
            if line.ts_ms.is_some() { prev_ms = line.ts_ms; }
        }
        self.counts = [0; 5];
        let mut mod_set: std::collections::BTreeSet<String> = Default::default();
        for l in &self.all_lines {
            self.counts[l.level.index()] += 1;
            if !l.module.is_empty() { mod_set.insert(l.module.clone()); }
        }
        self.modules = mod_set.into_iter().collect();
        self.module_filter.clear();
        self.selected = None; self.detail_open = false; self.current_scroll_offset = 0.0;
        self.bookmarks.clear();
        self.apply_filters();
    }

    fn load_file(&mut self, path: &PathBuf) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.load_text(&text);
                self.current_file = Some(path.clone());
                self.status = format!("{}  ·  {} lines",
                    path.file_name().unwrap_or_default().to_string_lossy(), self.all_lines.len());
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn apply_filters(&mut self) {
        let show = self.show;
        let filter_lc = self.filter_text.to_lowercase();
        let mf = self.module_filter.clone();
        self.filtered = self.all_lines.iter().enumerate()
            .filter(|(_, l)| {
                show[l.level.index()]
                    && (mf.is_empty() || l.module == mf)
                    && (filter_lc.is_empty() || l.raw.to_lowercase().contains(&filter_lc))
            })
            .map(|(i, _)| i).collect();
        self.minimap_levels = self.filtered.iter()
            .map(|&i| self.all_lines[i].level.index() as u8).collect();
        self.search.find_all(&self.filtered, &self.all_lines);
        self.recompute_nav();
    }

    fn do_find_next(&mut self) {
        if self.search.matches.is_empty() { self.search.find_all(&self.filtered, &self.all_lines); }
        if self.search.matches.is_empty() { self.status = "No matches found".to_string(); return; }
        if let Some(row) = self.search.next() {
            self.scroll_to_offset = Some(row as f32 * self.row_height);
            self.selected = Some(row); self.detail_open = true;
        }
    }

    fn do_find_prev(&mut self) {
        if let Some(row) = self.search.prev() {
            self.scroll_to_offset = Some(row as f32 * self.row_height);
            self.selected = Some(row); self.detail_open = true;
        }
    }

    fn do_find_all_with_results(&mut self) {
        self.search.find_all(&self.filtered, &self.all_lines);
        if self.search.matches.is_empty() {
            self.status = "No matches found".to_string();
        } else {
            self.search.results_panel_open = true;
            self.status = format!("{} matches", self.search.matches.len());
            if let Some(mat) = self.search.matches.first() {
                self.scroll_to_offset = Some(mat.row_idx as f32 * self.row_height);
                self.selected = Some(mat.row_idx);
            }
        }
    }

    fn toggle_bookmark(&mut self, row_idx: usize) {
        if let Some(pos) = self.bookmarks.iter().position(|&r| r == row_idx) { self.bookmarks.remove(pos); }
        else { self.bookmarks.push(row_idx); self.bookmarks.sort_unstable(); }
        self.recompute_nav();
    }

    fn is_bookmarked(&self, row_idx: usize) -> bool {
        self.bookmarks.binary_search(&row_idx).is_ok()
    }

    fn recompute_nav(&mut self) {
        self.nav_entries.clear();
        let kw_lc = self.nav_custom_kw.to_lowercase();
        for (row_idx, &line_idx) in self.filtered.iter().enumerate() {
            let line = &self.all_lines[line_idx];
            let raw_lc = line.raw.to_lowercase();
            if self.is_bookmarked(row_idx) {
                self.nav_entries.push(NavEntry { kind: NavKind::Bookmark, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) });
            }
            if matches!(line.level, Level::Error) {
                self.nav_entries.push(NavEntry { kind: NavKind::Error, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if matches!(line.level, Level::Warning) {
                self.nav_entries.push(NavEntry { kind: NavKind::Warning, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("test serie started") || raw_lc.contains("test started:")
                || raw_lc.contains("test case started") || raw_lc.contains("testcase start") {
                self.nav_entries.push(NavEntry { kind: NavKind::TestStart, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("test case status") || raw_lc.contains("test serie ended")
                || raw_lc.contains("testcase end") || raw_lc.contains("test result:")
                || (raw_lc.contains("result") && (raw_lc.contains("passed") || raw_lc.contains("failed"))) {
                self.nav_entries.push(NavEntry { kind: NavKind::TestEnd, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("] step ") || raw_lc.contains("[step]")
                || (raw_lc.contains("step ") && matches!(line.level, Level::Info)) {
                self.nav_entries.push(NavEntry { kind: NavKind::Step, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("teardown") || raw_lc.contains("tear down") || raw_lc.contains("cleanup") {
                self.nav_entries.push(NavEntry { kind: NavKind::Teardown, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if !kw_lc.is_empty() && raw_lc.contains(kw_lc.as_str()) {
                self.nav_entries.push(NavEntry { kind: NavKind::Custom, row_idx,
                    line_num: line.num, label: trunc(&line.message, 38) });
            }
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log files", &["log", "txt"])
            .add_filter("All files", &["*"])
            .pick_file()
        { self.load_file(&path); }
    }

    fn clear_file(&mut self) { *self = LogViewerApp::default(); }

    fn export_filtered(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log","txt"]).set_file_name("filtered.log").save_file() {
            let content = self.filtered.iter().filter_map(|&i| self.all_lines.get(i))
                .map(|l| l.raw.as_str()).collect::<Vec<_>>().join("\n");
            match std::fs::write(&path, content) {
                Ok(_)  => self.status = format!("Exported {} lines", self.filtered.len()),
                Err(e) => self.status = format!("Export failed: {e}"),
            }
        }
    }

    fn export_search_results(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text", &["txt"]).add_filter("CSV", &["csv"])
            .set_file_name("search_results.txt").save_file() {
            let is_csv = path.extension().map(|e| e == "csv").unwrap_or(false);
            let content = if is_csv {
                let mut lines = vec!["Line,Level,Module,Match,Context".to_string()];
                for m in &self.search.matches {
                    lines.push(format!("{},{},{},\"{}\",\"{}{}{}\"", m.line_num, m.level.label(), m.module,
                        m.match_text.replace('"',"\"\""), m.context_before.replace('"',"\"\""),
                        m.match_text.replace('"',"\"\""), m.context_after.replace('"',"\"\"")));
                }
                lines.join("\n")
            } else {
                let mut lines = vec![format!("Search: \"{}\"  ({} matches)",
                    self.search.find_what, self.search.matches.len()), "─".repeat(80), String::new()];
                for m in &self.search.matches {
                    let mod_s = if m.module.len() > 12 { &m.module[..12] } else { &m.module };
                    lines.push(format!("Line {:>5} │ {:>4} │ {:>12} │ {}[{}]{}",
                        m.line_num, m.level.label(), mod_s, m.context_before, m.match_text, m.context_after));
                }
                lines.join("\n")
            };
            match std::fs::write(&path, content) {
                Ok(_)  => self.status = format!("Exported {} matches", self.search.matches.len()),
                Err(e) => self.status = format!("Export failed: {e}"),
            }
        }
    }
}

// ─── UI helpers ───────────────────────────────────────────────────────────────

fn nav_kind_pill(ui: &mut egui::Ui, kind: NavKind) {
    let col = kind.color();
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 28))
        .stroke(Stroke::new(0.5, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 100)))
        .rounding(Rounding::same(3.0))
        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(kind.short_label()).font(FontId::monospace(9.0)).color(col).strong());
        });
}

fn render_match_context(painter: &egui::Painter, pos: egui::Pos2,
    mat: &SearchMatch, max_width: f32, col: &Colors) {
    let font = FontId::monospace(11.0);
    let before = if mat.context_before.len() > 20
        { format!("…{}", &mat.context_before[mat.context_before.len()-20..]) }
        else { mat.context_before.clone() };
    let after  = if mat.context_after.len()  > 30
        { format!("{}…", &mat.context_after[..30]) }
        else { mat.context_after.clone() };
    let match_text = if mat.match_text.len() > 50
        { format!("{}…", &mat.match_text[..49]) }
        else { mat.match_text.clone() };

    let before_width = painter.layout_no_wrap(before.clone(), font.clone(), col.muted).size().x;
    let match_width  = painter.layout_no_wrap(match_text.clone(), font.clone(), col.match_hl).size().x;
    let mut x = pos.x;
    if !before.is_empty() {
        painter.text(egui::pos2(x, pos.y), Align2::LEFT_CENTER, &before, font.clone(), col.muted);
        x += before_width;
    }
    let mr = egui::Rect::from_min_size(egui::pos2(x-2.0, pos.y-8.0), Vec2::new(match_width+4.0, 16.0));
    painter.rect_filled(mr, Rounding::same(2.0),
        Color32::from_rgba_unmultiplied(col.match_hl.r(), col.match_hl.g(), col.match_hl.b(), 40));
    painter.text(egui::pos2(x, pos.y), Align2::LEFT_CENTER, &match_text, font.clone(), col.match_hl);
    x += match_width;
    if !after.is_empty() && x < pos.x + max_width - 50.0 {
        painter.text(egui::pos2(x, pos.y), Align2::LEFT_CENTER, &after, font.clone(), col.muted);
    }
}

// ─── App::update ─────────────────────────────────────────────────────────────

impl App for LogViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        let col = if self.dark_mode { Colors::dark() } else { Colors::light() };
        ctx.set_visuals(col.visuals(self.dark_mode));

        if let Some(ref path) = self.current_file {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                format!("{} — XTR Log Viewer", path.file_name().unwrap_or_default().to_string_lossy())));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("XTR Log Viewer".to_string()));
        }

        // Drag & drop
        ctx.input(|i| {
            self.drag_hover = !i.raw.hovered_files.is_empty();
            for d in &i.raw.dropped_files {
                if let Some(p) = d.path.clone() { self.load_file(&p); }
                else if let Some(b) = &d.bytes { if let Ok(t) = std::str::from_utf8(b) { self.load_text(t); } }
            }
        });

        // Keyboard shortcuts
        ctx.input(|i| {
            if i.key_pressed(Key::O) && i.modifiers.ctrl { self.open_file_dialog(); }
            if i.key_pressed(Key::F) && i.modifiers.ctrl { self.find_dialog_open = true; }
            if i.key_pressed(Key::N) && i.modifiers.ctrl { self.nav_open = !self.nav_open; }
            if i.key_pressed(Key::B) && i.modifiers.ctrl { if let Some(s) = self.selected { self.toggle_bookmark(s); } }
            if i.key_pressed(Key::W) && i.modifiers.ctrl { self.wrap_lines = !self.wrap_lines; }
        });

        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                if self.search.results_panel_open { self.search.results_panel_open = false; }
                else if self.find_dialog_open     { self.find_dialog_open = false; }
                else if !self.filter_text.is_empty() { self.filter_text.clear(); self.apply_filters(); }
                else { self.selected = None; self.detail_open = false; }
            }
            if i.key_pressed(Key::F3) {
                if i.modifiers.shift { self.do_find_prev(); } else { self.do_find_next(); }
            }
        });

        // ════════════════════════════════════════════════════════════════════
        // MENU BAR
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::top("menubar")
            .exact_height(28.0)
            .frame(egui::Frame::none()
                .fill(col.bg_panel)
                .stroke(Stroke::new(1.0, col.border)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add_space(8.0);

                    let mb = |label: &str| {
                        RichText::new(label).font(FontId::proportional(12.0)).color(col.muted)
                    };

                    ui.menu_button(mb("File"), |ui| {
                        ui.set_min_width(210.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        if ui.button("📂  Open…  Ctrl+O").clicked() { self.open_file_dialog(); ui.close_menu(); }
                        if ui.button("🔄  Reload").clicked() {
                            if let Some(p) = self.current_file.clone() { self.load_file(&p); } ui.close_menu();
                        }
                        ui.separator();
                        ui.add_enabled_ui(!self.all_lines.is_empty(), |ui| {
                            if ui.button("💾  Export Filtered…").clicked() { self.export_filtered(); ui.close_menu(); }
                            if ui.button("📋  Export Search Results…").clicked() { self.export_search_results(); ui.close_menu(); }
                        });
                        ui.separator();
                        if ui.button("🗑  Clear").clicked() { self.clear_file(); ui.close_menu(); }
                        ui.separator();
                        if ui.button("✕  Exit  Alt+F4").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                    });

                    ui.menu_button(mb("Help"), |ui| {
                        ui.set_min_width(240.0);
                        ui.add_space(4.0);
                        ui.label(RichText::new("KEYBOARD SHORTCUTS").font(FontId::monospace(9.5)).color(col.faint));
                        ui.add_space(6.0);
                        for (k, v) in [
                            ("Ctrl+O",   "Open file"),
                            ("Ctrl+F",   "Find dialog"),
                            ("F3",       "Find next"),
                            ("Shift+F3", "Find previous"),
                            ("Ctrl+N",   "Navigation panel"),
                            ("Ctrl+B",   "Toggle bookmark"),
                            ("Ctrl+W",   "Toggle line wrap"),
                            ("Esc",      "Close / clear"),
                            ("Dbl-click","Bookmark row"),
                        ] {
                            ui.horizontal(|ui| {
                                ui.add_sized([95.0, 20.0], egui::Label::new(
                                    RichText::new(k).font(FontId::monospace(10.0)).color(col.accent)));
                                ui.label(RichText::new(v).font(FontId::proportional(11.0)).color(col.text));
                            });
                        }
                        ui.add_space(4.0);
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // TOOLBAR  — single unified bar
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::top("toolbar")
            .exact_height(50.0)
            .frame(egui::Frame::none()
                .fill(col.bg_panel)
                .stroke(Stroke::new(1.0, col.border))
                .inner_margin(egui::Margin { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 }))
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    // ── Filter input with clear button ────────────────────────
                    let filter_active = !self.filter_text.is_empty();
                    let te = ui.add(
                        TextEdit::singleline(&mut self.filter_text)
                            .id(egui::Id::new("toolbar_filter"))
                            .hint_text(RichText::new("🔍  Filter log…").color(col.faint))
                            .desired_width(200.0)
                            .font(FontId::monospace(12.5))
                            .frame(true),
                    );
                    if te.changed() { self.apply_filters(); }

                    // Tiny ✕ clear button only when filter is active
                    if filter_active {
                        if ui.add(
                            Button::new(RichText::new("✕").color(col.muted).font(FontId::proportional(11.0)))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(20.0, 28.0))
                        ).on_hover_text("Clear filter").clicked() {
                            self.filter_text.clear();
                            self.apply_filters();
                        }
                    }

                    // ── Module combo ──────────────────────────────────────────
                    if !self.modules.is_empty() {
                        ui.add(egui::Separator::default().vertical().spacing(4.0));
                        let lbl = if self.module_filter.is_empty() { "All modules".to_string() }
                            else if self.module_filter.len() > 18
                                { format!("…{}", &self.module_filter[self.module_filter.len()-16..]) }
                            else { self.module_filter.clone() };
                        let mut changed = false;
                        egui::ComboBox::from_id_source("mod_cb")
                            .selected_text(RichText::new(lbl).font(FontId::proportional(11.5)).color(col.text))
                            .width(148.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(self.module_filter.is_empty(),
                                    RichText::new("All modules").color(col.muted).font(FontId::proportional(11.5))).clicked() {
                                    self.module_filter.clear(); changed = true;
                                }
                                for m in self.modules.clone() {
                                    let d = if m.len() > 26 { format!("…{}", &m[m.len()-24..]) } else { m.clone() };
                                    if ui.selectable_label(self.module_filter == m,
                                        RichText::new(d).font(FontId::proportional(11.5))).clicked() {
                                        self.module_filter = m; changed = true;
                                    }
                                }
                            });
                        if changed { self.apply_filters(); }
                    }

                    ui.add(egui::Separator::default().vertical().spacing(4.0));

                    // ── Level toggles ─────────────────────────────────────────
                    let defs: [(usize,&str,Color32); 5] = [
                        (0,"ERR",Level::Error.color()),
                        (1,"WRN",Level::Warning.color()),
                        (2,"INF",Level::Info.color()),
                        (3,"DBG",Level::Debug.color()),
                        (4,"TRC",Level::Trace.color()),
                    ];
                    let mut fc = false;
                    for (idx, lbl, lv_color) in defs {
                        let active = self.show[idx];
                        let (fg, bg, stroke) = if active {
                            (lv_color,
                             Color32::from_rgba_unmultiplied(lv_color.r(), lv_color.g(), lv_color.b(), 22),
                             Stroke::new(1.0, Color32::from_rgba_unmultiplied(lv_color.r(), lv_color.g(), lv_color.b(), 140)))
                        } else {
                            (col.faint, Color32::TRANSPARENT, Stroke::new(0.5, col.border))
                        };
                        if ui.add(Button::new(
                            RichText::new(format!("{} {}", lbl, self.counts[idx]))
                                .color(fg).font(FontId::monospace(11.0)).strong(),
                        ).fill(bg).stroke(stroke).rounding(Rounding::same(6.0))
                            .min_size(Vec2::new(0.0, 28.0))).clicked()
                        { self.show[idx] = !self.show[idx]; fc = true; }
                    }
                    if fc { self.apply_filters(); }

                    // ── Right-side controls ───────────────────────────────────
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Theme toggle
                        let theme_icon = if self.dark_mode { "☀" } else { "🌙" };
                        let theme_tip  = if self.dark_mode { "Switch to light mode" } else { "Switch to dark mode" };
                        if ui.add(
                            Button::new(RichText::new(theme_icon).color(col.muted).font(FontId::proportional(14.0)))
                                .fill(col.bg_input)
                                .stroke(Stroke::new(0.5, col.border))
                                .rounding(Rounding::same(7.0))
                                .min_size(Vec2::new(32.0, 30.0))
                        ).on_hover_text(theme_tip).clicked() {
                            self.dark_mode = !self.dark_mode;
                        }

                        ui.add(egui::Separator::default().vertical().spacing(4.0));

                        // Font size
                        for (icon, delta, tip) in [("A+", 1.0_f32, "Larger font"), ("A-", -1.0, "Smaller font")] {
                            if ui.add(
                                Button::new(RichText::new(icon).color(col.muted).font(FontId::proportional(12.0)))
                                    .fill(col.bg_input)
                                    .stroke(Stroke::new(0.5, col.border))
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(32.0, 30.0))
                            ).on_hover_text(tip).clicked() {
                                self.font_size = (self.font_size + delta).clamp(9.0, 20.0);
                                self.row_height = self.font_size + 8.0;
                            }
                        }

                        ui.add(egui::Separator::default().vertical().spacing(4.0));

                        // Nav toggle
                        let nav_active = self.nav_open;
                        let nav_lbl = if !self.nav_entries.is_empty()
                            { format!("Nav  {}", self.nav_entries.len()) } else { "Nav".into() };
                        if ui.add(
                            Button::new(RichText::new(nav_lbl)
                                .color(if nav_active { col.accent } else { col.muted })
                                .font(FontId::proportional(11.5)))
                                .fill(if nav_active {
                                    Color32::from_rgba_unmultiplied(col.accent.r(), col.accent.g(), col.accent.b(), 22)
                                } else { col.bg_input })
                                .stroke(Stroke::new(if nav_active { 1.0 } else { 0.5 },
                                    if nav_active {
                                        Color32::from_rgba_unmultiplied(col.accent.r(), col.accent.g(), col.accent.b(), 140)
                                    } else { col.border }))
                                .rounding(Rounding::same(7.0))
                                .min_size(Vec2::new(0.0, 30.0))
                        ).on_hover_text("Navigation panel  Ctrl+N").clicked() {
                            self.nav_open = !self.nav_open;
                        }

                        // Wrap toggle
                        let wrap_active = self.wrap_lines;
                        if ui.add(
                            Button::new(RichText::new(if wrap_active { "↩ Wrap" } else { "→ Wrap" })
                                .color(if wrap_active { col.accent } else { col.muted })
                                .font(FontId::proportional(11.5)))
                                .fill(col.bg_input)
                                .stroke(Stroke::new(0.5, if wrap_active {
                                    Color32::from_rgba_unmultiplied(col.accent.r(), col.accent.g(), col.accent.b(), 100)
                                } else { col.border }))
                                .rounding(Rounding::same(7.0))
                                .min_size(Vec2::new(0.0, 30.0))
                        ).on_hover_text("Toggle line wrap  Ctrl+W").clicked() {
                            self.wrap_lines = !self.wrap_lines;
                        }

                        ui.add(egui::Separator::default().vertical().spacing(4.0));

                        // Open / Clear
                        if self.all_lines.is_empty() {
                            if ui.add(
                                Button::new(RichText::new("📂  Open File")
                                    .strong().color(col.bg_base).font(FontId::proportional(12.0)))
                                    .fill(col.accent)
                                    .stroke(Stroke::NONE)
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(0.0, 30.0))
                            ).clicked() { self.open_file_dialog(); }
                        } else {
                            if ui.add(
                                Button::new(RichText::new("📂  Open")
                                    .color(col.text).font(FontId::proportional(11.5)))
                                    .fill(col.bg_input)
                                    .stroke(Stroke::new(0.5, col.border))
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(0.0, 30.0))
                            ).clicked() { self.open_file_dialog(); }

                            if ui.add(
                                Button::new(RichText::new("✕  Clear")
                                    .color(col.muted).font(FontId::proportional(11.5)))
                                    .fill(col.bg_input)
                                    .stroke(Stroke::new(0.5, col.border))
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(0.0, 30.0))
                            ).clicked() { self.clear_file(); }
                        }
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // FIND DIALOG  (floating window, not a panel — order doesn't matter)
        // ════════════════════════════════════════════════════════════════════
        self.render_find_dialog(ctx, &col);

        // Bottom panels are stacked bottom-to-top in declaration order:
        //   1. statusbar  — always at the very bottom edge
        //   2. detail     — above statusbar, pinned to bottom
        //   3. results    — above detail, closest to the log area

        // ════════════════════════════════════════════════════════════════════
        // STATUS BAR  (1 — very bottom)
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(26.0)
            .frame(egui::Frame::none()
                .fill(col.bg_panel)
                .stroke(Stroke::new(1.0, col.border))
                .inner_margin(egui::Margin::symmetric(14.0, 0.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let mk = |n: usize, s: &str, c: Color32|
                        RichText::new(format!("{} {}", n, s)).color(c).font(FontId::monospace(10.5));
                    ui.label(mk(self.counts[0],"errors",   Level::Error.color()));
                    ui.label(mk(self.counts[1],"warnings", Level::Warning.color()));
                    ui.label(mk(self.counts[2],"info",     Level::Info.color()));
                    ui.label(mk(self.counts[3],"debug",    Level::Debug.color()));
                    ui.label(mk(self.counts[4],"trace",    Level::Trace.color()));
                    if !self.search.matches.is_empty() {
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        ui.label(RichText::new(format!("Match {}/{}",
                            self.search.current_match_idx+1, self.search.matches.len()))
                            .color(col.accent).font(FontId::monospace(10.5)));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} / {} lines",
                            self.filtered.len(), self.all_lines.len()))
                            .color(col.faint).font(FontId::monospace(10.5)));
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        ui.label(RichText::new(&self.status).color(col.muted).font(FontId::monospace(10.5)));
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // DETAIL PANEL  (2 — above statusbar)
        // ════════════════════════════════════════════════════════════════════
        if self.detail_open {
            let sel: Option<LogLine> = self.selected
                .and_then(|r| self.filtered.get(r).copied())
                .and_then(|li| self.all_lines.get(li)).cloned();
            if let Some(line) = sel {
                let detail_bg = if self.dark_mode { Color32::from_rgb(11, 15, 22) } else { col.bg_input };
                egui::TopBottomPanel::bottom("detail_panel")
                    .resizable(true).default_height(155.0).min_height(80.0)
                    .frame(egui::Frame::none()
                        .fill(detail_bg)
                        .stroke(Stroke::new(1.0, col.border))
                        .inner_margin(egui::Margin::symmetric(14.0, 10.0)))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("LINE DETAIL")
                                .font(FontId::monospace(9.5)).color(col.faint).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let mut close = false;
                                if ui.add(
                                    Button::new(RichText::new("✕").color(col.muted).font(FontId::proportional(13.0)))
                                        .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)
                                        .rounding(Rounding::same(4.0)).min_size(Vec2::new(28.0, 28.0))
                                ).on_hover_text("Close (Esc)").clicked() { close = true; }

                                let is_bm = self.is_bookmarked(self.selected.unwrap_or(0));
                                if ui.add(
                                    Button::new(RichText::new(if is_bm { "♥ Bookmarked" } else { "♡ Bookmark" })
                                        .color(if is_bm { Color32::from_rgb(255,140,200) } else { col.muted })
                                        .font(FontId::proportional(11.0)))
                                        .fill(col.bg_input)
                                        .stroke(Stroke::new(0.5, col.border))
                                        .rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 26.0))
                                ).on_hover_text("Ctrl+B").clicked() {
                                    if let Some(s) = self.selected { self.toggle_bookmark(s); }
                                }
                                if ui.add(
                                    Button::new(RichText::new("📋").color(col.muted).font(FontId::proportional(13.0)))
                                        .fill(col.bg_input).stroke(Stroke::new(0.5, col.border))
                                        .rounding(Rounding::same(6.0)).min_size(Vec2::new(30.0, 26.0))
                                ).on_hover_text("Copy raw line").clicked() {
                                    ui.output_mut(|o| o.copied_text = line.raw.clone());
                                }
                                if close { self.detail_open = false; }
                            });
                        });
                        ui.add_space(4.0);
                        ui.add(egui::Separator::default().horizontal().spacing(2.0));
                        ui.add_space(4.0);
                        egui::Grid::new("detail_grid").num_columns(4).spacing([20.0,4.0]).show(ui, |ui| {
                            let lbl = |s: &str| RichText::new(s).color(col.faint).font(FontId::monospace(9.5));
                            let val = |s: String| RichText::new(s).color(col.text).font(FontId::monospace(11.0));
                            ui.label(lbl("LINE"));   ui.label(val(line.num.to_string()));
                            ui.label(lbl("LEVEL"));  ui.label(RichText::new(line.level.label()).color(line.level.color()).strong().font(FontId::monospace(11.0)));
                            ui.end_row();
                            ui.label(lbl("TIME"));   ui.label(val(line.timestamp.clone()));
                            ui.label(lbl("Δ TIME")); ui.label(val(line.delta_ms.map(format_delta).unwrap_or_else(||"—".into())));
                            ui.end_row();
                            ui.label(lbl("MODULE")); ui.label(val(line.module.clone()));
                            ui.label(lbl("")); ui.label(val(String::new()));
                            ui.end_row();
                        });
                        ui.add_space(4.0);
                        ui.label(RichText::new("MESSAGE").color(col.faint).font(FontId::monospace(9.5)));
                        ui.add_space(2.0);
                        ScrollArea::vertical().id_source("detail_scroll").max_height(55.0).show(ui, |ui| {
                            ui.label(RichText::new(&line.message).font(FontId::monospace(11.5)).color(col.text));
                        });
                    });
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // RESULTS PANEL  (3 — above detail, closest to the log area)
        // ════════════════════════════════════════════════════════════════════
        self.render_results_panel(ctx, &col);

        // ════════════════════════════════════════════════════════════════════
        // MINIMAP
        // ════════════════════════════════════════════════════════════════════
        if !self.all_lines.is_empty() {
            let n_filt = self.filtered.len(); let row_h = self.row_height;
            let scroll_off = self.current_scroll_offset; let viewport_h = self.scroll_area_height;
            let ml = self.minimap_levels.clone();
            let mut jump_off: Option<f32> = None;

            let mm_bg = if self.dark_mode { Color32::from_rgb(9, 12, 17) } else { Color32::from_rgb(225, 230, 242) };
            const MM: [Color32;5] = [
                Color32::from_rgb(245,95,85), Color32::from_rgb(235,180,55),
                Color32::from_rgb(70,200,95), Color32::from_rgb(95,165,245), Color32::from_rgb(115,120,135),
            ];

            egui::SidePanel::right("minimap_panel").exact_width(30.0).resizable(false)
                .frame(egui::Frame::none().fill(mm_bg))
                .show(ctx, |ui| {
                    let avail = ui.available_rect_before_wrap();
                    let (resp, painter) = ui.allocate_painter(avail.size(), Sense::click_and_drag());
                    let r = resp.rect;
                    painter.rect_filled(r, Rounding::ZERO, mm_bg);
                    painter.rect_filled(egui::Rect::from_min_max(r.left_top(), egui::pos2(r.min.x+1.0, r.max.y)), Rounding::ZERO, col.border);
                    if n_filt == 0 { return; }
                    let (bx0, bx1, by0, ah) = (r.min.x+2.0, r.max.x-2.0, r.min.y, r.height());
                    for py in 0..ah as usize {
                        let i0 = ((py as f32*n_filt as f32/ah) as usize).min(n_filt-1);
                        let i1 = (((py+1) as f32*n_filt as f32/ah) as usize).min(n_filt-1).max(i0);
                        let bucket = (i1-i0+1) as f32;
                        let mut counts = [0u16;5];
                        for i in i0..=i1 { counts[ml[i] as usize] += 1; }
                        let dom = (0..5).find(|&l| counts[l] as f32/bucket >= 0.20)
                            .unwrap_or_else(|| counts.iter().enumerate()
                                .max_by(|(ia,&ca),(ib,&cb)| ca.cmp(&cb).then(ib.cmp(ia)))
                                .map(|(i,_)| i).unwrap_or(4));
                        let y0 = by0 + py as f32;
                        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(bx0,y0),egui::pos2(bx1,y0+1.5)), Rounding::ZERO, MM[dom]);
                    }
                    let total_h = n_filt as f32 * row_h;
                    if total_h > 0.0 && viewport_h > 0.0 {
                        let vt = (scroll_off/total_h).clamp(0.0,1.0);
                        let vb = ((scroll_off+viewport_h)/total_h).clamp(0.0,1.0);
                        let wy0 = (by0+vt*ah).min(r.max.y-4.0);
                        let wy1 = (by0+vb*ah).clamp(wy0+4.0, r.max.y);
                        painter.rect(
                            egui::Rect::from_min_max(egui::pos2(r.min.x+0.5,wy0),egui::pos2(r.max.x-0.5,wy1)),
                            Rounding::same(2.0),
                            Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),20),
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),120)));
                    }
                    if resp.dragged() || resp.clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let frac = ((pos.y-by0)/ah).clamp(0.0,1.0);
                            let tr = ((frac*n_filt as f32) as usize).min(n_filt.saturating_sub(1));
                            let mut toff = tr as f32*row_h;
                            if total_h > viewport_h { toff = toff.min(total_h-viewport_h); } else { toff = 0.0; }
                            jump_off = Some(toff);
                        }
                    }
                });
            if let Some(off) = jump_off { self.scroll_to_offset = Some(off); }
        }

        // ════════════════════════════════════════════════════════════════════
        // NAVIGATION PANEL
        // ════════════════════════════════════════════════════════════════════
        if self.nav_open && !self.all_lines.is_empty() {
            let mut jump: Option<usize> = None;
            let mut kw_changed = false;
            let mut new_kw = self.nav_custom_kw_buf.clone();

            let visible_data: Vec<(NavKind,usize,usize,String)> = self.nav_entries.iter()
                .filter(|e| match e.kind {
                    NavKind::Error     => self.nav_show_error,
                    NavKind::Warning   => self.nav_show_warning,
                    NavKind::TestStart => self.nav_show_teststart,
                    NavKind::TestEnd   => self.nav_show_testend,
                    NavKind::Step      => self.nav_show_step,
                    NavKind::Teardown  => self.nav_show_teardown,
                    NavKind::Custom    => self.nav_show_custom,
                    NavKind::Bookmark  => self.nav_show_bookmark,
                })
                .map(|e| (e.kind, e.row_idx, e.line_num, e.label.clone()))
                .collect();

            let nav_bg    = if self.dark_mode { Color32::from_rgb(13,16,24) } else { Color32::from_rgb(230,236,250) };
            let nav_hdr   = if self.dark_mode { Color32::from_rgb(16,20,30) } else { Color32::from_rgb(220,228,244) };
            let nav_flt   = if self.dark_mode { Color32::from_rgb(12,15,22) } else { Color32::from_rgb(235,240,252) };

            egui::SidePanel::right("nav_panel")
                .default_width(220.0).width_range(160.0..=320.0).resizable(true)
                .frame(egui::Frame::none().fill(nav_bg).stroke(Stroke::new(1.0, col.border)))
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

                    egui::Frame::none().fill(nav_hdr).inner_margin(egui::Margin::symmetric(10.0,8.0)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("NAVIGATION").font(FontId::monospace(9.5)).color(col.faint).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("{}", visible_data.len()))
                                    .font(FontId::monospace(9.5)).color(col.muted));
                            });
                        });
                    });
                    ui.add(egui::Separator::default().horizontal().spacing(0.0));

                    egui::Frame::none().fill(nav_flt).inner_margin(egui::Margin::symmetric(10.0,8.0)).show(ui, |ui| {
                        ui.label(RichText::new("SHOW").font(FontId::monospace(9.0)).color(col.faint));
                        ui.add_space(4.0);
                        egui::Grid::new("nav_flt").num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                            macro_rules! cb {
                                ($f:expr, $l:expr, $k:expr) => {{
                                    let c = $k.color();
                                    ui.checkbox(&mut $f, RichText::new($l).color(c).font(FontId::monospace(10.5)));
                                }};
                            }
                            cb!(self.nav_show_error,     "ERR",      NavKind::Error);
                            cb!(self.nav_show_warning,   "WRN",      NavKind::Warning);     ui.end_row();
                            cb!(self.nav_show_teststart, "Test ▶",   NavKind::TestStart);
                            cb!(self.nav_show_testend,   "Test ■",   NavKind::TestEnd);     ui.end_row();
                            cb!(self.nav_show_step,      "Step",     NavKind::Step);
                            cb!(self.nav_show_teardown,  "Teardown", NavKind::Teardown);    ui.end_row();
                            cb!(self.nav_show_custom,    "★ Custom", NavKind::Custom);
                            cb!(self.nav_show_bookmark,  "♥ Bmarks", NavKind::Bookmark);   ui.end_row();
                        });
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Keyword:").font(FontId::monospace(9.5)).color(col.faint));
                            let kr = ui.add(TextEdit::singleline(&mut new_kw)
                                .hint_text("any text").desired_width(f32::INFINITY)
                                .font(FontId::monospace(10.5)));
                            if kr.lost_focus() && new_kw != self.nav_custom_kw { kw_changed = true; }
                        });
                    });
                    ui.add(egui::Separator::default().horizontal().spacing(0.0));

                    if visible_data.is_empty() {
                        ui.add_space(16.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(
                                if self.nav_entries.is_empty()
                                    { "No landmark lines\ndetected." }
                                    else { "All types filtered out." })
                                .font(FontId::proportional(11.0)).color(col.faint));
                        });
                    } else {
                        ScrollArea::vertical().id_source("nav_scroll").auto_shrink(false).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            for (kind, row_idx, line_num, label) in &visible_data {
                                let is_sel = self.selected == Some(*row_idx);
                                let c = kind.color();
                                let sel_bg = Color32::from_rgba_unmultiplied(col.accent.r(), col.accent.g(), col.accent.b(), 24);
                                let ir = egui::Frame::none()
                                    .fill(if is_sel { sel_bg } else { Color32::TRANSPARENT })
                                    .inner_margin(egui::Margin{left:12.0,right:8.0,top:6.0,bottom:6.0})
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let bar = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(3.0,34.0));
                                            ui.painter().rect_filled(bar, Rounding::ZERO,
                                                Color32::from_rgba_unmultiplied(c.r(),c.g(),c.b(),190));
                                            ui.add_space(8.0);
                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 2.0;
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 5.0;
                                                    nav_kind_pill(ui, *kind);
                                                    ui.label(RichText::new(format!("line {}", line_num))
                                                        .font(FontId::monospace(9.0)).color(col.faint));
                                                });
                                                ui.label(RichText::new(label.as_str())
                                                    .font(FontId::monospace(10.5)).color(col.text));
                                            });
                                        });
                                    }).response;

                                let interact = ui.interact(ir.rect, egui::Id::new(("nav_e",*row_idx)), Sense::click());
                                if interact.hovered() && !is_sel {
                                    ui.painter().rect_filled(ir.rect, Rounding::ZERO,
                                        Color32::from_rgba_unmultiplied(255,255,255,6));
                                }
                                if interact.clicked() { jump = Some(*row_idx); }
                                ui.add(egui::Separator::default().horizontal().spacing(0.0));
                            }
                        });
                    }
                });

            self.nav_custom_kw_buf = new_kw.clone();
            if kw_changed { self.nav_custom_kw = new_kw; self.recompute_nav(); }
            if let Some(row) = jump {
                self.scroll_to_offset = Some(row as f32 * self.row_height);
                self.selected = Some(row); self.detail_open = true;
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // MAIN LOG AREA
        // ════════════════════════════════════════════════════════════════════
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(col.bg_base))
            .show(ctx, |ui| {
                if self.all_lines.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            let top = (ui.available_height()-170.0).max(0.0)/2.0;
                            ui.add_space(top);
                            ui.label(RichText::new("📄").size(48.0));
                            ui.add_space(10.0);
                            ui.label(RichText::new("Drop a log file here").size(22.0).color(col.text));
                            ui.add_space(8.0);
                            ui.label(RichText::new("Better readability · Test output · Log exploration")
                                .size(13.0).color(col.muted));
                            ui.add_space(28.0);
                            if ui.add(
                                Button::new(RichText::new("  📂  Open File  ")
                                    .strong().color(col.bg_base).font(FontId::proportional(13.0)))
                                    .fill(col.accent).stroke(Stroke::NONE)
                                    .rounding(Rounding::same(8.0)).min_size(Vec2::new(0.0, 34.0))
                            ).clicked() { self.open_file_dialog(); }
                            ui.add_space(18.0);
                            ui.label(RichText::new("Ctrl+O  open  ·  Ctrl+F  find  ·  Ctrl+N  nav  ·  Esc  clear")
                                .size(11.5).color(col.faint));
                        });
                    });
                    return;
                }
                if self.filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("No lines match the current filters")
                            .size(15.0).color(col.faint));
                    });
                    return;
                }

                // Column headers
                {
                    let hh = 20.0;
                    let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), hh), Sense::hover());
                    let p = ui.painter(); let y = hr.center().y; let x0 = hr.min.x;
                    let fid = FontId::monospace(9.5); let hcol = col.faint;
                    p.text(egui::pos2(x0+COL_LN-8.0, y), Align2::RIGHT_CENTER, "#",      fid.clone(), hcol);
                    p.text(egui::pos2(x0+COL_LN,     y), Align2::LEFT_CENTER,  "TIME",   fid.clone(), hcol);
                    p.text(egui::pos2(x0+COL_LN+COL_TS,                  y), Align2::LEFT_CENTER, "Δ",      fid.clone(), hcol);
                    p.text(egui::pos2(x0+COL_LN+COL_TS+COL_DT,           y), Align2::LEFT_CENTER, "LVL",    fid.clone(), hcol);
                    p.text(egui::pos2(x0+COL_LN+COL_TS+COL_DT+COL_LV,        y), Align2::LEFT_CENTER, "MODULE", fid.clone(), hcol);
                    p.text(egui::pos2(x0+COL_LN+COL_TS+COL_DT+COL_LV+COL_MOD, y), Align2::LEFT_CENTER, "MESSAGE", fid.clone(), hcol);
                }
                ui.add(egui::Separator::default().horizontal().spacing(1.0));

                let row_h = self.row_height; let font_sz = self.font_size;
                let n = self.filtered.len(); let visible_height = ui.available_height();

                let mut sa = ScrollArea::vertical().id_source("log_scroll").auto_shrink(false)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                if let Some(off) = self.scroll_to_offset.take() {
                    sa = sa.scroll_offset(Vec2::new(0.0, off));
                }

                let out = sa.show_rows(ui, row_h, n, |ui, row_range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    for row_idx in row_range {
                        let line_idx = match self.filtered.get(row_idx) { Some(&i)=>i, None=>continue };
                        let line     = match self.all_lines.get(line_idx) { Some(l)=>l, None=>continue };
                        let is_sel         = self.selected == Some(row_idx);
                        let is_find_match  = self.search.matches.iter().any(|m| m.row_idx == row_idx);
                        let is_cur_find    = is_find_match && self.search.matches
                            .get(self.search.current_match_idx).map(|m| m.row_idx) == Some(row_idx);
                        let is_bookmarked  = self.is_bookmarked(row_idx);
                        let nav_kind: Option<NavKind> = self.nav_entries.iter()
                            .find(|e| e.row_idx==row_idx && e.kind!=NavKind::Bookmark)
                            .map(|e| e.kind);

                        let (row_rect, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row_h), Sense::click());
                        if !ui.is_rect_visible(row_rect) { continue; }

                        let bg = if is_sel          { col.bg_row_sel }
                            else if is_cur_find     { Color32::from_rgba_unmultiplied(col.match_hl.r(),col.match_hl.g(),col.match_hl.b(),45) }
                            else if is_find_match   { Color32::from_rgba_unmultiplied(col.match_hl.r(),col.match_hl.g(),col.match_hl.b(),20) }
                            else if resp.hovered()  { col.bg_row_hover }
                            else if let Some(c) = line.level.row_bg() { c }
                            else                    { Color32::TRANSPARENT };
                        if bg != Color32::TRANSPARENT {
                            ui.painter().rect_filled(row_rect, Rounding::ZERO, bg);
                        }

                        // Bookmark bar
                        if is_bookmarked {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_h)),
                                Rounding::ZERO, Color32::from_rgb(255,140,200));
                        }
                        // Level accent bar
                        if matches!(line.level, Level::Error|Level::Warning) {
                            let xo = if is_bookmarked { 3.0 } else { 0.0 };
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.min.x+xo, row_rect.min.y), Vec2::new(2.5, row_h)),
                                Rounding::ZERO, line.level.color());
                        }
                        // Nav strip
                        if let Some(kind) = nav_kind {
                            let c = kind.color();
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.max.x-3.0, row_rect.min.y), Vec2::new(3.0, row_h)),
                                Rounding::ZERO, Color32::from_rgba_unmultiplied(c.r(),c.g(),c.b(),130));
                        }

                        let p = ui.painter(); let y = row_rect.center().y;
                        let fid = FontId::monospace(font_sz);
                        let fsm = FontId::monospace((font_sz-1.0).max(8.0));
                        let fxs = FontId::monospace((font_sz-2.0).max(7.5));
                        let mut x = row_rect.min.x + if is_bookmarked { 6.0 } else { 4.0 };

                        // Line number
                        p.text(egui::pos2(x+COL_LN-10.0, y), Align2::RIGHT_CENTER,
                            line.num.to_string(), fxs.clone(), col.faint);
                        x += COL_LN;

                        // Timestamp
                        let ts = if line.timestamp.len()>12 { &line.timestamp[..12] } else { &line.timestamp };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, ts, fsm.clone(), col.ts_color);
                        x += COL_TS;

                        // Delta
                        if let Some(dms) = line.delta_ms { if dms > 0 {
                            let dc = if dms>=1000 { Color32::from_rgb(255,200,80) }
                                else if dms>=100 { col.muted } else { col.faint };
                            p.text(egui::pos2(x, y), Align2::LEFT_CENTER, format_delta(dms), fxs.clone(), dc);
                        }}
                        x += COL_DT;

                        // Level
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, line.level.label(), fsm.clone(), line.level.color());
                        x += COL_LV;

                        // Module
                        let md = if line.module.len()>22 { &line.module[..22] } else { &line.module };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, md, fsm.clone(), col.muted);
                        x += COL_MOD;

                        // Message
                        let msg = &line.message;
                        let msg_col = match line.level {
                            Level::Error   => Color32::from_rgb(255,175,165),
                            Level::Warning => Color32::from_rgb(255,218,148),
                            _              => col.text,
                        };
                        let avail_w = row_rect.max.x - x - 8.0;
                        let max_chars = (avail_w / (font_sz*0.6)) as usize;
                        let msg_disp = if msg.len() > max_chars.max(40)
                            { format!("{}…", &msg[..max_chars.max(40).saturating_sub(1)]) }
                            else { msg.clone() };
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
    }
}

// ─── Find dialog ─────────────────────────────────────────────────────────────

impl LogViewerApp {
    fn render_find_dialog(&mut self, ctx: &egui::Context, col: &Colors) {
        if !self.find_dialog_open { return; }

        let screen = ctx.screen_rect();
        let dw = 480.0;
        let pos = egui::pos2((screen.width()-dw)/2.0, 88.0);
        let mut close_req = false;

        Window::new("find_dlg_w")
            .id(egui::Id::new("find_dlg"))
            .fixed_pos(pos).fixed_size([dw, 260.0])
            .collapsible(false).resizable(false).title_bar(false)
            .frame(egui::Frame::none()
                .fill(col.bg_panel)
                .stroke(Stroke::new(1.5, col.border_hl))
                .rounding(Rounding::same(10.0)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // Header bar
                let hh = 44.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), hh), Sense::hover());
                let painter = ui.painter();
                let hdr_bg = if self.dark_mode { Color32::from_rgb(20,25,36) } else { col.bg_input };
                painter.rect_filled(rect, Rounding{nw:10.0,ne:10.0,sw:0.0,se:0.0}, hdr_bg);
                painter.text(egui::pos2(rect.min.x+18.0, rect.center().y), Align2::LEFT_CENTER,
                    "Find", FontId::proportional(14.0), col.text);

                if !self.search.matches.is_empty() {
                    let badge = format!("{} / {}", self.search.current_match_idx+1, self.search.matches.len());
                    let bw = 80.0;
                    let br = egui::Rect::from_center_size(
                        egui::pos2(rect.max.x - bw/2.0 - 46.0, rect.center().y), Vec2::new(bw, 22.0));
                    painter.rect_filled(br, Rounding::same(11.0),
                        Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),35));
                    painter.text(br.center(), Align2::CENTER_CENTER, badge, FontId::monospace(11.0), col.accent);
                }

                let cr = egui::Rect::from_center_size(egui::pos2(rect.max.x-18.0, rect.center().y), Vec2::splat(28.0));
                let cresp = ui.interact(cr, egui::Id::new("dlg_cls"), Sense::click());
                let xcol = if cresp.hovered() { Color32::from_rgb(255,100,100) } else { col.muted };
                painter.text(cr.center(), Align2::CENTER_CENTER, "✕", FontId::proportional(13.0), xcol);
                painter.line_segment([egui::pos2(rect.min.x,rect.max.y), egui::pos2(rect.max.x,rect.max.y)],
                    Stroke::new(1.0, col.border));
                if cresp.clicked() { close_req = true; }

                // Body
                egui::Frame::none()
                    .inner_margin(egui::Margin{left:20.0,right:20.0,top:16.0,bottom:16.0})
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;

                        // Search input
                        let te_resp = ui.add(
                            TextEdit::singleline(&mut self.search.find_what)
                                .hint_text(RichText::new("Search…").color(col.faint))
                                .desired_width(ui.available_width())
                                .font(FontId::monospace(13.0))
                                .frame(true)
                        );
                        if te_resp.changed() {
                            self.search.first_search = true;
                            self.search.find_all(&self.filtered, &self.all_lines);
                        }
                        let enter_pressed = te_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

                        ui.add_space(14.0);

                        // Options
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 18.0;
                            let mut chg = false;
                            chg |= ui.checkbox(&mut self.search.match_case,  "Case").changed();
                            chg |= ui.checkbox(&mut self.search.whole_word,  "Whole word").changed();
                            chg |= ui.checkbox(&mut self.search.wrap_around, "Wrap").changed();
                            chg |= ui.checkbox(&mut self.search.backward,    "Backward").changed();
                            if chg {
                                self.search.first_search = true;
                                self.search.find_all(&self.filtered, &self.all_lines);
                            }
                        });

                        ui.add_space(10.0);

                        // Mode selector
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(RichText::new("Mode:").font(FontId::monospace(10.5)).color(col.faint));
                            ui.add_space(4.0);
                            for (mode, lbl) in [(SearchMode::Normal,"Normal"),
                                (SearchMode::Extended,"Extended (\\n \\t)"),
                                (SearchMode::Regex,"Regex")] {
                                let sel = self.search.mode == mode;
                                if ui.add(Button::new(
                                    RichText::new(lbl).font(FontId::proportional(11.0))
                                        .color(if sel { col.accent } else { col.muted }))
                                    .fill(if sel { Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),22) } else { Color32::TRANSPARENT })
                                    .stroke(if sel { Stroke::new(1.0, Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),100)) } else { Stroke::NONE })
                                    .rounding(Rounding::same(5.0)).min_size(Vec2::new(0.0, 24.0))).clicked() {
                                    self.search.mode = mode;
                                    self.search.find_all(&self.filtered, &self.all_lines);
                                }
                            }
                        });

                        ui.add_space(16.0);

                        // Action buttons
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let has_m = !self.search.matches.is_empty();

                            if ui.add(
                                Button::new(RichText::new("▶  Find Next").strong()
                                    .color(col.bg_base).font(FontId::proportional(12.0)))
                                    .fill(col.accent).stroke(Stroke::NONE)
                                    .rounding(Rounding::same(7.0)).min_size(Vec2::new(110.0, 32.0))
                            ).clicked() || enter_pressed {
                                self.do_find_next();
                            }

                            if ui.add_enabled(has_m,
                                Button::new(RichText::new("◀  Prev").color(col.text).font(FontId::proportional(12.0)))
                                    .fill(col.bg_input).stroke(Stroke::new(1.0, col.border_hl))
                                    .rounding(Rounding::same(7.0)).min_size(Vec2::new(80.0, 32.0))
                            ).clicked() { self.do_find_prev(); }

                            if ui.add(
                                Button::new(RichText::new("☰  Find All").color(col.accent).font(FontId::proportional(12.0)))
                                    .fill(Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),18))
                                    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),80)))
                                    .rounding(Rounding::same(7.0)).min_size(Vec2::new(80.0, 32.0))
                            ).clicked() { self.do_find_all_with_results(); }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(
                                    Button::new(RichText::new("Close").color(col.muted).font(FontId::proportional(11.5)))
                                        .fill(Color32::TRANSPARENT).stroke(Stroke::new(0.5, col.border))
                                        .rounding(Rounding::same(7.0)).min_size(Vec2::new(60.0, 32.0))
                                ).clicked() { close_req = true; }
                            });
                        });

                        // No-match hint
                        if !self.search.find_what.is_empty() && self.search.matches.is_empty() {
                            ui.add_space(10.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(200,60,50,18))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin::symmetric(12.0, 7.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("✗  No matches in current view")
                                        .font(FontId::proportional(11.5))
                                        .color(Color32::from_rgb(255,110,100)));
                                });
                        }
                    });
            });

        if close_req { self.find_dialog_open = false; }
    }

    fn render_results_panel(&mut self, ctx: &egui::Context, col: &Colors) {
        if !self.search.results_panel_open || self.search.matches.is_empty() { return; }

        let mut jump_to: Option<usize> = None;
        let mut close_panel = false;
        let rp_bg  = if self.dark_mode { Color32::from_rgb(10,13,20) } else { col.bg_input };
        let rp_hdr = if self.dark_mode { Color32::from_rgb(14,18,26) } else { col.bg_panel };
        let rp_col = if self.dark_mode { Color32::from_rgb(13,16,22) } else { col.bg_base };

        egui::TopBottomPanel::bottom("results_panel")
            .resizable(true)
            .default_height(self.search.results_panel_height)
            .height_range(100.0..=400.0)
            .frame(egui::Frame::none().fill(rp_bg).stroke(Stroke::new(1.0, col.border)))
            .show(ctx, |ui| {
                self.search.results_panel_height = ui.available_height();

                egui::Frame::none().fill(rp_hdr)
                    .inner_margin(egui::Margin::symmetric(16.0,9.0)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Search Results").font(FontId::proportional(12.5)).color(col.text).strong());
                            ui.add_space(8.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),35))
                                .rounding(Rounding::same(10.0))
                                .inner_margin(egui::Margin::symmetric(10.0,3.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("{} matches", self.search.matches.len()))
                                        .font(FontId::monospace(10.5)).color(col.accent));
                                });
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("for \"{}\"", self.search.find_what))
                                .font(FontId::proportional(11.5)).color(col.muted));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if ui.add(Button::new(RichText::new("✕").color(col.muted))
                                    .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)
                                    .min_size(Vec2::splat(26.0))).clicked() { close_panel = true; }
                                if ui.add(
                                    Button::new(RichText::new("📋 Copy").color(col.muted).font(FontId::proportional(11.5)))
                                        .fill(col.bg_input).stroke(Stroke::new(0.5, col.border))
                                        .rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 26.0))
                                ).clicked() {
                                    let t = self.search.matches.iter()
                                        .map(|m| format!("Line {}: {}", m.line_num, m.match_text))
                                        .collect::<Vec<_>>().join("\n");
                                    ui.output_mut(|o| o.copied_text = t);
                                }
                                if ui.add(
                                    Button::new(RichText::new("💾 Export").color(col.muted).font(FontId::proportional(11.5)))
                                        .fill(col.bg_input).stroke(Stroke::new(0.5, col.border))
                                        .rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 26.0))
                                ).clicked() { self.export_search_results(); }
                            });
                        });
                    });
                ui.add(egui::Separator::default().spacing(0.0));

                egui::Frame::none().fill(rp_col)
                    .inner_margin(egui::Margin::symmetric(16.0,5.0)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (w, lbl) in [(55.0,"LINE"),(46.0,"LVL"),(80.0,"MODULE"),(200.0,"MATCH CONTEXT")] {
                                ui.add_sized([w,16.0], egui::Label::new(
                                    RichText::new(lbl).font(FontId::monospace(9.0)).color(col.faint)));
                            }
                        });
                    });
                ui.add(egui::Separator::default().spacing(0.0));

                ScrollArea::vertical().id_source("results_scroll").auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (idx, mat) in self.search.matches.iter().enumerate() {
                        let is_current = idx == self.search.current_match_idx;
                        let rh = 30.0;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::click());
                        if !ui.is_rect_visible(rect) { continue; }
                        let painter = ui.painter();

                        let row_even_bg = if self.dark_mode { Color32::from_rgb(13,16,22) } else { col.bg_base };
                        let row_odd_bg  = if self.dark_mode { Color32::TRANSPARENT } else { Color32::TRANSPARENT };
                        let bg = if is_current  { Color32::from_rgba_unmultiplied(col.accent.r(),col.accent.g(),col.accent.b(),32) }
                            else if resp.hovered() { if self.dark_mode { Color32::from_rgb(18,22,32) } else { col.bg_panel } }
                            else if idx%2==1    { row_even_bg }
                            else                { row_odd_bg };
                        if bg != Color32::TRANSPARENT { painter.rect_filled(rect, Rounding::ZERO, bg); }

                        if is_current {
                            painter.rect_filled(
                                egui::Rect::from_min_size(rect.min, Vec2::new(3.0,rh)),
                                Rounding::ZERO, col.accent);
                        }
                        let lc = mat.level.color();
                        painter.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(rect.min.x+4.0, rect.min.y+5.0), Vec2::new(2.0, rh-10.0)),
                            Rounding::same(1.0), lc);

                        let y = rect.center().y; let mut x = rect.min.x + 14.0;
                        painter.text(egui::pos2(x+42.0, y), Align2::RIGHT_CENTER,
                            mat.line_num.to_string(), FontId::monospace(10.5), col.muted); x += 60.0;
                        painter.text(egui::pos2(x, y), Align2::LEFT_CENTER,
                            mat.level.label(), FontId::monospace(10.0), lc); x += 46.0;
                        let md = if mat.module.len()>10 { format!("{}…",&mat.module[..9]) } else { mat.module.clone() };
                        painter.text(egui::pos2(x, y), Align2::LEFT_CENTER,
                            &md, FontId::monospace(10.0), col.muted); x += 80.0;
                        render_match_context(painter, egui::pos2(x, y), mat, rect.max.x-x-16.0, col);

                        if resp.clicked()        { self.search.current_match_idx = idx; jump_to = Some(mat.row_idx); }
                        if resp.double_clicked() { self.search.current_match_idx = idx; jump_to = Some(mat.row_idx); close_panel = true; }
                    }
                });
            });

        if close_panel { self.search.results_panel_open = false; }
        if let Some(row) = jump_to {
            self.scroll_to_offset = Some(row as f32 * self.row_height);
            self.selected = Some(row); self.detail_open = true;
        }
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

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
