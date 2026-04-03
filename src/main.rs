#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, App, Frame, NativeOptions};
use egui::{
    Align, Align2, Color32, FontId, Key, Layout, Rounding, ScrollArea, Sense, Stroke, TextEdit,
    Vec2,
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
            "ERR" | "ERROR" => Self::Error,
            "WRN" | "WARN" | "WARNING" => Self::Warning,
            "INF" | "INFO" => Self::Info,
            "DBG" | "DEBUG" => Self::Debug,
            "TRC" | "TRACE" | "VERBOSE" => Self::Trace,
            _ => Self::Debug,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Error => "ERR",
            Self::Warning => "WRN",
            Self::Info => "INF",
            Self::Debug => "DBG",
            Self::Trace => "TRC",
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
            Self::Error   => Some(Color32::from_rgba_unmultiplied(200, 50, 40, 22)),
            Self::Warning => Some(Color32::from_rgba_unmultiplied(200, 150, 30, 18)),
            _ => None,
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Error => 0, Self::Warning => 1, Self::Info => 2,
            Self::Debug => 3, Self::Trace   => 4,
        }
    }
}

// ─── LogLine ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LogLine {
    num:      usize,
    timestamp: String,
    ts_ms:    Option<u64>,
    delta_ms: Option<u64>,
    level:    Level,
    module:   String,
    message:  String,
    raw:      String,
}

/// Parse "HH:MM:SS.mmm" (or "HH:MM:SS") into total milliseconds since midnight.
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
    } else {
        (sec_rest, "")
    };
    // strip any non-digit trailer (e.g. timezone)
    let s_str = s_str.trim_end_matches(|c: char| !c.is_ascii_digit());
    let s: u64 = s_str.parse().ok()?;
    let ms: u64 = if frac.is_empty() {
        0
    } else {
        let n = frac.len().min(3);
        let v: u64 = frac[..n].parse().ok()?;
        match n { 1 => v * 100, 2 => v * 10, _ => v }
    };
    Some(h * 3_600_000 + m * 60_000 + s * 1_000 + ms)
}

fn format_delta(ms: u64) -> String {
    if ms < 1_000      { format!("+{}ms",        ms) }
    else if ms < 10_000  { format!("+{:.2}s", ms as f64 / 1000.0) }
    else if ms < 60_000  { format!("+{:.1}s", ms as f64 / 1000.0) }
    else {
        let s = ms / 1_000;
        format!("+{}m{:02}s", s / 60, s % 60)
    }
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

    // Format 1: [timestamp] [LEVEL] [module] message
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

    // Format 2: HH:MM:SS [thread] LEVEL: message
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

    // Format 3: date  [Module] [LVL] message
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

// ─── App ─────────────────────────────────────────────────────────────────────

struct LogViewerApp {
    all_lines:   Vec<LogLine>,
    filtered:    Vec<usize>,
    search:      String,
    search_lc:   String,
    show:        [bool; 5],
    module_filter: String,
    modules:     Vec<String>,
    counts:      [usize; 5],
    row_height:  f32,
    font_size:   f32,
    selected:    Option<usize>,
    detail_open: bool,
    status:      String,
    drag_hover:  bool,
    // minimap cache (level per filtered row, rebuilt on filter change)
    minimap_levels: Vec<u8>,   // level index 0-4
    // scroll sync
    scroll_to_offset:       Option<f32>, // target scroll offset in pixels
    current_scroll_offset:  f32,
    scroll_area_height:     f32,
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
        }
    }
}

impl LogViewerApp {
    fn load_text(&mut self, text: &str) {
        self.all_lines = text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                let mut line = parse_log_line(l, i + 1);
                line.ts_ms = parse_timestamp_ms(&line.timestamp);
                line
            })
            .collect();

        // compute delta times
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
        let n = self.all_lines.len();
        self.status = format!("Loaded {} lines", n);
        self.apply_filters();
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

        // rebuild minimap cache
        self.minimap_levels = self.filtered.iter()
            .map(|&i| self.all_lines[i].level.index() as u8)
            .collect();
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log files", &["log", "txt"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.load_file(&path);
        }
    }
}

// ─── UI constants ─────────────────────────────────────────────────────────────

const BG_BASE:      Color32 = Color32::from_rgb(13, 17, 23);
const BG_PANEL:     Color32 = Color32::from_rgb(20, 25, 32);
const BG_ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 8);
const BG_ROW_SEL:   Color32 = Color32::from_rgba_premultiplied(88, 166, 255, 38);
const COL_BORDER:   Color32 = Color32::from_rgb(42, 48, 58);
const COL_TEXT:     Color32 = Color32::from_gray(215);
const COL_MUTED:    Color32 = Color32::from_gray(130);
const COL_FAINT:    Color32 = Color32::from_gray(62);
const COL_ACCENT:   Color32 = Color32::from_rgb(88, 166, 255);

// column widths (px)
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
    v.widgets.inactive.bg_fill   = Color32::from_rgb(28, 34, 43);
    v.widgets.inactive.bg_stroke = Stroke::new(0.5, COL_BORDER);
    v.widgets.hovered.bg_fill    = Color32::from_rgb(38, 44, 55);
    v.widgets.hovered.bg_stroke  = Stroke::new(0.5, Color32::from_gray(75));
    v.widgets.active.bg_fill     = Color32::from_rgb(50, 58, 72);
    v.selection.bg_fill          = Color32::from_rgba_unmultiplied(88, 166, 255, 52);
    v
}

fn styled_btn<'a>(text: &'a str, text_color: Color32) -> egui::Button<'a> {
    egui::Button::new(
        egui::RichText::new(text)
            .font(FontId::proportional(12.0))
            .color(text_color),
    )
    .fill(Color32::from_rgb(26, 32, 42))
    .stroke(Stroke::new(0.5, COL_BORDER))
    .rounding(Rounding::same(5.0))
    .min_size(Vec2::new(0.0, 26.0))
}

fn icon_btn<'a>(text: &'a str) -> egui::Button<'a> {
    egui::Button::new(
        egui::RichText::new(text)
            .font(FontId::proportional(12.0))
            .color(COL_MUTED),
    )
    .fill(Color32::from_rgb(26, 32, 42))
    .stroke(Stroke::new(0.5, COL_BORDER))
    .rounding(Rounding::same(5.0))
    .min_size(Vec2::new(32.0, 26.0))
}

fn accent_btn<'a>(text: &'a str) -> egui::Button<'a> {
    egui::Button::new(
        egui::RichText::new(text)
            .font(FontId::proportional(12.0))
            .color(COL_TEXT),
    )
    .fill(Color32::from_rgb(28, 40, 56))
    .stroke(Stroke::new(0.75, Color32::from_rgba_unmultiplied(88, 166, 255, 140)))
    .rounding(Rounding::same(5.0))
    .min_size(Vec2::new(0.0, 26.0))
}

fn level_toggle(ui: &mut egui::Ui, label: &str, count: usize, active: bool, color: Color32) -> bool {
    let (fg, bg, stroke) = if active {
        (
            color,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30),
            Stroke::new(0.75, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 170)),
        )
    } else {
        (
            Color32::from_gray(68),
            Color32::from_rgb(20, 25, 32),
            Stroke::new(0.5, Color32::from_gray(36)),
        )
    };
    let btn = egui::Button::new(
        egui::RichText::new(format!("{} {}", label, count))
            .color(fg).font(FontId::monospace(11.0)).strong(),
    )
    .fill(bg).stroke(stroke).rounding(Rounding::same(4.0))
    .min_size(Vec2::new(0.0, 26.0));
    ui.add(btn).clicked()
}


// ─── App::update ─────────────────────────────────────────────────────────────

impl App for LogViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        ctx.set_visuals(dark_visuals());

        // ── drag & drop ──────────────────────────────────────────────────────
        ctx.input(|i| {
            self.drag_hover = !i.raw.hovered_files.is_empty();
            for d in &i.raw.dropped_files {
                if let Some(p) = d.path.clone() { self.load_file(&p); }
                else if let Some(b) = &d.bytes {
                    if let Ok(t) = std::str::from_utf8(b) { self.load_text(t); }
                }
            }
        });

        // ── keyboard ─────────────────────────────────────────────────────────
        let open_dialog = ctx.input(|i| i.key_pressed(Key::O) && i.modifiers.ctrl);
        if open_dialog { self.open_file_dialog(); }

        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                if !self.search.is_empty() {
                    self.search.clear(); self.search_lc.clear(); self.apply_filters();
                } else { self.selected = None; self.detail_open = false; }
            }
        });

        // ════════════════════════════════════════════════════════════════════
        // TOP TOOLBAR
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::none()
                .fill(BG_PANEL)
                .stroke(Stroke::new(1.0, COL_BORDER))
                .inner_margin(egui::Margin::symmetric(12.0, 6.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    ui.label(
                        egui::RichText::new("▣  XTR LOG VIEWER")
                            .color(COL_ACCENT)
                            .font(FontId::monospace(12.5))
                            .strong(),
                    );
                    ui.add(egui::Separator::default().vertical().spacing(6.0));

                    // Search
                    let search_id = egui::Id::new("search_box");
                    let re = ui.add(
                        TextEdit::singleline(&mut self.search)
                            .id(search_id)
                            .hint_text("⌕  Search  (Ctrl+F)")
                            .desired_width(230.0)
                            .font(FontId::monospace(12.0)),
                    );
                    if re.changed() {
                        self.search_lc = self.search.to_lowercase();
                        self.apply_filters();
                    }
                    if ctx.input(|i| i.key_pressed(Key::F) && (i.modifiers.ctrl || i.modifiers.command)) {
                        ctx.memory_mut(|m| m.request_focus(search_id));
                    }

                    // Module combo
                    if !self.modules.is_empty() {
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        let label = if self.module_filter.is_empty() {
                            "All modules".to_string()
                        } else if self.module_filter.len() > 26 {
                            format!("…{}", &self.module_filter[self.module_filter.len().saturating_sub(24)..])
                        } else {
                            self.module_filter.clone()
                        };
                        let mut changed = false;
                        egui::ComboBox::from_id_source("mod_cb")
                            .selected_text(egui::RichText::new(label).font(FontId::proportional(12.0)))
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(self.module_filter.is_empty(), "All modules").clicked() {
                                    self.module_filter.clear(); changed = true;
                                }
                                for m in self.modules.clone() {
                                    let d = if m.len() > 38 { format!("…{}", &m[m.len()-36..]) } else { m.clone() };
                                    if ui.selectable_label(self.module_filter == m, d).clicked() {
                                        self.module_filter = m; changed = true;
                                    }
                                }
                            });
                        if changed { self.apply_filters(); }
                    }

                    ui.add(egui::Separator::default().vertical().spacing(6.0));

                    // Level toggles
                    let defs: [(usize, &str, Color32); 5] = [
                        (0, "ERR", Level::Error.color()),
                        (1, "WRN", Level::Warning.color()),
                        (2, "INF", Level::Info.color()),
                        (3, "DBG", Level::Debug.color()),
                        (4, "TRC", Level::Trace.color()),
                    ];
                    let mut filter_changed = false;
                    for (idx, lbl, color) in defs {
                        if level_toggle(ui, lbl, self.counts[idx], self.show[idx], color) {
                            self.show[idx] = !self.show[idx];
                            filter_changed = true;
                        }
                    }
                    if filter_changed { self.apply_filters(); }

                    // Right-side controls
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        if ui.add(styled_btn("Open  Ctrl+O", COL_TEXT)).clicked() {
                            self.open_file_dialog();
                        }
                        if !self.all_lines.is_empty() {
                            if ui.add(styled_btn("Clear", COL_MUTED)).clicked() {
                                *self = LogViewerApp::default();
                            }
                        }
                        ui.add(egui::Separator::default().vertical().spacing(6.0));
                        if ui.add(icon_btn("A+")).on_hover_text("Increase font size").clicked() {
                            self.font_size = (self.font_size + 1.0).min(20.0);
                            self.row_height = self.font_size + 8.0;
                        }
                        if ui.add(icon_btn("A−")).on_hover_text("Decrease font size").clicked() {
                            self.font_size = (self.font_size - 1.0).max(9.0);
                            self.row_height = self.font_size + 8.0;
                        }
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // STATUS BAR
        // ════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::bottom("statusbar")
            .frame(egui::Frame::none()
                .fill(BG_PANEL)
                .stroke(Stroke::new(1.0, COL_BORDER))
                .inner_margin(egui::Margin::symmetric(12.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let mk = |n: usize, s: &str, c: Color32| {
                        egui::RichText::new(format!("{n} {s}")).color(c).font(FontId::monospace(11.0))
                    };
                    ui.label(mk(self.counts[0], "errors",   Level::Error.color()));
                    ui.label(mk(self.counts[1], "warnings", Level::Warning.color()));
                    ui.label(mk(self.counts[2], "info",     Level::Info.color()));
                    ui.label(mk(self.counts[3], "debug",    Level::Debug.color()));
                    ui.label(mk(self.counts[4], "trace",    Level::Trace.color()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(egui::RichText::new(
                            format!("{} / {} lines shown", self.filtered.len(), self.all_lines.len())
                        ).color(COL_MUTED).font(FontId::monospace(11.0)));
                        ui.add(egui::Separator::default().vertical());
                        ui.label(egui::RichText::new(&self.status).color(COL_MUTED).font(FontId::monospace(11.0)));
                    });
                });
            });

        // ════════════════════════════════════════════════════════════════════
        // DETAIL PANEL
        // ════════════════════════════════════════════════════════════════════
        if self.detail_open {
            let sel: Option<LogLine> = self.selected
                .and_then(|r| self.filtered.get(r).copied())
                .and_then(|li| self.all_lines.get(li))
                .cloned();

            if let Some(line) = sel {
                egui::TopBottomPanel::bottom("detail_panel")
                    .resizable(true)
                    .default_height(148.0)
                    .min_height(80.0)
                    .frame(egui::Frame::none()
                        .fill(Color32::from_rgb(14, 19, 27))
                        .stroke(Stroke::new(1.0, COL_BORDER))
                        .inner_margin(egui::Margin::symmetric(14.0, 10.0)))
                    .show(ctx, |ui| {
                        // ── header bar ────────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("LINE DETAIL")
                                    .font(FontId::monospace(10.0))
                                    .color(Color32::from_gray(52))
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let mut close = false;
                                if ui.add(styled_btn("✕  Close", COL_MUTED)).clicked() { close = true; }
                                if ui.add(accent_btn("⧉  Copy Raw"))
                                    .on_hover_text("Copy raw line to clipboard").clicked() {
                                    ui.output_mut(|o| o.copied_text = line.raw.clone());
                                }
                                if close { self.detail_open = false; }
                            });
                        });
                        ui.add_space(5.0);
                        ui.add(egui::Separator::default().horizontal().spacing(3.0));
                        ui.add_space(4.0);

                        // ── meta grid (4 columns) ─────────────────────────
                        egui::Grid::new("detail_grid")
                            .num_columns(4)
                            .spacing([20.0, 5.0])
                            .show(ui, |ui| {
                                let lbl = |s: &str| egui::RichText::new(s).color(COL_FAINT).font(FontId::monospace(10.0));
                                let val = |s: String| egui::RichText::new(s).color(COL_TEXT).font(FontId::monospace(11.0));

                                ui.label(lbl("LINE"));
                                ui.label(val(line.num.to_string()));
                                ui.label(lbl("LEVEL"));
                                ui.label(egui::RichText::new(line.level.label())
                                    .color(line.level.color()).strong().font(FontId::monospace(11.0)));
                                ui.end_row();

                                ui.label(lbl("TIME"));
                                ui.label(val(line.timestamp.clone()));
                                ui.label(lbl("Δ TIME"));
                                ui.label(val(line.delta_ms.map(format_delta).unwrap_or_else(|| "—".into())));
                                ui.end_row();

                                ui.label(lbl("MODULE"));
                                ui.label(val(line.module.clone()));
                                ui.label(lbl(""));
                                ui.label(val(String::new()));
                                ui.end_row();
                            });

                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("MESSAGE").color(COL_FAINT).font(FontId::monospace(10.0)));
                        ui.add_space(3.0);
                        ScrollArea::vertical().id_source("detail_scroll").max_height(55.0).show(ui, |ui| {
                            ui.label(egui::RichText::new(&line.message)
                                .font(FontId::monospace(11.5)).color(COL_TEXT));
                        });
                    });
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // MINIMAP / COLOUR SCROLLBAR  (right side panel)
        // ════════════════════════════════════════════════════════════════════
        {
            let n_filt     = self.filtered.len();
            let row_h      = self.row_height;
            let scroll_off = self.current_scroll_offset;
            let viewport_h = self.scroll_area_height;
            let ml         = self.minimap_levels.clone();
            let mut jump_to_offset: Option<f32> = None;

            // Mirror Level::color() exactly — same hues the row badges use.
            const MM_ERR:   Color32 = Color32::from_rgb(245,  95,  85);  // red
            const MM_WRN:   Color32 = Color32::from_rgb(235, 180,  55);  // amber
            const MM_INF:   Color32 = Color32::from_rgb( 70, 200,  95);  // green
            const MM_DBG:   Color32 = Color32::from_rgb( 95, 165, 245);  // blue
            const MM_TRC:   Color32 = Color32::from_rgb(105, 110, 122);  // gray
            const MM_COLS: [Color32; 5] = [MM_ERR, MM_WRN, MM_INF, MM_DBG, MM_TRC];

            egui::SidePanel::right("minimap_panel")
                .exact_width(34.0)
                .resizable(false)
                .frame(egui::Frame::none().fill(Color32::from_rgb(10, 13, 18)))
                .show(ctx, |ui| {
                    let avail = ui.available_rect_before_wrap();
                    let (resp, painter) = ui.allocate_painter(avail.size(), Sense::click_and_drag());
                    let r = resp.rect;

                    painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(10, 13, 18));
                    painter.rect_filled(
                        egui::Rect::from_min_max(r.left_top(), egui::pos2(r.min.x + 1.0, r.max.y)),
                        Rounding::ZERO, COL_BORDER,
                    );

                    if n_filt == 0 { return; }

                    let bx0 = r.min.x + 3.0;
                    let bx1 = r.max.x - 2.0;
                    let by0 = r.min.y;
                    let ah  = r.height();

                    // draw colour bars (unchanged)
                    const THRESHOLD: f32 = 0.20;
                    let n = n_filt as f32;
                    let pixels = ah as usize;
                    for py in 0..pixels {
                        let i0 = ((py       as f32 * n / ah) as usize).min(n_filt - 1);
                        let i1 = (((py + 1) as f32 * n / ah) as usize).min(n_filt - 1);
                        let i1 = i1.max(i0);
                        let bucket_size = (i1 - i0 + 1) as f32;

                        let mut counts = [0u16; 5];
                        for i in i0..=i1 {
                            counts[ml[i] as usize] += 1;
                        }

                        let dominant = (0..5_usize)
                            .find(|&lvl| counts[lvl] as f32 / bucket_size >= THRESHOLD)
                            .unwrap_or_else(|| {
                                counts.iter().enumerate()
                                    .max_by(|&(ia, &ca), &(ib, &cb)| {
                                        ca.cmp(&cb).then(ib.cmp(&ia))
                                    })
                                    .map(|(idx, _)| idx)
                                    .unwrap_or(4)
                            });

                        let y0 = by0 + py as f32;
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(bx0, y0),
                                egui::pos2(bx1, y0 + 1.6),
                            ),
                            Rounding::ZERO,
                            MM_COLS[dominant],
                        );
                    }

                    // viewport window indicator
                    let total_h = n_filt as f32 * row_h;
                    if total_h > 0.0 && viewport_h > 0.0 {
                        let vt = (scroll_off / total_h).clamp(0.0, 1.0);
                        let vb = ((scroll_off + viewport_h) / total_h).clamp(0.0, 1.0);
                        let wy0 = (by0 + vt * ah).min(r.max.y - 4.0);
                        let wy1 = (by0 + vb * ah).clamp(wy0 + 4.0, r.max.y);
                        painter.rect(
                            egui::Rect::from_min_max(
                                egui::pos2(r.min.x + 1.5, wy0),
                                egui::pos2(r.max.x - 1.0, wy1),
                            ),
                            Rounding::same(2.0),
                            Color32::from_rgba_unmultiplied(200, 225, 255, 18),
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 225, 255, 130)),
                        );
                    }

                    // click/drag to jump – compute target offset, clamped
                    if resp.dragged() || resp.clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let frac = ((pos.y - by0) / ah).clamp(0.0, 1.0);
                            let target_row = (frac * n_filt as f32) as usize;
                            let target_row = target_row.min(n_filt.saturating_sub(1));
                            // compute offset from row, then clamp to valid range
                            let mut target_offset = target_row as f32 * row_h;
                            if total_h > viewport_h {
                                target_offset = target_offset.min(total_h - viewport_h);
                            } else {
                                target_offset = 0.0;
                            }
                            jump_to_offset = Some(target_offset);
                        }
                    }
                });

            if let Some(offset) = jump_to_offset {
                self.scroll_to_offset = Some(offset);
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // MAIN LOG AREA
        // ════════════════════════════════════════════════════════════════════
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_BASE))
            .show(ctx, |ui| {

                // ── empty / drop zone ────────────────────────────────────
                if self.all_lines.is_empty() {
                    let outer = ui.available_rect_before_wrap();
                    if self.drag_hover {
                        ui.painter().rect(
                            outer.shrink(4.0),
                            Rounding::same(8.0),
                            Color32::from_rgba_unmultiplied(88, 166, 255, 22),
                            Stroke::new(2.0, COL_ACCENT),
                        );
                    }
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(60.0);
                            ui.label(egui::RichText::new("◫").size(52.0).color(Color32::from_gray(40)));
                            ui.add_space(14.0);
                            ui.label(egui::RichText::new("Drop a log file here").size(22.0).color(Color32::from_gray(165)));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(
                                "Supports XTR · MTF · CARIAD · TLS-Attacker · DiagnosticToolBox formats",
                            ).size(12.0).color(COL_FAINT));
                            ui.add_space(22.0);
                            if ui.add(
                                egui::Button::new(egui::RichText::new("  Open file  (Ctrl+O)  ").size(13.0).color(Color32::from_gray(10)))
                                    .fill(COL_ACCENT).stroke(Stroke::NONE).rounding(Rounding::same(6.0))
                            ).clicked() { self.open_file_dialog(); }
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new(
                                "Ctrl+O  open  |  Ctrl+F  search  |  Esc  clear/deselect  |  Click row  →  detail"
                            ).size(11.0).color(Color32::from_gray(48)));
                        });
                    });
                    return;
                }

                // ── no results ──────────────────────────────────────────
                if self.filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("No lines match your filters").size(16.0).color(COL_FAINT));
                    });
                    return;
                }

                // ── column headers ──────────────────────────────────────
                {
                    let hdr_h = 18.0;
                    let (hdr_rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), hdr_h), Sense::hover(),
                    );
                    let p = ui.painter();
                    let y = hdr_rect.center().y;
                    let x0 = hdr_rect.min.x;
                    let fid = FontId::monospace(9.5);
                    let col = Color32::from_gray(52);

                    p.text(egui::pos2(x0 + COL_LN - 6.0, y), Align2::RIGHT_CENTER, "#",       fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN,        y), Align2::LEFT_CENTER,  "TIME",    fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS, y), Align2::LEFT_CENTER, "Δ TIME", fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT, y), Align2::LEFT_CENTER, "LVL",     fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + COL_LV, y), Align2::LEFT_CENTER, "MODULE",  fid.clone(), col);
                    p.text(egui::pos2(x0 + COL_LN + COL_TS + COL_DT + COL_LV + COL_MOD, y), Align2::LEFT_CENTER, "MESSAGE", fid.clone(), col);
                }
                ui.add(egui::Separator::default().horizontal().spacing(1.0));

                // ── virtual-scrolling rows ───────────────────────────────
                let row_h   = self.row_height;
                let font_sz = self.font_size;
                let n       = self.filtered.len();

                // capture visible height BEFORE the scroll area
                let visible_height = ui.available_height();

                let mut sa = ScrollArea::vertical()
                    .id_source("log_scroll")
                    .auto_shrink(false)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);

                // apply pending scroll offset (if any)
                if let Some(off) = self.scroll_to_offset.take() {
                    sa = sa.scroll_offset(Vec2::new(0.0, off));
                }

                let out = sa.show_rows(ui, row_h, n, |ui, row_range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    for row_idx in row_range {
                        let line_idx = match self.filtered.get(row_idx) { Some(&i) => i, None => continue };
                        let line     = match self.all_lines.get(line_idx) { Some(l) => l, None => continue };
                        let is_sel   = self.selected == Some(row_idx);

                        let (row_rect, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row_h), Sense::click(),
                        );
                        if !ui.is_rect_visible(row_rect) { continue; }

                        // background
                        let bg = if is_sel {
                            BG_ROW_SEL
                        } else if resp.hovered() {
                            BG_ROW_HOVER
                        } else if let Some(c) = line.level.row_bg() {
                            c
                        } else {
                            Color32::TRANSPARENT
                        };
                        if bg != Color32::TRANSPARENT {
                            ui.painter().rect_filled(row_rect, Rounding::ZERO, bg);
                        }

                        // left accent bar (error / warning)
                        if matches!(line.level, Level::Error | Level::Warning) {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(row_rect.min, Vec2::new(2.5, row_h)),
                                Rounding::ZERO, line.level.color(),
                            );
                        }

                        let p    = ui.painter();
                        let y    = row_rect.center().y;
                        let fid  = FontId::monospace(font_sz);
                        let fsm  = FontId::monospace((font_sz - 1.0).max(8.0));
                        let fxs  = FontId::monospace((font_sz - 2.0).max(7.5));
                        let mut x = row_rect.min.x;

                        // line number
                        p.text(egui::pos2(x + COL_LN - 6.0, y), Align2::RIGHT_CENTER,
                            line.num.to_string(), fxs.clone(), COL_FAINT);
                        x += COL_LN;

                        // timestamp
                        let ts = if line.timestamp.len() > 12 { &line.timestamp[..12] } else { &line.timestamp };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, ts, fsm.clone(),
                            Color32::from_gray(145));
                        x += COL_TS;

                        // delta time
                        if let Some(dms) = line.delta_ms {
                            if dms > 0 {
                                let s = format_delta(dms);
                                let dc = if dms >= 1000 {
                                    Color32::from_rgb(225, 170, 55)
                                } else if dms >= 100 {
                                    Color32::from_gray(148)
                                } else {
                                    Color32::from_gray(80)
                                };
                                p.text(egui::pos2(x, y), Align2::LEFT_CENTER, s, fxs.clone(), dc);
                            }
                        }
                        x += COL_DT;

                        // level
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER,
                            line.level.label(), fsm.clone(), line.level.color());
                        x += COL_LV;

                        // module
                        let mod_disp = if line.module.len() > 26 { &line.module[..26] } else { &line.module };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, mod_disp, fsm.clone(),
                            Color32::from_gray(122));
                        x += COL_MOD;

                        // message
                        let max_chars = ((row_rect.max.x - x - 8.0) / (font_sz * 0.6)) as usize;
                        let msg = &line.message;
                        let msg_disp = if msg.len() > max_chars.max(40) { &msg[..max_chars.max(40)] } else { msg.as_str() };
                        let msg_col = match line.level {
                            Level::Error   => Color32::from_rgb(255, 145, 130),
                            Level::Warning => Color32::from_rgb(248, 200,  85),
                            _              => Color32::from_gray(200),
                        };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, msg_disp, fid.clone(), msg_col);

                        // click handler
                        if resp.clicked() {
                            if is_sel { self.detail_open = !self.detail_open; }
                            else { self.selected = Some(row_idx); self.detail_open = true; }
                        }
                    }
                });

                // store viewport height and current scroll offset for minimap
                self.scroll_area_height = visible_height;
                self.current_scroll_offset = out.state.offset.y;
            });
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
    eframe::run_native(
        "XTR Log Viewer",
        opts,
        Box::new(|_cc| Box::new(LogViewerApp::default())),
    )
}
