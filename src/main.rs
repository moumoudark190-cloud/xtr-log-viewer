#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, App, Frame, NativeOptions};
use egui::{
    Align, Align2, Color32, FontId, Key, Layout, Rounding, ScrollArea, Sense, Stroke, Vec2,
    RichText, Button,
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

// ─── NavEntry ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavKind {
    Error,
    Warning,
    TestStart,
    TestEnd,
    Step,
    Teardown,
    Custom,
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
        }
    }
}

#[derive(Debug, Clone)]
struct NavEntry {
    kind:     NavKind,
    row_idx:  usize,   // index into `filtered`
    line_num: usize,   // 1-based original line number
    label:    String,
}

// ─── LogLine ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LogLine {
    num:       usize,
    timestamp: String,
    ts_ms:     Option<u64>,
    delta_ms:  Option<u64>,
    level:     Level,
    module:    String,
    message:   String,
    raw:       String,
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
    let s:  u64 = s_str.parse().ok()?;
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
        num, timestamp: ts, ts_ms: None, delta_ms: None, level, module, message,
        raw: raw.to_string(),
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
                } else {
                    (String::new(), rest2.to_string())
                };
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

// ─── App ─────────────────────────────────────────────────────────────────────

struct LogViewerApp {
    all_lines:     Vec<LogLine>,
    filtered:      Vec<usize>,
    search:        String,
    search_lc:     String,
    show:          [bool; 5],
    module_filter: String,
    modules:       Vec<String>,
    counts:        [usize; 5],
    row_height:    f32,
    font_size:     f32,
    selected:      Option<usize>,
    detail_open:   bool,
    status:        String,
    drag_hover:    bool,

    minimap_levels:        Vec<u8>,
    scroll_to_offset:      Option<f32>,
    current_scroll_offset: f32,
    scroll_area_height:    f32,

    // ── Find bar ──────────────────────────────────────────────────────────
    find_open:      bool,
    find_term:      String,
    find_case:      bool,
    find_word:      bool,
    find_highlight: bool,
    match_rows:     Vec<usize>,
    current_match:  usize,
    total_matches:  usize,

    // ── Navigation panel ─────────────────────────────────────────────────
    nav_open:           bool,
    nav_entries:        Vec<NavEntry>,
    nav_show_error:     bool,
    nav_show_warning:   bool,
    nav_show_teststart: bool,
    nav_show_testend:   bool,
    nav_show_step:      bool,
    nav_show_teardown:  bool,
    nav_show_custom:    bool,
    nav_custom_kw:      String,   // committed keyword
    nav_custom_kw_buf:  String,   // edit buffer
}

impl Default for LogViewerApp {
    fn default() -> Self {
        Self {
            all_lines: vec![], filtered: vec![],
            search: String::new(), search_lc: String::new(),
            show: [true; 5],
            module_filter: String::new(), modules: vec![],
            counts: [0; 5],
            row_height: 20.0, font_size: 12.0,
            selected: None, detail_open: false,
            status: "Drop a .log / .txt file here or press Ctrl+O".into(),
            drag_hover: false,
            minimap_levels: vec![],
            scroll_to_offset: None,
            current_scroll_offset: 0.0,
            scroll_area_height: 0.0,
            find_open: false,
            find_term: String::new(),
            find_case: false,
            find_word: false,
            find_highlight: true,
            match_rows: vec![],
            current_match: 0,
            total_matches: 0,
            nav_open: false,
            nav_entries: vec![],
            nav_show_error:     true,
            nav_show_warning:   true,
            nav_show_teststart: true,
            nav_show_testend:   true,
            nav_show_step:      true,
            nav_show_teardown:  true,
            nav_show_custom:    true,
            nav_custom_kw:     String::new(),
            nav_custom_kw_buf: String::new(),
        }
    }
}

impl LogViewerApp {
    fn load_text(&mut self, text: &str) {
        self.all_lines = text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                let mut ln = parse_log_line(l, i + 1);
                ln.ts_ms = parse_timestamp_ms(&ln.timestamp);
                ln
            })
            .collect();

        let mut prev_ms: Option<u64> = None;
        for line in &mut self.all_lines {
            line.delta_ms = match (prev_ms, line.ts_ms) {
                (Some(p), Some(c)) => Some(c.saturating_sub(p)),
                _ => None,
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
        self.selected = None;
        self.detail_open = false;
        self.current_scroll_offset = 0.0;
        self.status = format!("Loaded {} lines", self.all_lines.len());
        self.apply_filters();
        self.recompute_matches();
    }

    fn load_file(&mut self, path: &PathBuf) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.load_text(&text);
                self.status = format!(
                    "{}  —  {} lines",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    self.all_lines.len()
                );
            }
            Err(e) => self.status = format!("Error reading file: {e}"),
        }
    }

    fn apply_filters(&mut self) {
        let show = self.show;
        let sl   = self.search_lc.clone();
        let mf   = self.module_filter.clone();
        self.filtered = self.all_lines.iter().enumerate()
            .filter(|(_, l)| {
                show[l.level.index()]
                    && (mf.is_empty() || l.module == mf)
                    && (sl.is_empty() || l.raw.to_lowercase().contains(sl.as_str()))
            })
            .map(|(i, _)| i)
            .collect();

        self.minimap_levels = self.filtered.iter()
            .map(|&i| self.all_lines[i].level.index() as u8)
            .collect();

        self.recompute_matches();
        self.recompute_nav();
    }

    // ── Find ─────────────────────────────────────────────────────────────

    fn recompute_matches(&mut self) {
        self.match_rows.clear();
        self.total_matches = 0;
        if self.find_term.is_empty() { return; }
        let needle = if self.find_case { self.find_term.clone() } else { self.find_term.to_lowercase() };
        for (idx, &li) in self.filtered.iter().enumerate() {
            let raw = &self.all_lines[li].raw;
            let hay = if self.find_case { raw.clone() } else { raw.to_lowercase() };
            let found = if self.find_word {
                let mut start = 0; let mut hit = false;
                while let Some(p) = hay[start..].find(&needle) {
                    let abs = start + p; let end = abs + needle.len();
                    let lc = abs == 0 || !hay.as_bytes().get(abs-1).copied().map(|b| b.is_ascii_alphanumeric()).unwrap_or(false);
                    let rc = end >= hay.len() || !hay.as_bytes().get(end).copied().map(|b| b.is_ascii_alphanumeric()).unwrap_or(false);
                    if lc && rc { hit = true; break; }
                    start = abs + 1;
                }
                hit
            } else { hay.contains(&needle) };
            if found { self.match_rows.push(idx); }
        }
        self.total_matches = self.match_rows.len();
        if self.current_match >= self.total_matches && self.total_matches > 0 { self.current_match = 0; }
    }

    fn jump_to_match(&mut self, idx: usize) {
        if self.total_matches == 0 { return; }
        self.current_match = idx.min(self.total_matches - 1);
        let row = self.match_rows[self.current_match];
        self.scroll_to_offset = Some(row as f32 * self.row_height);
        self.selected = Some(row);
        self.detail_open = true;
    }
    fn next_match(&mut self) { if self.total_matches == 0 { return; } let n = (self.current_match + 1) % self.total_matches; self.jump_to_match(n); }
    fn prev_match(&mut self) { if self.total_matches == 0 { return; } let n = if self.current_match == 0 { self.total_matches - 1 } else { self.current_match - 1 }; self.jump_to_match(n); }
    fn is_match_row(&self, r: usize) -> bool { self.find_highlight && !self.find_term.is_empty() && self.match_rows.binary_search(&r).is_ok() }
    fn is_current_match_row(&self, r: usize) -> bool { self.find_highlight && !self.find_term.is_empty() && self.total_matches > 0 && self.match_rows.get(self.current_match) == Some(&r) }

    // ── Navigation ───────────────────────────────────────────────────────

    fn recompute_nav(&mut self) {
        self.nav_entries.clear();
        let kw_lc = self.nav_custom_kw.to_lowercase();

        for (row_idx, &line_idx) in self.filtered.iter().enumerate() {
            let line = &self.all_lines[line_idx];
            let raw_lc = line.raw.to_lowercase();

            // ERR / WRN — use level, no double-check
            if matches!(line.level, Level::Error) {
                self.nav_entries.push(NavEntry { kind: NavKind::Error, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
                continue;
            }
            if matches!(line.level, Level::Warning) {
                self.nav_entries.push(NavEntry { kind: NavKind::Warning, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
                continue;
            }

            // Test start
            if raw_lc.contains("test serie started") || raw_lc.contains("test started:")
                || raw_lc.contains("test case started") || raw_lc.contains("testcase start")
            {
                self.nav_entries.push(NavEntry { kind: NavKind::TestStart, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
                continue;
            }

            // Test end / result
            if raw_lc.contains("test case status") || raw_lc.contains("test serie ended")
                || raw_lc.contains("testcase end") || raw_lc.contains("test result:")
                || (raw_lc.contains("result") && (raw_lc.contains("passed") || raw_lc.contains("failed")))
            {
                self.nav_entries.push(NavEntry { kind: NavKind::TestEnd, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
                continue;
            }

            // Step markers
            if raw_lc.contains("] step ") || raw_lc.contains("[step]")
                || (raw_lc.contains("step ") && matches!(line.level, Level::Info))
            {
                self.nav_entries.push(NavEntry { kind: NavKind::Step, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
                continue;
            }

            // Teardown / separator
            if raw_lc.contains("teardown") || raw_lc.contains("tear down")
                || raw_lc.contains("cleanup") || raw_lc.contains("---teardown---")
                || (raw_lc.contains("---") && raw_lc.len() < 80)
            {
                self.nav_entries.push(NavEntry { kind: NavKind::Teardown, row_idx, line_num: line.num, label: trunc(&line.message, 38) });
                continue;
            }

            // Custom keyword
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
}

// ─── UI constants ─────────────────────────────────────────────────────────────

const BG_BASE:      Color32 = Color32::from_rgb(13, 17, 23);
const BG_PANEL:     Color32 = Color32::from_rgb(20, 25, 32);
const BG_ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 12);
const BG_ROW_SEL:   Color32 = Color32::from_rgba_premultiplied(88, 166, 255, 48);
const COL_BORDER:   Color32 = Color32::from_rgb(42, 48, 58);
const COL_TEXT:     Color32 = Color32::from_rgb(220, 225, 235);
const COL_MUTED:    Color32 = Color32::from_rgb(150, 155, 165);
const COL_FAINT:    Color32 = Color32::from_rgb(90, 95, 105);
const COL_ACCENT:   Color32 = Color32::from_rgb(88, 166, 255);

const COL_LN:  f32 = 54.0;
const COL_TS:  f32 = 96.0;
const COL_DT:  f32 = 76.0;
const COL_LV:  f32 = 46.0;
const COL_MOD: f32 = 206.0;

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill          = BG_PANEL;
    v.window_fill         = BG_BASE;
    v.override_text_color = Some(COL_TEXT);
    v.widgets.inactive.bg_fill   = Color32::from_rgb(30, 36, 45);
    v.widgets.inactive.bg_stroke = Stroke::new(0.5, COL_BORDER);
    v.widgets.hovered.bg_fill    = Color32::from_rgb(40, 48, 58);
    v.widgets.hovered.bg_stroke  = Stroke::new(0.5, Color32::from_gray(85));
    v.widgets.active.bg_fill     = Color32::from_rgb(55, 65, 80);
    v.selection.bg_fill          = Color32::from_rgba_unmultiplied(88, 166, 255, 70);
    v
}

fn primary_button(text: &str) -> Button<'_> {
    Button::new(RichText::new(text).color(COL_TEXT).font(FontId::proportional(12.0)))
        .fill(Color32::from_rgb(40, 50, 65))
        .stroke(Stroke::new(0.5, COL_BORDER))
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 28.0))
}

fn icon_button(icon: &str) -> Button<'_> {
    Button::new(RichText::new(icon).color(COL_MUTED).font(FontId::proportional(14.0)))
        .fill(Color32::from_rgb(30, 36, 45))
        .stroke(Stroke::new(0.5, COL_BORDER))
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(32.0, 28.0))
        .sense(Sense::click())
}

fn close_button() -> Button<'static> {
    Button::new(RichText::new("✕ Close")
        .color(Color32::from_rgb(255, 140, 130)).font(FontId::proportional(12.0)))
        .fill(Color32::from_rgb(45, 30, 35))
        .stroke(Stroke::new(0.5, Color32::from_rgb(200, 80, 70)))
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 28.0))
}

fn level_toggle(ui: &mut egui::Ui, label: &str, count: usize, active: bool, color: Color32) -> bool {
    let (fg, bg, stroke) = if active {
        (color,
         Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 35),
         Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200)))
    } else {
        (Color32::from_gray(90), Color32::from_rgb(20, 25, 32), Stroke::new(0.5, Color32::from_gray(45)))
    };
    ui.add(Button::new(
        RichText::new(format!("{} {}", label, count)).color(fg).font(FontId::monospace(11.0)).strong(),
    ).fill(bg).stroke(stroke).rounding(Rounding::same(5.0)).min_size(Vec2::new(0.0, 28.0))).clicked()
}

fn find_toggle(ui: &mut egui::Ui, label: &str, tooltip: &str, active: bool) -> bool {
    let (fg, bg, stroke) = if active {
        (COL_ACCENT, Color32::from_rgba_unmultiplied(88,166,255,30), Stroke::new(1.0,Color32::from_rgba_unmultiplied(88,166,255,180)))
    } else {
        (Color32::from_gray(100), Color32::from_rgb(18,23,30), Stroke::new(0.5,Color32::from_gray(40)))
    };
    ui.add(Button::new(RichText::new(label).color(fg).font(FontId::monospace(10.5)).strong())
        .fill(bg).stroke(stroke).rounding(Rounding::same(4.0)).min_size(Vec2::new(28.0, 24.0)))
        .on_hover_text(tooltip).clicked()
}

fn nav_kind_pill(ui: &mut egui::Ui, kind: NavKind) {
    let col = kind.color();
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 30))
        .stroke(Stroke::new(0.5, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 120)))
        .rounding(Rounding::same(3.0))
        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(kind.short_label()).font(FontId::monospace(9.0)).color(col).strong());
        });
}

// ─── App::update ─────────────────────────────────────────────────────────────

impl App for LogViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        ctx.set_visuals(dark_visuals());

        ctx.input(|i| {
            self.drag_hover = !i.raw.hovered_files.is_empty();
            for d in &i.raw.dropped_files {
                if let Some(p) = d.path.clone() { self.load_file(&p); }
                else if let Some(b) = &d.bytes { if let Ok(t) = std::str::from_utf8(b) { self.load_text(t); } }
            }
        });

        if ctx.input(|i| i.key_pressed(Key::O) && i.modifiers.ctrl) { self.open_file_dialog(); }
        if ctx.input(|i| (i.key_pressed(Key::H) && i.modifiers.ctrl)
            || (i.key_pressed(Key::F) && i.modifiers.ctrl && self.find_open))
        { self.find_open = !self.find_open; }
        if ctx.input(|i| i.key_pressed(Key::N) && i.modifiers.ctrl) { self.nav_open = !self.nav_open; }

        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                if self.find_open { self.find_open = false; }
                else if !self.search.is_empty() { self.search.clear(); self.search_lc.clear(); self.apply_filters(); }
                else { self.selected = None; self.detail_open = false; }
            }
            if i.key_pressed(Key::Enter) && self.find_open {
                if i.modifiers.shift { self.prev_match(); } else { self.next_match(); }
            }
        });

        // ════════════════════════════════════════════════════════════════════
        // TOOLBAR
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::none().fill(BG_PANEL).inner_margin(egui::Margin::symmetric(12.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    let search_id = egui::Id::new("search_box");
                    let re = ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .id(search_id)
                            .hint_text(RichText::new("🔍  Search (Ctrl+F)").color(COL_FAINT))
                            .desired_width(260.0)
                            .font(FontId::monospace(12.0))
                            .frame(true),
                    );
                    if re.changed() { self.search_lc = self.search.to_lowercase(); self.apply_filters(); }
                    if ctx.input(|i| i.key_pressed(Key::F) && (i.modifiers.ctrl || i.modifiers.command)) {
                        ctx.memory_mut(|m| m.request_focus(search_id));
                    }

                    // Find bar toggle
                    let fc = if self.find_open { COL_ACCENT } else { COL_MUTED };
                    if ui.add(Button::new(RichText::new("⊞").color(fc).font(FontId::proportional(15.0)))
                        .fill(if self.find_open { Color32::from_rgba_unmultiplied(88,166,255,22) } else { Color32::from_rgb(30,36,45) })
                        .stroke(Stroke::new(if self.find_open {1.0} else {0.5}, if self.find_open { Color32::from_rgba_unmultiplied(88,166,255,160) } else { COL_BORDER }))
                        .rounding(Rounding::same(6.0)).min_size(Vec2::new(32.0, 28.0)))
                        .on_hover_text("Advanced Find  (Ctrl+H)").clicked()
                    { self.find_open = !self.find_open; }

                    if !self.modules.is_empty() {
                        ui.add(egui::Separator::default().vertical().spacing(8.0));
                        let label = if self.module_filter.is_empty() { "All modules".to_string() }
                            else if self.module_filter.len() > 26 { format!("…{}", &self.module_filter[self.module_filter.len().saturating_sub(24)..]) }
                            else { self.module_filter.clone() };
                        let mut changed = false;
                        egui::ComboBox::from_id_source("mod_cb")
                            .selected_text(RichText::new(label).font(FontId::proportional(12.0)).color(COL_TEXT))
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(self.module_filter.is_empty(), "All modules").clicked() { self.module_filter.clear(); changed = true; }
                                for m in self.modules.clone() {
                                    let d = if m.len() > 38 { format!("…{}", &m[m.len()-36..]) } else { m.clone() };
                                    if ui.selectable_label(self.module_filter == m, d).clicked() { self.module_filter = m; changed = true; }
                                }
                            });
                        if changed { self.apply_filters(); }
                    }

                    ui.add(egui::Separator::default().vertical().spacing(8.0));

                    let defs: [(usize, &str, Color32); 5] = [
                        (0,"ERR",Level::Error.color()),(1,"WRN",Level::Warning.color()),
                        (2,"INF",Level::Info.color()),(3,"DBG",Level::Debug.color()),(4,"TRC",Level::Trace.color()),
                    ];
                    let mut fc2 = false;
                    for (idx,lbl,color) in defs { if level_toggle(ui,lbl,self.counts[idx],self.show[idx],color) { self.show[idx]=!self.show[idx]; fc2=true; } }
                    if fc2 { self.apply_filters(); }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if ui.add(primary_button("📂 Open  Ctrl+O")).clicked() { self.open_file_dialog(); }
                        if !self.all_lines.is_empty() {
                            if ui.add(primary_button("🗑 Clear")).clicked() { *self = LogViewerApp::default(); }
                        }
                        ui.add(egui::Separator::default().vertical().spacing(8.0));

                        // Nav panel toggle
                        let nav_col = if self.nav_open { COL_ACCENT } else { COL_MUTED };
                        let nav_lbl = if !self.nav_entries.is_empty() {
                            format!("⊟ Nav  {}", self.nav_entries.len())
                        } else { "⊟ Nav".to_string() };
                        if ui.add(Button::new(RichText::new(nav_lbl).color(nav_col).font(FontId::proportional(12.0)))
                            .fill(if self.nav_open { Color32::from_rgba_unmultiplied(88,166,255,22) } else { Color32::from_rgb(30,36,45) })
                            .stroke(Stroke::new(if self.nav_open {1.0} else {0.5}, if self.nav_open { Color32::from_rgba_unmultiplied(88,166,255,160) } else { COL_BORDER }))
                            .rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 28.0)))
                            .on_hover_text("Navigation panel — landmark lines  (Ctrl+N)").clicked()
                        { self.nav_open = !self.nav_open; }

                        ui.add(egui::Separator::default().vertical().spacing(8.0));
                        if ui.add(icon_button("A+")).on_hover_text("Increase font size").clicked() { self.font_size=(self.font_size+1.0).min(20.0); self.row_height=self.font_size+8.0; }
                        if ui.add(icon_button("A-")).on_hover_text("Decrease font size").clicked() { self.font_size=(self.font_size-1.0).max(9.0); self.row_height=self.font_size+8.0; }
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // FIND BAR
        // ════════════════════════════════════════════════════════════════════
        if self.find_open {
            let (ctr_txt, ctr_col) = if self.find_term.is_empty() { ("No term".to_string(), COL_FAINT) }
                else if self.total_matches == 0 { ("0 results".to_string(), Color32::from_rgb(255,110,90)) }
                else { (format!("{} / {}", self.current_match+1, self.total_matches), COL_ACCENT) };

            let mut opts_changed=false; let mut do_next=false; let mut do_prev=false; let mut do_close=false;
            let mut new_term=self.find_term.clone(); let mut new_case=self.find_case;
            let mut new_word=self.find_word; let mut new_hl=self.find_highlight;

            egui::TopBottomPanel::top("find_bar")
                .frame(egui::Frame::none().fill(Color32::from_rgb(16,21,29))
                    .stroke(Stroke::new(1.0,COL_BORDER))
                    .inner_margin(egui::Margin{left:14.0,right:10.0,top:7.0,bottom:7.0}))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(RichText::new("Find").font(FontId::monospace(10.5)).color(COL_MUTED).strong());

                        let find_id = egui::Id::new("find_bar_input");
                        let istroke = if !new_term.is_empty() && self.total_matches==0 { Stroke::new(1.5,Color32::from_rgb(220,80,70)) }
                            else if !new_term.is_empty() { Stroke::new(1.5,Color32::from_rgba_unmultiplied(88,166,255,200)) }
                            else { Stroke::new(1.0,COL_BORDER) };
                        let te = ui.add(egui::TextEdit::singleline(&mut new_term).id(find_id)
                            .hint_text(RichText::new("Search in log…").color(COL_FAINT))
                            .desired_width(260.0).font(FontId::monospace(12.0)).frame(true));
                        ui.painter().rect_stroke(te.rect, Rounding::same(4.0), istroke);
                        if te.changed() { opts_changed=true; }
                        ctx.memory_mut(|m| m.request_focus(find_id));

                        ui.add_space(2.0);
                        if find_toggle(ui,"Aa","Match case",new_case) { new_case=!new_case; opts_changed=true; }
                        if find_toggle(ui,"W","Whole word",new_word)   { new_word=!new_word; opts_changed=true; }
                        if find_toggle(ui,"🔆","Highlight all",new_hl)  { new_hl=!new_hl;   opts_changed=true; }

                        ui.add_space(4.0);
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        ui.add_space(4.0);

                        let badge_bg = if !new_term.is_empty() && self.total_matches==0 { Color32::from_rgba_unmultiplied(180,50,40,60) } else { Color32::from_rgb(22,28,38) };
                        egui::Frame::none().fill(badge_bg).stroke(Stroke::new(0.5,COL_BORDER)).rounding(Rounding::same(4.0))
                            .inner_margin(egui::Margin::symmetric(8.0,3.0)).show(ui, |ui| {
                                ui.label(RichText::new(&ctr_txt).font(FontId::monospace(10.5)).color(ctr_col).strong());
                            });

                        let nav_ok=self.total_matches>0;
                        if ui.add(Button::new(RichText::new("↑").color(if nav_ok{COL_TEXT}else{COL_FAINT}).font(FontId::monospace(13.0)))
                            .fill(Color32::from_rgb(28,34,44)).stroke(Stroke::new(0.5,COL_BORDER))
                            .rounding(Rounding::same(5.0)).min_size(Vec2::new(28.0,26.0)))
                            .on_hover_text("Previous  (Shift+Enter)").clicked() && nav_ok { do_prev=true; }
                        if ui.add(Button::new(RichText::new("↓").color(if nav_ok{COL_TEXT}else{COL_FAINT}).font(FontId::monospace(13.0)))
                            .fill(Color32::from_rgb(28,34,44)).stroke(Stroke::new(0.5,COL_BORDER))
                            .rounding(Rounding::same(5.0)).min_size(Vec2::new(28.0,26.0)))
                            .on_hover_text("Next  (Enter)").clicked() && nav_ok { do_next=true; }

                        ui.add_space(6.0);
                        ui.label(RichText::new("Enter ↓  Shift+Enter ↑  Esc close").font(FontId::monospace(9.5)).color(COL_FAINT));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(Button::new(RichText::new("✕").color(Color32::from_gray(130)).font(FontId::proportional(12.0)))
                                .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)
                                .rounding(Rounding::same(4.0)).min_size(Vec2::new(26.0,26.0)))
                                .on_hover_text("Close  (Esc)").clicked() { do_close=true; }
                        });
                    });
                });

            if new_term != self.find_term { self.find_term = new_term; }
            if opts_changed || new_case!=self.find_case || new_word!=self.find_word || new_hl!=self.find_highlight {
                self.find_case=new_case; self.find_word=new_word; self.find_highlight=new_hl; self.recompute_matches();
            }
            if do_prev { self.prev_match(); }
            if do_next { self.next_match(); }
            if do_close { self.find_open=false; }
        }

        // ════════════════════════════════════════════════════════════════════
        // STATUS BAR
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::bottom("statusbar")
            .frame(egui::Frame::none().fill(BG_PANEL).stroke(Stroke::new(1.0,COL_BORDER)).inner_margin(egui::Margin::symmetric(12.0,6.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let mk = |n: usize, s: &str, c: Color32| RichText::new(format!("{} {}", n, s)).color(c).font(FontId::monospace(11.0));
                    ui.label(mk(self.counts[0],"errors",Level::Error.color()));
                    ui.label(mk(self.counts[1],"warnings",Level::Warning.color()));
                    ui.label(mk(self.counts[2],"info",Level::Info.color()));
                    ui.label(mk(self.counts[3],"debug",Level::Debug.color()));
                    ui.label(mk(self.counts[4],"trace",Level::Trace.color()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} / {} lines shown",self.filtered.len(),self.all_lines.len())).color(COL_MUTED).font(FontId::monospace(11.0)));
                        ui.add(egui::Separator::default().vertical());
                        ui.label(RichText::new(&self.status).color(COL_MUTED).font(FontId::monospace(11.0)));
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // DETAIL PANEL
        // ════════════════════════════════════════════════════════════════════
        if self.detail_open {
            let sel: Option<LogLine> = self.selected
                .and_then(|r| self.filtered.get(r).copied())
                .and_then(|li| self.all_lines.get(li)).cloned();

            if let Some(line) = sel {
                egui::TopBottomPanel::bottom("detail_panel")
                    .resizable(true).default_height(148.0).min_height(80.0)
                    .frame(egui::Frame::none().fill(Color32::from_rgb(14,19,27))
                        .stroke(Stroke::new(1.0,COL_BORDER)).inner_margin(egui::Margin::symmetric(14.0,10.0)))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("LINE DETAIL").font(FontId::monospace(10.0)).color(COL_FAINT).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let mut close = false;
                                if ui.add(close_button()).on_hover_text("Close detail panel").clicked() { close=true; }
                                if ui.add(icon_button("📋")).on_hover_text("Copy raw line").clicked() {
                                    ui.output_mut(|o| o.copied_text = line.raw.clone());
                                }
                                if close { self.detail_open=false; }
                            });
                        });
                        ui.add_space(5.0);
                        ui.add(egui::Separator::default().horizontal().spacing(3.0));
                        ui.add_space(4.0);
                        egui::Grid::new("detail_grid").num_columns(4).spacing([20.0,5.0]).show(ui, |ui| {
                            let lbl=|s:&str| RichText::new(s).color(COL_FAINT).font(FontId::monospace(10.0));
                            let val=|s:String| RichText::new(s).color(COL_TEXT).font(FontId::monospace(11.0));
                            ui.label(lbl("LINE")); ui.label(val(line.num.to_string()));
                            ui.label(lbl("LEVEL")); ui.label(RichText::new(line.level.label()).color(line.level.color()).strong().font(FontId::monospace(11.0)));
                            ui.end_row();
                            ui.label(lbl("TIME")); ui.label(val(line.timestamp.clone()));
                            ui.label(lbl("Δ TIME")); ui.label(val(line.delta_ms.map(format_delta).unwrap_or_else(||"—".into())));
                            ui.end_row();
                            ui.label(lbl("MODULE")); ui.label(val(line.module.clone()));
                            ui.label(lbl("")); ui.label(val(String::new()));
                            ui.end_row();
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new("MESSAGE").color(COL_FAINT).font(FontId::monospace(10.0)));
                        ui.add_space(3.0);
                        ScrollArea::vertical().id_source("detail_scroll").max_height(55.0).show(ui, |ui| {
                            ui.label(RichText::new(&line.message).font(FontId::monospace(11.5)).color(COL_TEXT));
                        });
                    });
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // MINIMAP  (rightmost fixed strip, always on)
        // ════════════════════════════════════════════════════════════════════
        {
            let n_filt=self.filtered.len(); let row_h=self.row_height;
            let scroll_off=self.current_scroll_offset; let viewport_h=self.scroll_area_height;
            let ml=self.minimap_levels.clone();
            let mut jump_to_offset:Option<f32>=None;

            const MM:[Color32;5]=[Color32::from_rgb(245,95,85),Color32::from_rgb(235,180,55),Color32::from_rgb(70,200,95),Color32::from_rgb(95,165,245),Color32::from_rgb(125,130,145)];

            egui::SidePanel::right("minimap_panel").exact_width(34.0).resizable(false)
                .frame(egui::Frame::none().fill(Color32::from_rgb(10,13,18)))
                .show(ctx, |ui| {
                    let avail=ui.available_rect_before_wrap();
                    let (resp,painter)=ui.allocate_painter(avail.size(),Sense::click_and_drag());
                    let r=resp.rect;
                    painter.rect_filled(r,Rounding::ZERO,Color32::from_rgb(10,13,18));
                    painter.rect_filled(egui::Rect::from_min_max(r.left_top(),egui::pos2(r.min.x+1.0,r.max.y)),Rounding::ZERO,COL_BORDER);
                    if n_filt==0 { return; }
                    let (bx0,bx1,by0,ah)=(r.min.x+3.0,r.max.x-2.0,r.min.y,r.height());
                    for py in 0..ah as usize {
                        let i0=((py as f32*n_filt as f32/ah) as usize).min(n_filt-1);
                        let i1=(((py+1) as f32*n_filt as f32/ah) as usize).min(n_filt-1).max(i0);
                        let bucket=(i1-i0+1) as f32;
                        let mut counts=[0u16;5];
                        for i in i0..=i1 { counts[ml[i] as usize]+=1; }
                        let dom=(0..5).find(|&l| counts[l] as f32/bucket>=0.20)
                            .unwrap_or_else(|| counts.iter().enumerate().max_by(|(ia,&ca),(ib,&cb)| ca.cmp(&cb).then(ib.cmp(ia))).map(|(i,_)| i).unwrap_or(4));
                        let y0=by0+py as f32;
                        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(bx0,y0),egui::pos2(bx1,y0+1.6)),Rounding::ZERO,MM[dom]);
                    }
                    let total_h=n_filt as f32*row_h;
                    if total_h>0.0 && viewport_h>0.0 {
                        let vt=(scroll_off/total_h).clamp(0.0,1.0);
                        let vb=((scroll_off+viewport_h)/total_h).clamp(0.0,1.0);
                        let wy0=(by0+vt*ah).min(r.max.y-4.0);
                        let wy1=(by0+vb*ah).clamp(wy0+4.0,r.max.y);
                        painter.rect(egui::Rect::from_min_max(egui::pos2(r.min.x+1.5,wy0),egui::pos2(r.max.x-1.0,wy1)),
                            Rounding::same(2.0),Color32::from_rgba_unmultiplied(200,225,255,18),Stroke::new(1.0,Color32::from_rgba_unmultiplied(200,225,255,130)));
                    }
                    if resp.dragged()||resp.clicked() {
                        if let Some(pos)=resp.interact_pointer_pos() {
                            let frac=((pos.y-by0)/ah).clamp(0.0,1.0);
                            let tr=(frac*n_filt as f32) as usize;
                            let tr=tr.min(n_filt.saturating_sub(1));
                            let mut toff=tr as f32*row_h;
                            if total_h>viewport_h { toff=toff.min(total_h-viewport_h); } else { toff=0.0; }
                            jump_to_offset=Some(toff);
                        }
                    }
                });
            if let Some(off)=jump_to_offset { self.scroll_to_offset=Some(off); }
        }

        // ════════════════════════════════════════════════════════════════════
        // NAVIGATION PANEL  (resizable, left of minimap, toggled via toolbar)
        // ════════════════════════════════════════════════════════════════════
        if self.nav_open {
            let mut jump: Option<usize> = None;
            let mut kw_changed  = false;
            let mut flt_changed = false;
            let mut new_kw = self.nav_custom_kw_buf.clone();

            // Snapshot filter flags before the mutable borrow below
            let show_err = self.nav_show_error;
            let show_wrn = self.nav_show_warning;
            let show_ts  = self.nav_show_teststart;
            let show_te  = self.nav_show_testend;
            let show_stp = self.nav_show_step;
            let show_tdn = self.nav_show_teardown;
            let show_cst = self.nav_show_custom;

            let visible_data: Vec<(NavKind, usize, usize, String)> = self.nav_entries.iter()
                .filter(|e| match e.kind {
                    NavKind::Error     => show_err,
                    NavKind::Warning   => show_wrn,
                    NavKind::TestStart => show_ts,
                    NavKind::TestEnd   => show_te,
                    NavKind::Step      => show_stp,
                    NavKind::Teardown  => show_tdn,
                    NavKind::Custom    => show_cst,
                })
                .map(|e| (e.kind, e.row_idx, e.line_num, e.label.clone()))
                .collect();

            egui::SidePanel::right("nav_panel")
                .default_width(230.0)
                .width_range(160.0..=340.0)
                .resizable(true)
                .frame(egui::Frame::none()
                    .fill(Color32::from_rgb(15,19,28))
                    .stroke(Stroke::new(1.0,COL_BORDER)))
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0,4.0);

                    // ── Header ────────────────────────────────────────────
                    egui::Frame::none()
                        .fill(Color32::from_rgb(18,23,33))
                        .inner_margin(egui::Margin::symmetric(10.0,8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("NAVIGATION").font(FontId::monospace(10.0)).color(COL_FAINT).strong());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{} entries", visible_data.len()))
                                        .font(FontId::monospace(9.5)).color(COL_MUTED));
                                });
                            });
                        });

                    ui.add(egui::Separator::default().horizontal().spacing(0.0));

                    // ── Filters ───────────────────────────────────────────
                    egui::Frame::none()
                        .fill(Color32::from_rgb(14,18,26))
                        .inner_margin(egui::Margin::symmetric(10.0,8.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("SHOW").font(FontId::monospace(9.0)).color(COL_FAINT));
                            ui.add_space(4.0);

                            egui::Grid::new("nav_flt").num_columns(2).spacing([8.0,4.0]).show(ui, |ui| {
                                macro_rules! cb {
                                    ($field:expr, $label:expr, $kind:expr) => {{
                                        let c=$kind.color();
                                        let r=ui.checkbox(&mut $field, RichText::new($label).color(c).font(FontId::monospace(10.5)));
                                        if r.changed() { flt_changed=true; }
                                    }};
                                }
                                cb!(self.nav_show_error,     "ERR",      NavKind::Error);
                                cb!(self.nav_show_warning,   "WRN",      NavKind::Warning);
                                ui.end_row();
                                cb!(self.nav_show_teststart, "Test ▶",   NavKind::TestStart);
                                cb!(self.nav_show_testend,   "Test ■",   NavKind::TestEnd);
                                ui.end_row();
                                cb!(self.nav_show_step,      "Step",     NavKind::Step);
                                cb!(self.nav_show_teardown,  "Teardown", NavKind::Teardown);
                                ui.end_row();
                                cb!(self.nav_show_custom,    "★ Custom", NavKind::Custom);
                                ui.end_row();
                            });

                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(RichText::new("Keyword:").font(FontId::monospace(9.5)).color(COL_FAINT));
                                let kr = ui.add(egui::TextEdit::singleline(&mut new_kw)
                                    .hint_text("any text  (Enter to apply)")
                                    .desired_width(f32::INFINITY)
                                    .font(FontId::monospace(10.5)));
                                if kr.lost_focus() && new_kw != self.nav_custom_kw { kw_changed=true; }
                            });
                        });

                    ui.add(egui::Separator::default().horizontal().spacing(0.0));

                    // ── Entry list ────────────────────────────────────────
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
                            ui.spacing_mut().item_spacing = Vec2::new(0.0,0.0);

                            for (kind, row_idx, line_num, label) in &visible_data {
                                let is_sel = self.selected == Some(*row_idx);
                                let entry_bg = if is_sel { Color32::from_rgba_unmultiplied(88,166,255,28) } else { Color32::TRANSPARENT };
                                let c = kind.color();

                                let item_resp = egui::Frame::none()
                                    .fill(entry_bg)
                                    .inner_margin(egui::Margin { left: 12.0, right: 8.0, top: 6.0, bottom: 6.0 })
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            // Colored left accent bar
                                            let bar = egui::Rect::from_min_size(
                                                ui.cursor().min,
                                                Vec2::new(3.0, 34.0),
                                            );
                                            ui.painter().rect_filled(bar, Rounding::ZERO,
                                                Color32::from_rgba_unmultiplied(c.r(),c.g(),c.b(),200));
                                            ui.add_space(5.0);

                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 2.0;
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 5.0;
                                                    nav_kind_pill(ui, *kind);
                                                    ui.label(RichText::new(format!("line {}", line_num))
                                                        .font(FontId::monospace(9.0)).color(COL_FAINT));
                                                });
                                                ui.label(RichText::new(label.as_str())
                                                    .font(FontId::monospace(10.5)).color(COL_TEXT));
                                            });
                                        });
                                    }).response;

                                let interact = ui.interact(
                                    item_resp.rect,
                                    egui::Id::new(("nav_e", *row_idx)),
                                    Sense::click(),
                                );
                                if interact.hovered() && !is_sel {
                                    ui.painter().rect_filled(item_resp.rect, Rounding::ZERO,
                                        Color32::from_rgba_unmultiplied(255,255,255,7));
                                }
                                if interact.clicked() { jump = Some(*row_idx); }

                                // thin separator between entries
                                ui.add(egui::Separator::default().horizontal().spacing(0.0));
                            }
                        });
                    }
                });

            // ── Deferred mutations ────────────────────────────────────────
            let _ = flt_changed; // checkboxes already mutated self.nav_show_* directly
            self.nav_custom_kw_buf = new_kw.clone();
            if kw_changed {
                self.nav_custom_kw = new_kw;
                self.recompute_nav();
            }
            if let Some(row) = jump {
                self.scroll_to_offset = Some(row as f32 * self.row_height);
                self.selected = Some(row);
                self.detail_open = true;
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // MAIN LOG AREA
        // ════════════════════════════════════════════════════════════════════
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_BASE))
            .show(ctx, |ui| {
                if self.all_lines.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            let top_margin=(ui.available_height()-120.0).max(0.0)/2.0;
                            ui.add_space(top_margin);
                            ui.label(RichText::new("Drop a log file here").size(22.0).color(COL_MUTED));
                            ui.add_space(8.0);
                            ui.label(RichText::new("Better readability for trace analysis · Test output visualization · Log exploration made easy").size(12.0).color(COL_FAINT));
                            ui.add_space(22.0);
                            if ui.add(Button::new(RichText::new("  Open file  (Ctrl+O)  ").size(13.0).color(BG_BASE)).fill(COL_ACCENT).stroke(Stroke::NONE).rounding(Rounding::same(6.0))).clicked() { self.open_file_dialog(); }
                            ui.add_space(12.0);
                            ui.label(RichText::new("Ctrl+O  open  |  Ctrl+F  search  |  Ctrl+H  find  |  Ctrl+N  nav  |  Esc  clear").size(11.0).color(COL_FAINT));
                        });
                    });
                    return;
                }

                if self.filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("No lines match your filters").size(16.0).color(COL_FAINT));
                    });
                    return;
                }

                // Column headers
                {
                    let hdr_h=18.0;
                    let (hdr_rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),hdr_h),Sense::hover());
                    let p=ui.painter(); let y=hdr_rect.center().y; let x0=hdr_rect.min.x;
                    let fid=FontId::monospace(9.5); let col=Color32::from_rgb(140,150,170);
                    p.text(egui::pos2(x0+COL_LN-6.0,y),Align2::RIGHT_CENTER,"#",fid.clone(),col);
                    p.text(egui::pos2(x0+COL_LN,y),Align2::LEFT_CENTER,"TIME",fid.clone(),col);
                    p.text(egui::pos2(x0+COL_LN+COL_TS,y),Align2::LEFT_CENTER,"Δ TIME",fid.clone(),col);
                    p.text(egui::pos2(x0+COL_LN+COL_TS+COL_DT,y),Align2::LEFT_CENTER,"LVL",fid.clone(),col);
                    p.text(egui::pos2(x0+COL_LN+COL_TS+COL_DT+COL_LV,y),Align2::LEFT_CENTER,"MODULE",fid.clone(),col);
                    p.text(egui::pos2(x0+COL_LN+COL_TS+COL_DT+COL_LV+COL_MOD,y),Align2::LEFT_CENTER,"MESSAGE",fid.clone(),col);
                }
                ui.add(egui::Separator::default().horizontal().spacing(1.0));

                let row_h=self.row_height; let font_sz=self.font_size; let n=self.filtered.len();
                let visible_height=ui.available_height();

                let mut sa=ScrollArea::vertical().id_source("log_scroll").auto_shrink(false)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                if let Some(off)=self.scroll_to_offset.take() { sa=sa.scroll_offset(Vec2::new(0.0,off)); }

                let out=sa.show_rows(ui,row_h,n,|ui,row_range| {
                    ui.spacing_mut().item_spacing=Vec2::ZERO;

                    for row_idx in row_range {
                        let line_idx=match self.filtered.get(row_idx){Some(&i)=>i,None=>continue};
                        let line=match self.all_lines.get(line_idx){Some(l)=>l,None=>continue};
                        let is_sel=self.selected==Some(row_idx);
                        let is_cur=self.is_current_match_row(row_idx);
                        let is_match=self.is_match_row(row_idx);
                        let nav_kind: Option<NavKind>=self.nav_entries.iter().find(|e| e.row_idx==row_idx).map(|e| e.kind);

                        let (row_rect,resp)=ui.allocate_exact_size(Vec2::new(ui.available_width(),row_h),Sense::click());
                        if !ui.is_rect_visible(row_rect) { continue; }

                        let bg=if is_sel{BG_ROW_SEL}
                            else if is_cur{Color32::from_rgba_unmultiplied(255,180,40,55)}
                            else if is_match{Color32::from_rgba_unmultiplied(200,150,30,28)}
                            else if resp.hovered(){BG_ROW_HOVER}
                            else if let Some(c)=line.level.row_bg(){c}
                            else{Color32::TRANSPARENT};
                        if bg!=Color32::TRANSPARENT { ui.painter().rect_filled(row_rect,Rounding::ZERO,bg); }

                        // Left accent bar
                        if is_cur {
                            ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min,Vec2::new(2.5,row_h)),Rounding::ZERO,Color32::from_rgb(255,200,60));
                        } else if matches!(line.level,Level::Error|Level::Warning) {
                            ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min,Vec2::new(2.5,row_h)),Rounding::ZERO,line.level.color());
                        }

                        // Right nav landmark indicator (thin strip on right edge)
                        if let Some(kind) = nav_kind {
                            let c=kind.color();
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.max.x-3.0,row_rect.min.y),Vec2::new(3.0,row_h)),
                                Rounding::ZERO, Color32::from_rgba_unmultiplied(c.r(),c.g(),c.b(),140),
                            );
                        }

                        let p=ui.painter(); let y=row_rect.center().y;
                        let fid=FontId::monospace(font_sz); let fsm=FontId::monospace((font_sz-1.0).max(8.0)); let fxs=FontId::monospace((font_sz-2.0).max(7.5));
                        let mut x=row_rect.min.x;

                        p.text(egui::pos2(x+COL_LN-6.0,y),Align2::RIGHT_CENTER,line.num.to_string(),fxs.clone(),COL_FAINT);
                        x+=COL_LN;
                        let ts=if line.timestamp.len()>12{&line.timestamp[..12]}else{&line.timestamp};
                        p.text(egui::pos2(x,y),Align2::LEFT_CENTER,ts,fsm.clone(),Color32::from_rgb(160,210,255));
                        x+=COL_TS;
                        if let Some(dms)=line.delta_ms { if dms>0 {
                            let dc=if dms>=1000{Color32::from_rgb(255,200,80)}else if dms>=100{Color32::from_rgb(180,180,200)}else{Color32::from_rgb(120,130,150)};
                            p.text(egui::pos2(x,y),Align2::LEFT_CENTER,format_delta(dms),fxs.clone(),dc);
                        }}
                        x+=COL_DT;
                        p.text(egui::pos2(x,y),Align2::LEFT_CENTER,line.level.label(),fsm.clone(),line.level.color());
                        x+=COL_LV;
                        let md=if line.module.len()>26{&line.module[..26]}else{&line.module};
                        p.text(egui::pos2(x,y),Align2::LEFT_CENTER,md,fsm.clone(),Color32::from_rgb(180,185,200));
                        x+=COL_MOD;
                        let max_chars=((row_rect.max.x-x-8.0)/(font_sz*0.6)) as usize;
                        let msg=&line.message;
                        let msg_disp=if msg.len()>max_chars.max(40){&msg[..max_chars.max(40)]}else{msg.as_str()};
                        let msg_col=match line.level{Level::Error=>Color32::from_rgb(255,180,170),Level::Warning=>Color32::from_rgb(255,220,150),_=>Color32::from_rgb(210,215,225)};
                        p.text(egui::pos2(x,y),Align2::LEFT_CENTER,msg_disp,fid.clone(),msg_col);

                        if resp.clicked() {
                            if is_sel{self.detail_open=!self.detail_open;}
                            else{self.selected=Some(row_idx);self.detail_open=true;}
                        }
                    }
                });

                self.scroll_area_height=visible_height;
                self.current_scroll_offset=out.state.offset.y;
            });
    }
}

// ─── main ────────────────────────────────────────────────────────────────────

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
