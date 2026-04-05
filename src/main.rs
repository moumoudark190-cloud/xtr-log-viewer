#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, App, Frame, NativeOptions};
use egui::{
    Align, Align2, Color32, FontId, Key, Layout, Rounding, ScrollArea, Sense, Stroke, Vec2,
    RichText, Button, Window, TextEdit,
};
use std::path::PathBuf;

// ─── Level ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Level {
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl Level {
    fn from_str(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "ERR" | "ERROR"             => Self::Error,
            "WRN" | "WARN" | "WARNING"  => Self::Warning,
            "INF" | "INFO"              => Self::Info,
            "DBG" | "DEBUG"             => Self::Debug,
            "TRC" | "TRACE" | "VERBOSE" => Self::Trace,
            _                           => Self::Debug,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Error   => "ERR",
            Self::Warning => "WRN",
            Self::Info    => "INF",
            Self::Debug   => "DBG",
            Self::Trace   => "TRC",
        }
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
            Self::Error   => Some(Color32::from_rgba_unmultiplied(200, 50,  40, 22)),
            Self::Warning => Some(Color32::from_rgba_unmultiplied(200, 150, 30, 18)),
            _             => None,
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Error => 0, Self::Warning => 1, Self::Info  => 2,
            Self::Debug => 3, Self::Trace   => 4,
        }
    }
}

// ─── Search ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Normal,
    Extended,
    Regex,
}
impl Default for SearchMode { fn default() -> Self { Self::Normal } }

#[derive(Debug, Clone)]
struct SearchMatch {
    row_idx: usize,
    line_idx: usize,
    line_num: usize,
    start_col: usize,
    end_col: usize,
    match_text: String,
    context_before: String,
    context_after: String,
    module: String,
    level: Level,
}

#[derive(Debug, Clone, Default)]
struct SearchState {
    find_what: String,
    match_case: bool,
    whole_word: bool,
    wrap_around: bool,
    backward: bool,
    mode: SearchMode,
    matches: Vec<SearchMatch>,
    current_match_idx: usize,
    results_panel_open: bool,
    results_panel_height: f32,
    first_search: bool,
}

impl SearchState {
    fn new() -> Self {
        Self {
            wrap_around: true,
            results_panel_height: 200.0,
            first_search: true,
            ..Default::default()
        }
    }

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
                        for _ in 0..2 {
                            if let Some(&c) = chars.peek() {
                                if c.is_ascii_hexdigit() { hex.push(chars.next().unwrap()); }
                            }
                        }
                        if let Ok(val) = u8::from_str_radix(&hex, 16) {
                            result.push(val as char);
                        } else {
                            result.push_str("\\x"); result.push_str(&hex);
                        }
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
            let left_ok = abs == 0 || !hay.as_bytes().get(abs.saturating_sub(1))
                .copied().map(|b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false);
            let right_ok = end >= hay.len() || !hay.as_bytes().get(end)
                .copied().map(|b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false);
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
            _ => self.find_what.clone(),
        };
        let needle = if self.match_case { search_text.clone() } else { search_text.to_lowercase() };

        for (row_idx, &line_idx) in filtered.iter().enumerate() {
            let Some(line) = all_lines.get(line_idx) else { continue };
            let hay = if self.match_case { line.raw.clone() } else { line.raw.to_lowercase() };
            let mut start = 0;
            while let Some(pos) = hay[start..].find(&needle) {
                let abs_pos = start + pos;
                let match_end = abs_pos + needle.len();
                if self.whole_word && !self.matches_whole_word(&hay, &needle) {
                    start = abs_pos + 1; continue;
                }
                let before_start = abs_pos.saturating_sub(30);
                let after_end = (match_end + 30).min(line.raw.len());
                self.matches.push(SearchMatch {
                    row_idx, line_idx, line_num: line.num,
                    start_col: abs_pos, end_col: match_end,
                    match_text: line.raw[abs_pos..match_end].to_string(),
                    context_before: if before_start < abs_pos { line.raw[before_start..abs_pos].to_string() } else { String::new() },
                    context_after: if match_end < after_end { line.raw[match_end..after_end].to_string() } else { String::new() },
                    module: line.module.clone(),
                    level: line.level,
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
            if self.wrap_around { self.current_match_idx = self.matches.len() - 1; }
        } else { self.current_match_idx -= 1; }
        Some(self.matches[self.current_match_idx].row_idx)
    }
}

// ─── NavEntry ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavKind {
    Error, Warning, TestStart, TestEnd, Step, Teardown, Custom, Bookmark,
}

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
            NavKind::Error     => "ERR",
            NavKind::Warning   => "WRN",
            NavKind::TestStart => "TST▶",
            NavKind::TestEnd   => "TST■",
            NavKind::Step      => "STP",
            NavKind::Teardown  => "TDN",
            NavKind::Custom    => "★",
            NavKind::Bookmark  => "♥",
        }
    }
}

#[derive(Debug, Clone)]
struct NavEntry {
    kind: NavKind, row_idx: usize, line_num: usize, label: String,
}

// ─── LogLine ─────────────────────────────────────────────────────────────────

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
    let (s_str, frac) = if let Some(dot) = sec_rest.find('.') {
        (&sec_rest[..dot], &sec_rest[dot + 1..])
    } else { (sec_rest, "") };
    let s_str = s_str.trim_end_matches(|c: char| !c.is_ascii_digit());
    let s: u64 = s_str.parse().ok()?;
    let ms: u64 = if frac.is_empty() { 0 } else {
        let n = frac.len().min(3);
        let v: u64 = frac[..n].parse().ok()?;
        match n { 1 => v * 100, 2 => v * 10, _ => v }
    };
    Some(h * 3_600_000 + m * 60_000 + s * 1_000 + ms)
}

fn format_delta(ms: u64) -> String {
    if      ms < 1_000  { format!("+{}ms",           ms) }
    else if ms < 10_000 { format!("+{:.2}s", ms as f64 / 1000.0) }
    else if ms < 60_000 { format!("+{:.1}s", ms as f64 / 1000.0) }
    else { let s = ms / 1_000; format!("+{}m{:02}s", s / 60, s % 60) }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') {
                it.next();
                for nc in it.by_ref() { if nc.is_ascii_alphabetic() { break; } }
            }
        } else { out.push(c); }
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
    let make = |ts: String, level: Level, module: String, message: String| LogLine {
        num, timestamp: ts, ts_ms: None, delta_ms: None, level, module, message, raw: raw.to_string(),
    };

    if s.starts_with('[') {
        if let Some((ts_raw, rest)) = take_bracket(&s) {
            let ts = ts_raw.split_whitespace().nth(1)
                .or_else(|| ts_raw.split_whitespace().next())
                .unwrap_or(ts_raw).to_string();
            if let Some((lv, rest2)) = take_bracket(rest) {
                let level = Level::from_str(lv);
                let (module, message) = if let Some((m, msg)) = take_bracket(rest2) {
                    (m.to_string(), msg.to_string())
                } else { (String::new(), rest2.to_string()) };
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
                        if matches!(lv.to_uppercase().as_str(),
                            "DEBUG"|"INFO"|"WARN"|"WARNING"|"ERROR"|"TRACE") {
                            return make(ts_cand.to_string(), Level::from_str(lv),
                                thread.to_string(), rest[cp + 1..].trim().to_string());
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
    if s.len() <= max { s.to_string() }
    else { format!("{}…", &s[..max.saturating_sub(1)]) }
}

// ─── Color palette ───────────────────────────────────────────────────────────

const BG_BASE:      Color32 = Color32::from_rgb(13, 17, 23);
const BG_PANEL:     Color32 = Color32::from_rgb(18, 22, 30);
const BG_ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 10);
const BG_ROW_SEL:   Color32 = Color32::from_rgba_premultiplied(88, 166, 255, 45);
const COL_BORDER:   Color32 = Color32::from_rgb(36, 42, 52);
const COL_BORDER_HL:Color32 = Color32::from_rgb(58, 68, 84);
const COL_TEXT:     Color32 = Color32::from_rgb(215, 222, 232);
const COL_MUTED:    Color32 = Color32::from_rgb(140, 148, 160);
const COL_FAINT:    Color32 = Color32::from_rgb(82, 88, 100);
const COL_ACCENT:   Color32 = Color32::from_rgb(88, 166, 255);
const COL_MATCH_HL: Color32 = Color32::from_rgb(255, 213, 79);

const COL_LN:  f32 = 54.0;
const COL_TS:  f32 = 96.0;
const COL_DT:  f32 = 76.0;
const COL_LV:  f32 = 46.0;
const COL_MOD: f32 = 180.0;

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill          = BG_PANEL;
    v.window_fill         = BG_BASE;
    v.override_text_color = Some(COL_TEXT);
    v.widgets.inactive.bg_fill   = Color32::from_rgb(24, 30, 40);
    v.widgets.inactive.bg_stroke = Stroke::new(0.5, COL_BORDER);
    v.widgets.hovered.bg_fill    = Color32::from_rgb(34, 42, 54);
    v.widgets.hovered.bg_stroke  = Stroke::new(0.5, COL_BORDER_HL);
    v.widgets.active.bg_fill     = Color32::from_rgb(50, 60, 76);
    v.selection.bg_fill          = Color32::from_rgba_unmultiplied(88, 166, 255, 65);
    v
}

// ─── Small UI helpers ─────────────────────────────────────────────────────────

fn accent_button_ui(text: &str) -> Button<'_> {
    Button::new(RichText::new(text).strong().color(BG_BASE).font(FontId::proportional(12.0)))
        .fill(COL_ACCENT)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(0.0, 30.0))
}

fn ghost_button(text: &str) -> Button<'_> {
    Button::new(RichText::new(text).color(COL_MUTED).font(FontId::proportional(12.0)))
        .fill(Color32::from_rgb(24, 30, 40))
        .stroke(Stroke::new(0.5, COL_BORDER))
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(0.0, 30.0))
}

fn icon_button(icon: &str) -> Button<'_> {
    Button::new(RichText::new(icon).color(COL_MUTED).font(FontId::proportional(14.0)))
        .fill(Color32::from_rgb(24, 30, 40))
        .stroke(Stroke::new(0.5, COL_BORDER))
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(30.0, 30.0))
        .sense(Sense::click())
}

fn close_button() -> Button<'static> {
    Button::new(RichText::new("✕").color(Color32::from_rgb(160, 160, 165)).font(FontId::proportional(13.0)))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(4.0))
        .min_size(Vec2::new(28.0, 28.0))
}

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

fn render_match_context(painter: &egui::Painter, pos: egui::Pos2, mat: &SearchMatch, max_width: f32) {
    let font = FontId::monospace(11.0);
    let before = if mat.context_before.len() > 20 {
        format!("…{}", &mat.context_before[mat.context_before.len()-20..])
    } else { mat.context_before.clone() };
    let after = if mat.context_after.len() > 30 {
        format!("{}…", &mat.context_after[..30])
    } else { mat.context_after.clone() };
    let match_text = if mat.match_text.len() > 50 {
        format!("{}…", &mat.match_text[..49])
    } else { mat.match_text.clone() };

    let before_width = painter.layout_no_wrap(before.clone(), font.clone(), COL_MUTED).size().x;
    let match_width  = painter.layout_no_wrap(match_text.clone(), font.clone(), COL_MATCH_HL).size().x;
    let mut x = pos.x;
    if !before.is_empty() {
        painter.text(egui::pos2(x, pos.y), Align2::LEFT_CENTER, &before, font.clone(), COL_MUTED);
        x += before_width;
    }
    let mr = egui::Rect::from_min_size(egui::pos2(x - 2.0, pos.y - 8.0), Vec2::new(match_width + 4.0, 16.0));
    painter.rect_filled(mr, Rounding::same(2.0), Color32::from_rgba_unmultiplied(255, 213, 79, 45));
    painter.text(egui::pos2(x, pos.y), Align2::LEFT_CENTER, &match_text, font.clone(), COL_MATCH_HL);
    x += match_width;
    if !after.is_empty() && x < pos.x + max_width - 50.0 {
        painter.text(egui::pos2(x, pos.y), Align2::LEFT_CENTER, &after, font.clone(), COL_MUTED);
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

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
        self.all_lines = text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| { let mut ln = parse_log_line(l, i + 1); ln.ts_ms = parse_timestamp_ms(&ln.timestamp); ln })
            .collect();
        let mut prev_ms: Option<u64> = None;
        for line in &mut self.all_lines {
            line.delta_ms = match (prev_ms, line.ts_ms) {
                (Some(p), Some(c)) => Some(c.saturating_sub(p)), _ => None,
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
        self.minimap_levels = self.filtered.iter().map(|&i| self.all_lines[i].level.index() as u8).collect();
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
        if let Some(pos) = self.bookmarks.iter().position(|&r| r == row_idx) {
            self.bookmarks.remove(pos);
        } else {
            self.bookmarks.push(row_idx); self.bookmarks.sort_unstable();
        }
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
                self.nav_entries.push(NavEntry { kind: NavKind::Bookmark, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
            }
            if matches!(line.level, Level::Error) {
                self.nav_entries.push(NavEntry { kind: NavKind::Error, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if matches!(line.level, Level::Warning) {
                self.nav_entries.push(NavEntry { kind: NavKind::Warning, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("test serie started") || raw_lc.contains("test started:") || raw_lc.contains("test case started") || raw_lc.contains("testcase start") {
                self.nav_entries.push(NavEntry { kind: NavKind::TestStart, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("test case status") || raw_lc.contains("test serie ended") || raw_lc.contains("testcase end") || raw_lc.contains("test result:") || (raw_lc.contains("result") && (raw_lc.contains("passed") || raw_lc.contains("failed"))) {
                self.nav_entries.push(NavEntry { kind: NavKind::TestEnd, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("] step ") || raw_lc.contains("[step]") || (raw_lc.contains("step ") && matches!(line.level, Level::Info)) {
                self.nav_entries.push(NavEntry { kind: NavKind::Step, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if raw_lc.contains("teardown") || raw_lc.contains("tear down") || raw_lc.contains("cleanup") || raw_lc.contains("---teardown---") {
                self.nav_entries.push(NavEntry { kind: NavKind::Teardown, row_idx, line_num: line.num, label: trunc(&line.message, 38) }); continue;
            }
            if !kw_lc.is_empty() && raw_lc.contains(kw_lc.as_str()) {
                self.nav_entries.push(NavEntry { kind: NavKind::Custom, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
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
            .add_filter("Log files", &["log", "txt"])
            .set_file_name("filtered.log")
            .save_file()
        {
            let content: String = self.filtered.iter()
                .filter_map(|&i| self.all_lines.get(i))
                .map(|l| l.raw.as_str()).collect::<Vec<_>>().join("\n");
            match std::fs::write(&path, content) {
                Ok(_)  => self.status = format!("Exported {} lines", self.filtered.len()),
                Err(e) => self.status = format!("Export failed: {e}"),
            }
        }
    }

    fn export_search_results(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text files", &["txt"])
            .add_filter("CSV files", &["csv"])
            .set_file_name("search_results.txt")
            .save_file()
        {
            let is_csv = path.extension().map(|e| e == "csv").unwrap_or(false);
            let content = if is_csv {
                let mut lines = vec!["Line,Level,Module,Match,Context".to_string()];
                for mat in &self.search.matches {
                    lines.push(format!("{},{},{},\"{}\",\"{}{}{}\"",
                        mat.line_num, mat.level.label(), mat.module,
                        mat.match_text.replace('"', "\"\""),
                        mat.context_before.replace('"', "\"\""),
                        mat.match_text.replace('"', "\"\""),
                        mat.context_after.replace('"', "\"\"")));
                }
                lines.join("\n")
            } else {
                let mut lines = vec![
                    format!("Search Results: \"{}\"", self.search.find_what),
                    format!("Total matches: {}", self.search.matches.len()),
                    String::new(), "─".repeat(80), String::new(),
                ];
                for mat in &self.search.matches {
                    lines.push(format!("Line {:>5} │ {:>4} │ {:>12} │ {}{}{}",
                        mat.line_num, mat.level.label(),
                        if mat.module.len() > 12 { &mat.module[..12] } else { &mat.module },
                        mat.context_before, mat.match_text, mat.context_after));
                }
                lines.join("\n")
            };
            match std::fs::write(&path, content) {
                Ok(_) => self.status = format!("Exported {} matches", self.search.matches.len()),
                Err(e) => self.status = format!("Export failed: {e}"),
            }
        }
    }
}

// ─── App::update ─────────────────────────────────────────────────────────────

impl App for LogViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        ctx.set_visuals(dark_visuals());

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
        });

        ctx.input(|i| {
            if i.key_pressed(Key::O) && i.modifiers.ctrl { self.open_file_dialog(); }
            if i.key_pressed(Key::F) && i.modifiers.ctrl { self.find_dialog_open = true; }
            if i.key_pressed(Key::N) && i.modifiers.ctrl { self.nav_open = !self.nav_open; }
            if i.key_pressed(Key::B) && i.modifiers.ctrl { if let Some(sel) = self.selected { self.toggle_bookmark(sel); } }
            if i.key_pressed(Key::W) && i.modifiers.ctrl { self.wrap_lines = !self.wrap_lines; }
        });

        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                if self.search.results_panel_open { self.search.results_panel_open = false; }
                else if self.find_dialog_open { self.find_dialog_open = false; }
                else if !self.filter_text.is_empty() { self.filter_text.clear(); self.apply_filters(); }
                else { self.selected = None; self.detail_open = false; }
            }
            if i.key_pressed(Key::F3) {
                if i.modifiers.shift { self.do_find_prev(); } else { self.do_find_next(); }
            }
        });

        // ── MODERN UNIFIED TOP BAR (Menubar + Toolbar) ─────────────────────────────
        egui::TopBottomPanel::top("top_bar")
            .exact_height(82.0)
            .frame(egui::Frame::none()
                .fill(Color32::from_rgb(13, 17, 23))
                .stroke(Stroke::new(1.0, COL_BORDER))
                .inner_margin(egui::Margin::symmetric(12.0, 0.0)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // ── Slim modern menubar ─────────────────────────────────────
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_space(4.0);

                        // File menu
                        ui.menu_button(
                            RichText::new("File").font(FontId::proportional(12.5)).color(COL_MUTED),
                        |ui| {
                            ui.set_min_width(210.0);
                            if ui.add(menu_item("📂", "Open…", "Ctrl+O")).clicked() {
                                self.open_file_dialog(); ui.close_menu();
                            }
                            if let Some(path) = &self.current_file {
                                if ui.add(menu_item("🔄", "Reload current file", "")).clicked() {
                                    self.load_file(path); ui.close_menu();
                                }
                            }
                            ui.separator();
                            ui.add_enabled_ui(!self.all_lines.is_empty(), |ui| {
                                if ui.add(menu_item("💾", "Export filtered log", "")).clicked() {
                                    self.export_filtered(); ui.close_menu();
                                }
                                if ui.add(menu_item("📤", "Export search results", "")).clicked() {
                                    self.export_search_results(); ui.close_menu();
                                }
                            });
                            ui.separator();
                            if ui.add(menu_item("🗑", "Clear everything", "")).clicked() {
                                self.clear_file(); ui.close_menu();
                            }
                            ui.separator();
                            if ui.add(menu_item("✕", "Exit", "Alt+F4")).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });

                        // Help menu
                        ui.menu_button(
                            RichText::new("Help").font(FontId::proportional(12.5)).color(COL_MUTED),
                        |ui| {
                            ui.set_min_width(230.0);
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.add_space(4.0);
                            ui.label(RichText::new("KEYBOARD SHORTCUTS").font(FontId::monospace(9.5)).color(COL_FAINT));
                            ui.add_space(6.0);
                            for (k, v) in [
                                ("Ctrl+O",     "Open file"),
                                ("Ctrl+F",     "Find & Replace dialog"),
                                ("F3",         "Find next match"),
                                ("Shift+F3",   "Find previous match"),
                                ("Ctrl+N",     "Toggle navigation panel"),
                                ("Ctrl+B",     "Toggle bookmark on row"),
                                ("Ctrl+W",     "Toggle line wrap"),
                                ("Esc",        "Close dialogs / clear filter"),
                                ("Dbl-click",  "Toggle bookmark on row"),
                            ] {
                                ui.horizontal(|ui| {
                                    ui.set_min_width(220.0);
                                    let kw = painter_shortcut_key(k);
                                    ui.add_sized([90.0, 20.0], egui::Label::new(
                                        RichText::new(kw).font(FontId::monospace(10.0)).color(COL_ACCENT)
                                    ));
                                    ui.label(RichText::new(v).font(FontId::proportional(11.0)).color(COL_TEXT));
                                });
                            }
                            ui.add_space(4.0);
                        });

                        // Current file name (centered title when loaded)
                        if let Some(path) = &self.current_file {
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                                ui.label(RichText::new(name)
                                    .font(FontId::proportional(13.0))
                                    .color(COL_TEXT)
                                    .strong());
                            });
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(ghost_button("📂 Open")).clicked() { self.open_file_dialog(); }
                            if !self.all_lines.is_empty() && ui.add(ghost_button("🗑 Clear")).clicked() {
                                self.clear_file();
                            }
                        });
                    });

                    ui.add(egui::Separator::default().stroke(Stroke::new(1.0, COL_BORDER)));

                    // ── Premium Toolbar ─────────────────────────────────────────────
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;

                        // ── HERO SEARCH BAR ──────────────────────────────────────
                        let search_width = 340.0;
                        let search_height = 36.0;
                        let filter_active = !self.filter_text.is_empty();

                        let (rect, _) = ui.allocate_exact_size(Vec2::new(search_width, search_height), Sense::hover());
                        let painter = ui.painter();

                        let fill = Color32::from_rgb(10, 13, 20);
                        let border = if filter_active { COL_ACCENT } else { COL_BORDER_HL };
                        painter.rect(rect, Rounding::same(10.0), fill, Stroke::new(1.5, border));

                        if filter_active {
                            painter.rect_stroke(rect.shrink(1.0), Rounding::same(9.0),
                                Stroke::new(1.0, Color32::from_rgba_unmultiplied(88, 166, 255, 40)));
                        }

                        painter.text(
                            egui::pos2(rect.min.x + 14.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            "⌕",
                            FontId::proportional(17.0),
                            if filter_active { COL_ACCENT } else { COL_FAINT },
                        );

                        let text_rect = egui::Rect::from_min_max(
                            egui::pos2(rect.min.x + 38.0, rect.min.y + 4.0),
                            egui::pos2(rect.max.x - (if filter_active { 34.0 } else { 12.0 }), rect.max.y - 4.0),
                        );
                        let te_resp = ui.put(text_rect, TextEdit::singleline(&mut self.filter_text)
                            .hint_text(RichText::new("Filter or search the log…").color(COL_FAINT))
                            .frame(false)
                            .font(FontId::monospace(13.5))
                        );
                        if te_resp.changed() { self.apply_filters(); }

                        if filter_active {
                            let clear_rect = egui::Rect::from_center_size(
                                egui::pos2(rect.max.x - 18.0, rect.center().y),
                                Vec2::splat(20.0)
                            );
                            let clear_resp = ui.interact(clear_rect, egui::Id::new("clear_search"), Sense::click());
                            let clear_col = if clear_resp.hovered() { COL_TEXT } else { COL_MUTED };
                            painter.text(clear_rect.center(), Align2::CENTER_CENTER, "✕",
                                FontId::proportional(13.0), clear_col);
                            if clear_resp.clicked() {
                                self.filter_text.clear();
                                self.apply_filters();
                            }
                        }

                        ui.add(egui::Separator::default().vertical().spacing(12.0));

                        // ── Module filter ─────────────────────────────────────
                        if !self.modules.is_empty() {
                            let label = if self.module_filter.is_empty() {
                                RichText::new("All modules").color(COL_MUTED)
                            } else {
                                RichText::new(trunc(&self.module_filter, 22)).color(COL_TEXT)
                            };
                            egui::ComboBox::from_id_source("mod_filter")
                                .width(160.0)
                                .selected_text(label.font(FontId::proportional(12.0)))
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(self.module_filter.is_empty(), "All modules").clicked() {
                                        self.module_filter.clear(); self.apply_filters();
                                    }
                                    for m in &self.modules {
                                        if ui.selectable_label(self.module_filter == *m, trunc(m, 30)).clicked() {
                                            self.module_filter = m.clone(); self.apply_filters();
                                        }
                                    }
                                });
                        }

                        ui.add(egui::Separator::default().vertical().spacing(12.0));

                        // ── Modern Level Pills ─────────────────────────────────────
                        let levels = [
                            (0, "ERR", Level::Error.color()),
                            (1, "WRN", Level::Warning.color()),
                            (2, "INF", Level::Info.color()),
                            (3, "DBG", Level::Debug.color()),
                            (4, "TRC", Level::Trace.color()),
                        ];
                        for (idx, label, color) in levels {
                            let active = self.show[idx];
                            let pill = egui::Frame::none()
                                .fill(if active {
                                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 35)
                                } else {
                                    Color32::from_rgb(24, 30, 40)
                                })
                                .stroke(if active {
                                    Stroke::new(1.5, color)
                                } else {
                                    Stroke::new(1.0, COL_BORDER)
                                })
                                .rounding(Rounding::same(8.0))
                                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(label)
                                            .color(if active { color } else { COL_MUTED })
                                            .font(FontId::monospace(11.5))
                                            .strong());
                                        ui.label(RichText::new(self.counts[idx].to_string())
                                            .color(if active { color } else { COL_FAINT })
                                            .font(FontId::monospace(10.0)));
                                    });
                                }).response;

                            if pill.clicked() {
                                self.show[idx] = !self.show[idx];
                                self.apply_filters();
                            }
                        }

                        // ── Right side controls ─────────────────────────────────────
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;

                            let nav_active = self.nav_open;
                            let nav_text = if self.nav_entries.is_empty() { "Nav" } else { &format!("Nav • {}", self.nav_entries.len()) };
                            if ui.add(
                                Button::new(RichText::new(nav_text).color(if nav_active { COL_ACCENT } else { COL_MUTED }))
                                    .fill(if nav_active { Color32::from_rgba_unmultiplied(88,166,255,22) } else { Color32::from_rgb(24,30,40) })
                                    .stroke(if nav_active { Stroke::new(1.5, COL_ACCENT) } else { Stroke::new(1.0, COL_BORDER) })
                                    .rounding(Rounding::same(8.0))
                                    .min_size(Vec2::new(0.0, 34.0))
                            ).clicked() {
                                self.nav_open = !self.nav_open;
                            }

                            let wrap_text = if self.wrap_lines { "↩ Wrap" } else { "→ No wrap" };
                            if ui.add(
                                Button::new(RichText::new(wrap_text).color(if self.wrap_lines { COL_ACCENT } else { COL_MUTED }))
                                    .fill(Color32::from_rgb(24,30,40))
                                    .stroke(Stroke::new(1.0, if self.wrap_lines { COL_ACCENT } else { COL_BORDER }))
                                    .rounding(Rounding::same(8.0))
                                    .min_size(Vec2::new(0.0, 34.0))
                            ).clicked() {
                                self.wrap_lines = !self.wrap_lines;
                            }

                            ui.add(egui::Separator::default().vertical().spacing(12.0));

                            if ui.add(icon_button("A+")).clicked() {
                                self.font_size = (self.font_size + 1.0).min(20.0);
                                self.row_height = self.font_size + 9.0;
                            }
                            if ui.add(icon_button("A-")).clicked() {
                                self.font_size = (self.font_size - 1.0).max(9.0);
                                self.row_height = self.font_size + 9.0;
                            }
                        });
                    });
                });
            });

        // ── Find Dialog ───────────────────────────────────────────────────────
        self.render_find_dialog(ctx);

        // ── Results panel ─────────────────────────────────────────────────────
        self.render_results_panel(ctx);

        // ── Status bar ────────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(28.0)
            .frame(egui::Frame::none()
                .fill(Color32::from_rgb(14, 18, 26))
                .stroke(Stroke::new(1.0, COL_BORDER))
                .inner_margin(egui::Margin::symmetric(14.0, 0.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let mk = |n: usize, s: &str, c: Color32|
                        RichText::new(format!("{} {}", n, s)).color(c).font(FontId::monospace(10.5));
                    ui.label(mk(self.counts[0], "errors",   Level::Error.color()));
                    ui.label(mk(self.counts[1], "warnings", Level::Warning.color()));
                    ui.label(mk(self.counts[2], "info",     Level::Info.color()));
                    ui.label(mk(self.counts[3], "debug",    Level::Debug.color()));
                    ui.label(mk(self.counts[4], "trace",    Level::Trace.color()));
                    if !self.search.matches.is_empty() {
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        ui.label(RichText::new(format!(
                            "Match {}/{}", self.search.current_match_idx + 1, self.search.matches.len()
                        )).color(COL_ACCENT).font(FontId::monospace(10.5)));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} / {} lines",
                            self.filtered.len(), self.all_lines.len()
                        )).color(COL_FAINT).font(FontId::monospace(10.5)));
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        ui.label(RichText::new(&self.status).color(COL_MUTED).font(FontId::monospace(10.5)));
                    });
                });
            });

        // ── Detail panel ──────────────────────────────────────────────────────
        if self.detail_open {
            let sel: Option<LogLine> = self.selected
                .and_then(|r| self.filtered.get(r).copied())
                .and_then(|li| self.all_lines.get(li)).cloned();

            if let Some(line) = sel {
                egui::TopBottomPanel::bottom("detail_panel")
                    .resizable(true).default_height(155.0).min_height(80.0)
                    .frame(egui::Frame::none()
                        .fill(Color32::from_rgb(11, 15, 22))
                        .stroke(Stroke::new(1.0, COL_BORDER))
                        .inner_margin(egui::Margin::symmetric(14.0, 10.0)))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("LINE DETAIL").font(FontId::monospace(9.5)).color(COL_FAINT).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let mut close = false;
                                if ui.add(close_button()).on_hover_text("Close (Esc)").clicked() { close = true; }
                                let is_bm = self.is_bookmarked(self.selected.unwrap_or(0));
                                let bm_text = if is_bm { "♥ Bookmarked" } else { "♡ Bookmark" };
                                let bm_col  = if is_bm { Color32::from_rgb(255, 140, 200) } else { COL_MUTED };
                                if ui.add(Button::new(RichText::new(bm_text).color(bm_col).font(FontId::proportional(11.0)))
                                    .fill(Color32::from_rgb(24, 30, 40)).stroke(Stroke::new(0.5, COL_BORDER))
                                    .rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 26.0)))
                                    .on_hover_text("Toggle bookmark (Ctrl+B)").clicked() {
                                    if let Some(sel) = self.selected { self.toggle_bookmark(sel); }
                                }
                                if ui.add(icon_button("📋")).on_hover_text("Copy raw line").clicked() {
                                    ui.output_mut(|o| o.copied_text = line.raw.clone());
                                }
                                if close { self.detail_open = false; }
                            });
                        });
                        ui.add_space(5.0);
                        ui.add(egui::Separator::default().horizontal().spacing(2.0));
                        ui.add_space(4.0);
                        egui::Grid::new("detail_grid").num_columns(4).spacing([20.0, 4.0]).show(ui, |ui| {
                            let lbl = |s: &str| RichText::new(s).color(COL_FAINT).font(FontId::monospace(9.5));
                            let val = |s: String| RichText::new(s).color(COL_TEXT).font(FontId::monospace(11.0));
                            ui.label(lbl("LINE")); ui.label(val(line.num.to_string()));
                            ui.label(lbl("LEVEL")); ui.label(RichText::new(line.level.label()).color(line.level.color()).strong().font(FontId::monospace(11.0)));
                            ui.end_row();
                            ui.label(lbl("TIME")); ui.label(val(line.timestamp.clone()));
                            ui.label(lbl("Δ TIME")); ui.label(val(line.delta_ms.map(format_delta).unwrap_or_else(|| "—".into())));
                            ui.end_row();
                            ui.label(lbl("MODULE")); ui.label(val(line.module.clone()));
                            ui.label(lbl("")); ui.label(val(String::new()));
                            ui.end_row();
                        });
                        ui.add_space(5.0);
                        ui.label(RichText::new("MESSAGE").color(COL_FAINT).font(FontId::monospace(9.5)));
                        ui.add_space(2.0);
                        ScrollArea::vertical().id_source("detail_scroll").max_height(55.0).show(ui, |ui| {
                            ui.label(RichText::new(&line.message).font(FontId::monospace(11.5)).color(COL_TEXT));
                        });
                    });
            }
        }

        // ── Minimap ───────────────────────────────────────────────────────────
        if !self.all_lines.is_empty() {
            let n_filt = self.filtered.len();
            let row_h = self.row_height;
            let scroll_off = self.current_scroll_offset;
            let viewport_h = self.scroll_area_height;
            let ml = self.minimap_levels.clone();
            let mut jump_to_offset: Option<f32> = None;

            const MM: [Color32; 5] = [
                Color32::from_rgb(245, 95, 85), Color32::from_rgb(235, 180, 55),
                Color32::from_rgb(70, 200, 95), Color32::from_rgb(95, 165, 245),
                Color32::from_rgb(115, 120, 135),
            ];

            egui::SidePanel::right("minimap_panel").exact_width(30.0).resizable(false)
                .frame(egui::Frame::none().fill(Color32::from_rgb(9, 12, 17)))
                .show(ctx, |ui| {
                    let avail = ui.available_rect_before_wrap();
                    let (resp, painter) = ui.allocate_painter(avail.size(), Sense::click_and_drag());
                    let r = resp.rect;
                    painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(9, 12, 17));
                    painter.rect_filled(
                        egui::Rect::from_min_max(r.left_top(), egui::pos2(r.min.x + 1.0, r.max.y)),
                        Rounding::ZERO, COL_BORDER);
                    if n_filt == 0 { return; }
                    let (bx0, bx1, by0, ah) = (r.min.x + 2.0, r.max.x - 2.0, r.min.y, r.height());
                    for py in 0..ah as usize {
                        let i0 = ((py as f32 * n_filt as f32 / ah) as usize).min(n_filt - 1);
                        let i1 = (((py + 1) as f32 * n_filt as f32 / ah) as usize).min(n_filt - 1).max(i0);
                        let bucket = (i1 - i0 + 1) as f32;
                        let mut counts = [0u16; 5];
                        for i in i0..=i1 { counts[ml[i] as usize] += 1; }
                        let dom = (0..5).find(|&l| counts[l] as f32 / bucket >= 0.20)
                            .unwrap_or_else(|| counts.iter().enumerate()
                                .max_by(|(ia, &ca), (ib, &cb)| ca.cmp(&cb).then(ib.cmp(ia)))
                                .map(|(i, _)| i).unwrap_or(4));
                        let y0 = by0 + py as f32;
                        painter.rect_filled(
                            egui::Rect::from_min_max(egui::pos2(bx0, y0), egui::pos2(bx1, y0 + 1.5)),
                            Rounding::ZERO, MM[dom]);
                    }
                    let total_h = n_filt as f32 * row_h;
                    if total_h > 0.0 && viewport_h > 0.0 {
                        let vt = (scroll_off / total_h).clamp(0.0, 1.0);
                        let vb = ((scroll_off + viewport_h) / total_h).clamp(0.0, 1.0);
                        let wy0 = (by0 + vt * ah).min(r.max.y - 4.0);
                        let wy1 = (by0 + vb * ah).clamp(wy0 + 4.0, r.max.y);
                        painter.rect(
                            egui::Rect::from_min_max(egui::pos2(r.min.x + 0.5, wy0), egui::pos2(r.max.x - 0.5, wy1)),
                            Rounding::same(2.0),
                            Color32::from_rgba_unmultiplied(200, 225, 255, 16),
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 225, 255, 120)));
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

        // ── Navigation panel ─────────────────────────────────────────────────
        if self.nav_open && !self.all_lines.is_empty() {
            let mut jump: Option<usize> = None;
            let mut kw_changed = false;
            let mut new_kw = self.nav_custom_kw_buf.clone();

            let visible_data: Vec<(NavKind, usize, usize, String)> = self.nav_entries.iter()
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

            egui::SidePanel::right("nav_panel")
                .default_width(220.0).width_range(160.0..=320.0).resizable(true)
                .frame(egui::Frame::none()
                    .fill(Color32::from_rgb(13, 16, 24))
                    .stroke(Stroke::new(1.0, COL_BORDER)))
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

                    egui::Frame::none().fill(Color32::from_rgb(16, 20, 30))
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("NAVIGATION").font(FontId::monospace(9.5)).color(COL_FAINT).strong());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{}", visible_data.len())).font(FontId::monospace(9.5)).color(COL_MUTED));
                                });
                            });
                        });
                    ui.add(egui::Separator::default().horizontal().spacing(0.0));

                    egui::Frame::none().fill(Color32::from_rgb(12, 15, 22))
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
                            ui.label(RichText::new("SHOW").font(FontId::monospace(9.0)).color(COL_FAINT));
                            ui.add_space(4.0);
                            egui::Grid::new("nav_flt").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                                macro_rules! cb { ($field:expr, $label:expr, $kind:expr) => {{
                                    let c = $kind.color();
                                    ui.checkbox(&mut $field, RichText::new($label).color(c).font(FontId::monospace(10.5)));
                                }}; }
                                cb!(self.nav_show_error,     "ERR",         NavKind::Error);
                                cb!(self.nav_show_warning,   "WRN",         NavKind::Warning);
                                ui.end_row();
                                cb!(self.nav_show_teststart, "Test ▶",      NavKind::TestStart);
                                cb!(self.nav_show_testend,   "Test ■",      NavKind::TestEnd);
                                ui.end_row();
                                cb!(self.nav_show_step,      "Step",        NavKind::Step);
                                cb!(self.nav_show_teardown,  "Teardown",    NavKind::Teardown);
                                ui.end_row();
                                cb!(self.nav_show_custom,    "★ Custom",    NavKind::Custom);
                                cb!(self.nav_show_bookmark,  "♥ Bookmarks", NavKind::Bookmark);
                                ui.end_row();
                            });
                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(RichText::new("Keyword:").font(FontId::monospace(9.5)).color(COL_FAINT));
                                let kr = ui.add(egui::TextEdit::singleline(&mut new_kw)
                                    .hint_text("any text (Enter)").desired_width(f32::INFINITY).font(FontId::monospace(10.5)));
                                if kr.lost_focus() && new_kw != self.nav_custom_kw { kw_changed = true; }
                            });
                        });
                    ui.add(egui::Separator::default().horizontal().spacing(0.0));

                    if visible_data.is_empty() {
                        ui.add_space(16.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(if self.nav_entries.is_empty() {
                                "No landmark lines\ndetected in this log."
                            } else { "All types are filtered out." })
                            .font(FontId::proportional(11.0)).color(COL_FAINT));
                        });
                    } else {
                        ScrollArea::vertical().id_source("nav_scroll").auto_shrink(false).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                            for (kind, row_idx, line_num, label) in &visible_data {
                                let is_sel = self.selected == Some(*row_idx);
                                let entry_bg = if is_sel { Color32::from_rgba_unmultiplied(88, 166, 255, 24) } else { Color32::TRANSPARENT };
                                let c = kind.color();
                                let item_resp = egui::Frame::none()
                                    .fill(entry_bg)
                                    .inner_margin(egui::Margin { left: 12.0, right: 8.0, top: 6.0, bottom: 6.0 })
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let bar = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(3.0, 34.0));
                                            ui.painter().rect_filled(bar, Rounding::ZERO, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 190));
                                            ui.add_space(5.0);
                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 2.0;
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 5.0;
                                                    nav_kind_pill(ui, *kind);
                                                    ui.label(RichText::new(format!("line {}", line_num)).font(FontId::monospace(9.0)).color(COL_FAINT));
                                                });
                                                ui.label(RichText::new(label.as_str()).font(FontId::monospace(10.5)).color(COL_TEXT));
                                            });
                                        });
                                    }).response;
                                let interact = ui.interact(item_resp.rect, egui::Id::new(("nav_e", *row_idx)), Sense::click());
                                if interact.hovered() && !is_sel {
                                    ui.painter().rect_filled(item_resp.rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(255, 255, 255, 6));
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

        // ── Main log area ─────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_BASE))
            .show(ctx, |ui| {
                if self.all_lines.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            let top_margin = (ui.available_height() - 170.0).max(0.0) / 2.0;
                            ui.add_space(top_margin);
                            ui.label(RichText::new("📄").size(48.0));
                            ui.add_space(10.0);
                            ui.label(RichText::new("Drop a log file here").size(22.0).color(COL_TEXT));
                            ui.add_space(8.0);
                            ui.label(RichText::new("Better readability · Test output · Log exploration")
                                .size(13.0).color(COL_MUTED));
                            ui.add_space(28.0);
                            if ui.add(accent_button_ui("  📂  Open File  ")).clicked() { self.open_file_dialog(); }
                            ui.add_space(20.0);
                            ui.label(RichText::new("Ctrl+O  open   ·   Ctrl+F  find   ·   Ctrl+N  nav   ·   Esc  clear")
                                .size(11.5).color(COL_FAINT));
                        });
                    });
                    return;
                }

                if self.filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("No lines match the current filters").size(15.0).color(COL_FAINT));
                    });
                    return;
                }

                // Column headers
                {
                    let hdr_h = 20.0;
                    let (hdr_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), hdr_h), Sense::hover());
                    let p = ui.painter(); let y = hdr_rect.center().y; let x0 = hdr_rect.min.x;
                    let fid = FontId::monospace(9.5);
                    let col = Color32::from_rgb(110, 118, 134);
                    p.text(egui::pos2(x0 + COL_LN - 8.0, y), Align2::RIGHT_CENTER, "#",       fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN,       y), Align2::LEFT_CENTER,  "TIME",    fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS,           y), Align2::LEFT_CENTER, "Δ",      fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT,  y), Align2::LEFT_CENTER, "LVL",   fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + COL_LV,        y), Align2::LEFT_CENTER, "MODULE",  fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + COL_LV + COL_MOD, y), Align2::LEFT_CENTER, "MESSAGE", fid.clone(), col);
                }
                ui.add(egui::Separator::default().horizontal().spacing(1.0));

                let row_h = self.row_height; let font_sz = self.font_size;
                let n = self.filtered.len(); let visible_height = ui.available_height();

                let mut sa = ScrollArea::vertical().id_source("log_scroll").auto_shrink(false)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                if let Some(off) = self.scroll_to_offset.take() { sa = sa.scroll_offset(Vec2::new(0.0, off)); }

                let out = sa.show_rows(ui, row_h, n, |ui, row_range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    for row_idx in row_range {
                        let line_idx = match self.filtered.get(row_idx) { Some(&i) => i, None => continue };
                        let line     = match self.all_lines.get(line_idx) { Some(l) => l, None => continue };

                        let is_sel           = self.selected == Some(row_idx);
                        let is_find_match    = self.search.matches.iter().any(|m| m.row_idx == row_idx);
                        let is_current_find  = is_find_match &&
                            self.search.matches.get(self.search.current_match_idx).map(|m| m.row_idx) == Some(row_idx);
                        let is_bookmarked    = self.is_bookmarked(row_idx);
                        let nav_kind: Option<NavKind> = self.nav_entries.iter()
                            .find(|e| e.row_idx == row_idx && e.kind != NavKind::Bookmark)
                            .map(|e| e.kind);

                        let (row_rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        if !ui.is_rect_visible(row_rect) { continue; }

                        let bg = if is_sel { BG_ROW_SEL }
                            else if is_current_find { Color32::from_rgba_unmultiplied(255, 175, 35, 52) }
                            else if is_find_match   { Color32::from_rgba_unmultiplied(195, 145, 28, 26) }
                            else if resp.hovered()  { BG_ROW_HOVER }
                            else if let Some(c) = line.level.row_bg() { c }
                            else { Color32::TRANSPARENT };
                        if bg != Color32::TRANSPARENT {
                            ui.painter().rect_filled(row_rect, Rounding::ZERO, bg);
                        }

                        if is_bookmarked {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_h)),
                                Rounding::ZERO, Color32::from_rgb(255, 140, 200));
                        }
                        if matches!(line.level, Level::Error | Level::Warning) {
                            let x_off = if is_bookmarked { 3.0 } else { 0.0 };
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.min.x + x_off, row_rect.min.y), Vec2::new(2.5, row_h)),
                                Rounding::ZERO, line.level.color());
                        }
                        if let Some(kind) = nav_kind {
                            let c = kind.color();
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.max.x - 3.0, row_rect.min.y), Vec2::new(3.0, row_h)),
                                Rounding::ZERO, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 130));
                        }

                        let p = ui.painter(); let y = row_rect.center().y;
                        let fid = FontId::monospace(font_sz);
                        let fsm = FontId::monospace((font_sz - 1.0).max(8.0));
                        let fxs = FontId::monospace((font_sz - 2.0).max(7.5));
                        let mut x = row_rect.min.x + if is_bookmarked { 6.0 } else { 4.0 };

                        p.text(egui::pos2(x + COL_LN - 10.0, y), Align2::RIGHT_CENTER, line.num.to_string(), fxs.clone(), COL_FAINT);
                        x += COL_LN;
                        let ts = if line.timestamp.len() > 12 { &line.timestamp[..12] } else { &line.timestamp };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, ts, fsm.clone(), Color32::from_rgb(150, 205, 255));
                        x += COL_TS;
                        if let Some(dms) = line.delta_ms { if dms > 0 {
                            let dc = if dms >= 1000 { Color32::from_rgb(255, 200, 80) }
                                else if dms >= 100 { Color32::from_rgb(175, 175, 195) }
                                else { Color32::from_rgb(110, 118, 138) };
                            p.text(egui::pos2(x, y), Align2::LEFT_CENTER, format_delta(dms), fxs.clone(), dc);
                        }}
                        x += COL_DT;
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, line.level.label(), fsm.clone(), line.level.color());
                        x += COL_LV;
                        let md = if line.module.len() > 22 { &line.module[..22] } else { &line.module };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, md, fsm.clone(), Color32::from_rgb(170, 178, 195));
                        x += COL_MOD;

                        let msg = &line.message;
                        let msg_col = match line.level {
                            Level::Error   => Color32::from_rgb(255, 175, 165),
                            Level::Warning => Color32::from_rgb(255, 218, 148),
                            _              => Color32::from_rgb(205, 210, 222),
                        };
                        let available_width = row_rect.max.x - x - 8.0;
                        let max_chars = (available_width / (font_sz * 0.6)) as usize;
                        let msg_disp = if msg.len() > max_chars.max(40) {
                            format!("{}…", &msg[..max_chars.max(40).saturating_sub(1)])
                        } else { msg.clone() };
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
    fn render_find_dialog(&mut self, ctx: &egui::Context) {
        if !self.find_dialog_open { return; }

        let screen = ctx.screen_rect();
        let dialog_w = 500.0;
        let dialog_h = 330.0;
        let pos = egui::pos2((screen.width() - dialog_w) / 2.0, 90.0);

        let mut close_req = false;

        Window::new("find_dialog")
            .id(egui::Id::new("find_dlg"))
            .fixed_pos(pos)
            .fixed_size([dialog_w, dialog_h])
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .frame(egui::Frame::none()
                .fill(Color32::from_rgb(16, 20, 28))
                .stroke(Stroke::new(1.0, COL_BORDER_HL))
                .rounding(Rounding::same(12.0)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // ── Header ────────────────────────────────────────────────────
                {
                    let hh = 46.0;
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), hh), Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min, rect.size()),
                        Rounding { nw: 12.0, ne: 12.0, sw: 0.0, se: 0.0 },
                        Color32::from_rgb(20, 25, 35));
                    painter.text(egui::pos2(rect.min.x + 18.0, rect.center().y),
                        Align2::LEFT_CENTER, "⌕", FontId::proportional(20.0), COL_ACCENT);
                    painter.text(egui::pos2(rect.min.x + 42.0, rect.center().y),
                        Align2::LEFT_CENTER, "Find", FontId::proportional(14.0), COL_TEXT);

                    if !self.search.matches.is_empty() {
                        let badge = format!("{} / {}", self.search.current_match_idx + 1, self.search.matches.len());
                        let badge_w = 70.0;
                        let badge_rect = egui::Rect::from_center_size(
                            egui::pos2(rect.max.x - 48.0, rect.center().y), Vec2::new(badge_w, 22.0));
                        painter.rect_filled(badge_rect, Rounding::same(11.0), Color32::from_rgba_unmultiplied(88, 166, 255, 40));
                        painter.text(badge_rect.center(), Align2::CENTER_CENTER, badge,
                            FontId::monospace(11.0), COL_ACCENT);
                    }

                    let close_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.max.x - 16.0, rect.center().y), Vec2::splat(26.0));
                    let close_resp = ui.interact(close_rect, egui::Id::new("dlg_cls"), Sense::click());
                    let x_col = if close_resp.hovered() {
                        painter.rect_filled(close_rect, Rounding::same(5.0), Color32::from_rgb(55, 22, 22));
                        Color32::from_rgb(255, 110, 110)
                    } else { COL_MUTED };
                    painter.text(close_rect.center(), Align2::CENTER_CENTER, "✕",
                        FontId::proportional(12.0), x_col);
                    if close_resp.clicked() { close_req = true; }

                    painter.line_segment([egui::pos2(rect.min.x, rect.max.y), egui::pos2(rect.max.x, rect.max.y)],
                        Stroke::new(1.0, COL_BORDER));
                }

                // ── Body ──────────────────────────────────────────────────────
                egui::Frame::none()
                    .inner_margin(egui::Margin { left: 20.0, right: 20.0, top: 18.0, bottom: 18.0 })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;

                        let input_h = 36.0;
                        let avail_w = ui.available_width();
                        let (input_rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, input_h), Sense::hover());
                        let painter = ui.painter();

                        let is_empty = self.search.find_what.is_empty();
                        let border_col = if !is_empty { COL_ACCENT } else { COL_BORDER_HL };
                        painter.rect(input_rect, Rounding::same(8.0),
                            Color32::from_rgb(9, 12, 18), Stroke::new(1.0, border_col));

                        let te_rect = egui::Rect::from_min_max(
                            egui::pos2(input_rect.min.x + 12.0, input_rect.min.y + 3.0),
                            egui::pos2(input_rect.max.x - 12.0, input_rect.max.y - 3.0),
                        );
                        let te_resp = ui.put(te_rect,
                            TextEdit::singleline(&mut self.search.find_what)
                                .hint_text(RichText::new("Search for…").color(COL_FAINT))
                                .frame(false)
                                .font(FontId::monospace(13.0))
                        );
                        if te_resp.changed() {
                            self.search.first_search = true;
                            self.search.find_all(&self.filtered, &self.all_lines);
                        }
                        let do_next = te_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

                        ui.add_space(16.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 18.0;
                            let mut changed = false;
                            changed |= find_checkbox(ui, &mut self.search.match_case,  "Aa",    "Match case");
                            changed |= find_checkbox(ui, &mut self.search.whole_word,  "\\b",   "Whole word");
                            changed |= find_checkbox(ui, &mut self.search.wrap_around, "↻",     "Wrap around");
                            changed |= find_checkbox(ui, &mut self.search.backward,    "←",     "Backward");
                            if changed {
                                self.search.first_search = true;
                                self.search.find_all(&self.filtered, &self.all_lines);
                            }
                        });

                        ui.add_space(14.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(RichText::new("Mode:").font(FontId::monospace(10.5)).color(COL_FAINT));
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let modes = [
                                (SearchMode::Normal,   "Normal"),
                                (SearchMode::Extended, "Extended (\\n \\t)"),
                                (SearchMode::Regex,    "Regex"),
                            ];
                            for (mode, label) in modes {
                                let selected = self.search.mode == mode;
                                let btn = ui.add(Button::new(
                                    RichText::new(label).font(FontId::proportional(11.0))
                                        .color(if selected { COL_ACCENT } else { COL_MUTED })
                                )
                                .fill(if selected { Color32::from_rgba_unmultiplied(88, 166, 255, 22) } else { Color32::TRANSPARENT })
                                .stroke(if selected { Stroke::new(1.0, Color32::from_rgba_unmultiplied(88, 166, 255, 100)) } else { Stroke::NONE })
                                .rounding(Rounding::same(5.0))
                                .min_size(Vec2::new(0.0, 24.0)));
                                if btn.clicked() {
                                    self.search.mode = mode;
                                    self.search.find_all(&self.filtered, &self.all_lines);
                                }
                                ui.add_space(4.0);
                            }
                        });

                        ui.add_space(18.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            let find_next_btn = ui.add(
                                Button::new(RichText::new("▶  Find Next").strong().color(BG_BASE).font(FontId::proportional(12.0)))
                                    .fill(COL_ACCENT)
                                    .stroke(Stroke::NONE)
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(120.0, 32.0))
                            );
                            if find_next_btn.clicked() || do_next { self.do_find_next(); }

                            let has_matches = !self.search.matches.is_empty();
                            let find_prev_btn = ui.add_enabled(has_matches,
                                Button::new(RichText::new("◀  Previous").color(COL_TEXT).font(FontId::proportional(12.0)))
                                    .fill(Color32::from_rgb(28, 34, 46))
                                    .stroke(Stroke::new(1.0, COL_BORDER_HL))
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(100.0, 32.0))
                            );
                            if find_prev_btn.clicked() { self.do_find_prev(); }

                            if ui.add(
                                Button::new(RichText::new("☰  Find All").color(COL_ACCENT).font(FontId::proportional(12.0)))
                                    .fill(Color32::from_rgba_unmultiplied(88, 166, 255, 18))
                                    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(88, 166, 255, 80)))
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(90.0, 32.0))
                            ).clicked() { self.do_find_all_with_results(); }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(
                                    Button::new(RichText::new("✕  Close").color(COL_MUTED).font(FontId::proportional(11.5)))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::new(0.5, COL_BORDER))
                                        .rounding(Rounding::same(7.0))
                                        .min_size(Vec2::new(70.0, 32.0))
                                ).clicked() { close_req = true; }
                            });
                        });

                        if !self.search.find_what.is_empty() && self.search.matches.is_empty() {
                            ui.add_space(10.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(255, 100, 90, 15))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin::symmetric(12.0, 7.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("✗  No matches in current view")
                                        .font(FontId::proportional(11.5))
                                        .color(Color32::from_rgb(255, 120, 110)));
                                });
                        }
                    });
            });

        if close_req { self.find_dialog_open = false; }
    }

    fn render_results_panel(&mut self, ctx: &egui::Context) {
        if !self.search.results_panel_open || self.search.matches.is_empty() { return; }

        let mut jump_to: Option<usize> = None;
        let mut close_panel = false;

        egui::TopBottomPanel::bottom("results_panel")
            .resizable(true)
            .default_height(self.search.results_panel_height)
            .height_range(100.0..=400.0)
            .frame(egui::Frame::none()
                .fill(Color32::from_rgb(10, 13, 20))
                .stroke(Stroke::new(1.0, COL_BORDER)))
            .show(ctx, |ui| {
                self.search.results_panel_height = ui.available_height();

                egui::Frame::none()
                    .fill(Color32::from_rgb(14, 18, 26))
                    .inner_margin(egui::Margin::symmetric(16.0, 9.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⌕").size(15.0).color(COL_ACCENT));
                            ui.add_space(6.0);
                            ui.label(RichText::new("Search Results").font(FontId::proportional(12.5)).color(COL_TEXT).strong());
                            ui.add_space(8.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(88, 166, 255, 40))
                                .rounding(Rounding::same(10.0))
                                .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("{} matches", self.search.matches.len()))
                                        .font(FontId::monospace(10.5)).color(COL_ACCENT));
                                });
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("for \"{}\"", self.search.find_what))
                                .font(FontId::proportional(11.5)).color(COL_MUTED));

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if ui.add(Button::new(RichText::new("✕").color(COL_MUTED))
                                    .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)
                                    .min_size(Vec2::splat(26.0))).clicked() { close_panel = true; }
                                if ui.add(ghost_button("📋 Copy")).on_hover_text("Copy all to clipboard").clicked() {
                                    let text = self.search.matches.iter()
                                        .map(|m| format!("Line {}: {}", m.line_num, m.match_text))
                                        .collect::<Vec<_>>().join("\n");
                                    ui.output_mut(|o| o.copied_text = text);
                                }
                                if ui.add(ghost_button("💾 Export")).clicked() { self.export_search_results(); }
                            });
                        });
                    });
                ui.add(egui::Separator::default().spacing(0.0));

                egui::Frame::none()
                    .fill(Color32::from_rgb(13, 16, 22))
                    .inner_margin(egui::Margin::symmetric(16.0, 5.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.set_min_width(ui.available_width());
                            for (w, label) in [(55.0, "LINE"), (46.0, "LVL"), (80.0, "MODULE"), (0.0, "MATCH CONTEXT")] {
                                ui.add_sized([if w > 0.0 { w } else { 200.0 }, 16.0],
                                    egui::Label::new(RichText::new(label).font(FontId::monospace(9.0)).color(COL_FAINT)));
                            }
                        });
                    });
                ui.add(egui::Separator::default().spacing(0.0));

                ScrollArea::vertical().id_source("results_scroll").auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    for (idx, mat) in self.search.matches.iter().enumerate() {
                        let is_current = idx == self.search.current_match_idx;
                        let row_h = 30.0;
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        if !ui.is_rect_visible(rect) { continue; }

                        let painter = ui.painter();
                        let bg = if is_current { Color32::from_rgba_unmultiplied(88, 166, 255, 32) }
                            else if response.hovered() { Color32::from_rgb(18, 22, 32) }
                            else if idx % 2 == 1 { Color32::from_rgb(13, 16, 22) }
                            else { Color32::TRANSPARENT };
                        if bg != Color32::TRANSPARENT { painter.rect_filled(rect, Rounding::ZERO, bg); }
                        if is_current {
                            painter.rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(3.0, row_h)),
                                Rounding::ZERO, COL_ACCENT);
                        }
                        let level_color = mat.level.color();
                        painter.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(rect.min.x + 4.0, rect.min.y + 5.0), Vec2::new(2.0, row_h - 10.0)),
                            Rounding::same(1.0), level_color);

                        let y = rect.center().y;
                        let mut x = rect.min.x + 14.0;
                        painter.text(egui::pos2(x + 42.0, y), Align2::RIGHT_CENTER, mat.line_num.to_string(), FontId::monospace(10.5), COL_MUTED);
                        x += 60.0;
                        painter.text(egui::pos2(x, y), Align2::LEFT_CENTER, mat.level.label(), FontId::monospace(10.0), level_color);
                        x += 46.0;
                        let module_display = if mat.module.len() > 10 { format!("{}…", &mat.module[..9]) } else { mat.module.clone() };
                        painter.text(egui::pos2(x, y), Align2::LEFT_CENTER, &module_display, FontId::monospace(10.0), Color32::from_rgb(130, 140, 160));
                        x += 80.0;
                        render_match_context(painter, egui::pos2(x, y), mat, rect.max.x - x - 16.0);

                        if response.clicked() { self.search.current_match_idx = idx; jump_to = Some(mat.row_idx); }
                        if response.double_clicked() { self.search.current_match_idx = idx; jump_to = Some(mat.row_idx); close_panel = true; }
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

// ─── Tiny helpers ─────────────────────────────────────────────────────────────

fn menu_item(icon: &str, label: &str, shortcut: &str) -> Button<'static> {
    let label_text = if shortcut.is_empty() {
        format!("{}  {}", icon, label)
    } else {
        format!("{}  {}  ({})", icon, label, shortcut)
    };
    Button::new(RichText::new(label_text).font(FontId::proportional(12.0)).color(COL_TEXT))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(180.0, 26.0))
}

fn painter_shortcut_key(k: &str) -> String { k.to_string() }

fn find_checkbox(ui: &mut egui::Ui, val: &mut bool, label: &str, tooltip: &str) -> bool {
    let old = *val;
    let resp = ui.add(Button::new(
        RichText::new(label).font(FontId::monospace(11.0))
            .color(if *val { COL_ACCENT } else { COL_MUTED })
    )
    .fill(if *val { Color32::from_rgba_unmultiplied(88, 166, 255, 22) } else { Color32::from_rgb(18, 22, 30) })
    .stroke(Stroke::new(1.0, if *val { Color32::from_rgba_unmultiplied(88, 166, 255, 100) } else { COL_BORDER }))
    .rounding(Rounding::same(5.0))
    .min_size(Vec2::new(0.0, 26.0)))
    .on_hover_text(tooltip);
    if resp.clicked() { *val = !*val; }
    *val != old
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
