#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, App, Frame, NativeOptions};
use egui::{
    Align, Align2, Color32, FontId, Key, Layout, Rounding, ScrollArea,
    Sense, Stroke, TextEdit, Vec2,
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
            Self::Error => Color32::from_rgb(255, 100, 90),
            Self::Warning => Color32::from_rgb(230, 180, 60),
            Self::Info => Color32::from_rgb(60, 185, 80),
            Self::Debug => Color32::from_rgb(88, 166, 255),
            Self::Trace => Color32::from_gray(100),
        }
    }
    fn row_bg(self) -> Option<Color32> {
        match self {
            Self::Error => Some(Color32::from_rgba_unmultiplied(200, 50, 40, 22)),
            Self::Warning => Some(Color32::from_rgba_unmultiplied(200, 150, 30, 18)),
            _ => None,
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
            Self::Debug => 3,
            Self::Trace => 4,
        }
    }
}

// ─── LogLine ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LogLine {
    num: usize,
    timestamp: String,
    level: Level,
    module: String,
    message: String,
    raw: String,
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut iter = s.chars().peekable();
    while let Some(c) = iter.next() {
        if c == '\x1b' {
            if iter.peek() == Some(&'[') {
                iter.next();
                for nc in iter.by_ref() {
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// Extract content of next [...] bracket pair from `s`, return (content, rest)
fn take_bracket(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('[') {
        return None;
    }
    let s = &s[1..];
    let end = s.find(']')?;
    Some((&s[..end], s[end + 1..].trim_start()))
}

fn parse_log_line(raw: &str, num: usize) -> LogLine {
    let s = strip_ansi(raw.trim());

    // ── Format 1: XTR / MTF / CARIAD
    // [2026-03-30 14:30:17.022] [INF] [module.name] message
    if s.starts_with('[') {
        if let Some((ts_raw, rest)) = take_bracket(&s) {
            // timestamp — keep HH:MM:SS.mmm part only
            let ts = ts_raw
                .split_whitespace()
                .nth(1)
                .or_else(|| ts_raw.split_whitespace().next())
                .unwrap_or(ts_raw)
                .to_string();

            if let Some((level_str, rest2)) = take_bracket(rest) {
                let level = Level::from_str(level_str);

                // optional module bracket
                let (module, message) = if let Some((m, msg_rest)) = take_bracket(rest2) {
                    (m.to_string(), msg_rest.to_string())
                } else {
                    (String::new(), rest2.to_string())
                };

                return LogLine { num, timestamp: ts, level, module, message, raw: raw.to_string() };
            }
        }
    }

    // ── Format 2: TLS-Attacker / Java logger (already ANSI-stripped)
    // 14:32:54 [Thread-1] DEBUG: WorkflowExecutor - message
    // 14:32:54 [main] INFO : ServerTcpTransportHandler - message
    {
        let parts: Vec<&str> = s.splitn(3, ' ').collect();
        if parts.len() == 3 {
            let ts_cand = parts[0];
            // crude time check: hh:mm:ss pattern
            let is_time = ts_cand.len() >= 5 && ts_cand.as_bytes().get(2) == Some(&b':');
            if is_time {
                if let Some((thread, rest)) = take_bracket(parts[1]) {
                    // rest = "DEBUG: msg" or "INFO : msg"
                    let colon_pos = rest.find(':');
                    if let Some(cp) = colon_pos {
                        let level_str = rest[..cp].trim();
                        if matches!(
                            level_str.to_uppercase().as_str(),
                            "DEBUG" | "INFO" | "WARN" | "WARNING" | "ERROR" | "TRACE"
                        ) {
                            let message = rest[cp + 1..].trim().to_string();
                            return LogLine {
                                num,
                                timestamp: ts_cand.to_string(),
                                level: Level::from_str(level_str),
                                module: thread.to_string(),
                                message,
                                raw: raw.to_string(),
                            };
                        }
                    }
                }
            }
        }
    }

    // ── Format 3: DTB / DiagnosticToolBox
    // 2026-03-30 14:32:44.282  [Module ] [DBG]  message
    if let Some(space2) = s.find("  ") {
        let ts_part = s[..space2].trim();
        let ts = ts_part
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        let rest = s[space2..].trim();
        if let Some((module, rest2)) = take_bracket(rest) {
            if let Some((level_str, msg)) = take_bracket(rest2) {
                let level = Level::from_str(level_str);
                if !matches!(level, Level::Debug) || level_str.trim().to_uppercase() == "DBG" {
                    return LogLine {
                        num,
                        timestamp: ts,
                        level,
                        module: module.trim().to_string(),
                        message: msg.to_string(),
                        raw: raw.to_string(),
                    };
                }
            }
        }
    }

    // ── Fallback
    LogLine {
        num,
        timestamp: String::new(),
        level: Level::Debug,
        module: String::new(),
        message: s.to_string(),
        raw: raw.to_string(),
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

struct LogViewerApp {
    all_lines: Vec<LogLine>,
    filtered: Vec<usize>, // indices into all_lines

    search: String,
    search_lc: String,
    show: [bool; 5],
    module_filter: String,
    modules: Vec<String>,
    counts: [usize; 5],

    row_height: f32,
    font_size: f32,

    selected: Option<usize>, // row index in filtered
    detail_open: bool,

    status: String,
    drag_hover: bool,
}

impl Default for LogViewerApp {
    fn default() -> Self {
        Self {
            all_lines: vec![],
            filtered: vec![],
            search: String::new(),
            search_lc: String::new(),
            show: [true; 5],
            module_filter: String::new(),
            modules: vec![],
            counts: [0; 5],
            row_height: 20.0,
            font_size: 12.0,
            selected: None,
            detail_open: false,
            status: "Drop a .log / .txt file here or press Ctrl+O".into(),
            drag_hover: false,
        }
    }
}

impl LogViewerApp {
    fn load_text(&mut self, text: &str) {
        self.all_lines = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| parse_log_line(l, i + 1))
            .collect();

        self.counts = [0; 5];
        let mut mod_set: std::collections::BTreeSet<String> = Default::default();
        for l in &self.all_lines {
            self.counts[l.level.index()] += 1;
            if !l.module.is_empty() {
                mod_set.insert(l.module.clone());
            }
        }
        self.modules = mod_set.into_iter().collect();
        self.module_filter.clear();
        self.selected = None;
        self.detail_open = false;
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
        let sl = &self.search_lc;
        let mf = &self.module_filter;
        self.filtered = self
            .all_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                show[l.level.index()]
                    && (mf.is_empty() || l.module == *mf)
                    && (sl.is_empty() || l.raw.to_lowercase().contains(sl.as_str()))
            })
            .map(|(i, _)| i)
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

// ─── UI helpers ──────────────────────────────────────────────────────────────

const BG_BASE: Color32 = Color32::from_rgb(13, 17, 23);
const BG_PANEL: Color32 = Color32::from_rgb(22, 27, 34);
const BG_ROW_HOVER: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 9);
const BG_ROW_SEL: Color32 = Color32::from_rgba_unmultiplied(88, 166, 255, 35);
const COL_BORDER: Color32 = Color32::from_rgb(48, 54, 61);
const COL_TEXT: Color32 = Color32::from_gray(210);
const COL_MUTED: Color32 = Color32::from_gray(120);
const COL_FAINT: Color32 = Color32::from_gray(60);
const COL_ACCENT: Color32 = Color32::from_rgb(88, 166, 255);

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_PANEL;
    v.window_fill = BG_BASE;
    v.override_text_color = Some(COL_TEXT);
    v.widgets.inactive.bg_fill = Color32::from_rgb(30, 36, 44);
    v.widgets.inactive.bg_stroke = Stroke::new(0.5, COL_BORDER);
    v.widgets.hovered.bg_fill = Color32::from_rgb(40, 46, 56);
    v.widgets.hovered.bg_stroke = Stroke::new(0.5, Color32::from_gray(80));
    v.widgets.active.bg_fill = Color32::from_rgb(50, 56, 68);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(88, 166, 255, 50);
    v
}

fn level_toggle_button(ui: &mut egui::Ui, label: &str, count: usize, active: bool, color: Color32) -> bool {
    let text = format!("{} {}", label, count);
    let btn_color = if active { color } else { Color32::from_gray(70) };
    let bg = if active {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28)
    } else {
        Color32::from_rgb(26, 32, 40)
    };
    let stroke = Stroke::new(0.5, if active { Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 160) } else { COL_BORDER });

    let btn = egui::Button::new(
        egui::RichText::new(text)
            .color(btn_color)
            .font(FontId::monospace(11.0))
            .strong(),
    )
    .fill(bg)
    .stroke(stroke)
    .rounding(Rounding::same(4.0));

    ui.add(btn).clicked()
}

// ─── App::update ─────────────────────────────────────────────────────────────

impl App for LogViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        ctx.set_visuals(dark_visuals());

        // ── Drag & drop / file input ──────────────────────────────────────────
        ctx.input(|i| {
            self.drag_hover = !i.raw.hovered_files.is_empty();
            for dropped in &i.raw.dropped_files {
                if let Some(path) = dropped.path.clone() {
                    self.load_file(&path);
                } else if let Some(bytes) = &dropped.bytes {
                    if let Ok(text) = std::str::from_utf8(bytes) {
                        self.load_text(text);
                    }
                }
            }
        });

        // ── Keyboard shortcuts ────────────────────────────────────────────────
        ctx.input(|i| {
            if i.key_pressed(Key::O) && i.modifiers.ctrl {
                self.open_file_dialog();
            }
            if i.key_pressed(Key::Escape) {
                if !self.search.is_empty() {
                    self.search.clear();
                    self.search_lc.clear();
                    self.apply_filters();
                } else {
                    self.selected = None;
                    self.detail_open = false;
                }
            }
            if (i.key_pressed(Key::F) && i.modifiers.ctrl)
                || (i.key_pressed(Key::F) && i.modifiers.command)
            {
                // focus handled by egui request_focus below
            }
        });

        // ─────────────────────────────────────────────────────────────────────
        // TOP TOOLBAR
        // ─────────────────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::none().fill(BG_PANEL).inner_margin(egui::Margin::symmetric(10.0, 6.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    // Logo
                    ui.label(
                        egui::RichText::new("▣  XTR LOG VIEWER")
                            .color(COL_ACCENT)
                            .font(FontId::monospace(13.0))
                            .strong(),
                    );
                    ui.add(egui::Separator::default().vertical().spacing(8.0));

                    // Search box
                    let search_id = egui::Id::new("search_box");
                    let re = ui.add(
                        TextEdit::singleline(&mut self.search)
                            .id(search_id)
                            .hint_text("Search  (Ctrl+F)")
                            .desired_width(240.0)
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
                        ui.add(egui::Separator::default().vertical().spacing(8.0));
                        let label = if self.module_filter.is_empty() {
                            "All modules".to_string()
                        } else if self.module_filter.len() > 28 {
                            format!("…{}", &self.module_filter[self.module_filter.len().saturating_sub(26)..])
                        } else {
                            self.module_filter.clone()
                        };
                        let mut changed = false;
                        egui::ComboBox::from_id_source("mod_cb")
                            .selected_text(egui::RichText::new(label).font(FontId::proportional(12.0)))
                            .width(190.0)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(self.module_filter.is_empty(), "All modules")
                                    .clicked()
                                {
                                    self.module_filter.clear();
                                    changed = true;
                                }
                                for m in self.modules.clone() {
                                    let disp = if m.len() > 40 {
                                        format!("…{}", &m[m.len() - 38..])
                                    } else {
                                        m.clone()
                                    };
                                    if ui.selectable_label(self.module_filter == m, disp).clicked() {
                                        self.module_filter = m;
                                        changed = true;
                                    }
                                }
                            });
                        if changed {
                            self.apply_filters();
                        }
                    }

                    ui.add(egui::Separator::default().vertical().spacing(8.0));

                    // Level toggles
                    let level_defs: [(usize, &str, Color32); 5] = [
                        (0, "ERR", Level::Error.color()),
                        (1, "WRN", Level::Warning.color()),
                        (2, "INF", Level::Info.color()),
                        (3, "DBG", Level::Debug.color()),
                        (4, "TRC", Level::Trace.color()),
                    ];
                    let mut filter_changed = false;
                    for (idx, label, color) in level_defs {
                        if level_toggle_button(ui, label, self.counts[idx], self.show[idx], color) {
                            self.show[idx] = !self.show[idx];
                            filter_changed = true;
                        }
                    }
                    if filter_changed {
                        self.apply_filters();
                    }

                    // Right side
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Font size
                        if ui.small_button("A▲").clicked() {
                            self.font_size = (self.font_size + 1.0).min(20.0);
                            self.row_height = self.font_size + 8.0;
                        }
                        if ui.small_button("A▼").clicked() {
                            self.font_size = (self.font_size - 1.0).max(9.0);
                            self.row_height = self.font_size + 8.0;
                        }

                        ui.add(egui::Separator::default().vertical().spacing(8.0));

                        if !self.all_lines.is_empty() {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Clear").font(FontId::proportional(12.0)),
                                    )
                                    .fill(Color32::from_rgb(30, 36, 44))
                                    .stroke(Stroke::new(0.5, COL_BORDER)),
                                )
                                .clicked()
                            {
                                *self = LogViewerApp::default();
                            }
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Open  Ctrl+O").font(FontId::proportional(12.0)),
                                )
                                .fill(Color32::from_rgb(30, 36, 44))
                                .stroke(Stroke::new(0.5, COL_BORDER)),
                            )
                            .clicked()
                        {
                            self.open_file_dialog();
                        }
                    });
                });
            });

        // ─────────────────────────────────────────────────────────────────────
        // STATUS BAR
        // ─────────────────────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("statusbar")
            .frame(egui::Frame::none().fill(BG_PANEL).inner_margin(egui::Margin::symmetric(10.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let mk = |n: usize, s: &str, c: Color32| {
                        egui::RichText::new(format!("{n} {s}"))
                            .color(c)
                            .font(FontId::monospace(11.0))
                    };
                    ui.label(mk(self.counts[0], "errors", Level::Error.color()));
                    ui.label(mk(self.counts[1], "warnings", Level::Warning.color()));
                    ui.label(mk(self.counts[2], "info", Level::Info.color()));
                    ui.label(mk(self.counts[3], "debug", Level::Debug.color()));
                    ui.label(mk(self.counts[4], "trace", Level::Trace.color()));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {} lines shown",
                                self.filtered.len(),
                                self.all_lines.len()
                            ))
                            .color(COL_MUTED)
                            .font(FontId::monospace(11.0)),
                        );
                        ui.add(egui::Separator::default().vertical());
                        ui.label(
                            egui::RichText::new(&self.status)
                                .color(COL_MUTED)
                                .font(FontId::monospace(11.0)),
                        );
                    });
                });
            });

        // ─────────────────────────────────────────────────────────────────────
        // DETAIL PANEL (bottom — appears when a line is selected)
        // ─────────────────────────────────────────────────────────────────────
        if self.detail_open {
            let selected_line: Option<LogLine> = self
                .selected
                .and_then(|r| self.filtered.get(r).copied())
                .and_then(|li| self.all_lines.get(li))
                .cloned();

            if let Some(line) = selected_line {
                egui::TopBottomPanel::bottom("detail_panel")
                    .resizable(true)
                    .default_height(130.0)
                    .frame(
                        egui::Frame::none()
                            .fill(Color32::from_rgb(17, 22, 30))
                            .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Line detail")
                                    .strong()
                                    .font(FontId::proportional(12.0))
                                    .color(COL_MUTED),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.small_button("✕  close").clicked() {
                                    self.detail_open = false;
                                }
                                if ui
                                    .small_button("⧉  copy raw")
                                    .on_hover_text("Copy raw line to clipboard")
                                    .clicked()
                                {
                                    ui.output_mut(|o| o.copied_text = line.raw.clone());
                                }
                            });
                        });
                        ui.add_space(4.0);

                        egui::Grid::new("detail_grid")
                            .num_columns(2)
                            .spacing([12.0, 3.0])
                            .show(ui, |ui| {
                                let lbl = |s: &str| {
                                    egui::RichText::new(s)
                                        .color(COL_FAINT)
                                        .font(FontId::monospace(11.0))
                                };
                                let val = |s: String| {
                                    egui::RichText::new(s)
                                        .color(COL_TEXT)
                                        .font(FontId::monospace(11.0))
                                };

                                ui.label(lbl("line"));
                                ui.label(val(line.num.to_string()));
                                ui.end_row();

                                ui.label(lbl("level"));
                                ui.label(
                                    egui::RichText::new(line.level.label())
                                        .color(line.level.color())
                                        .strong()
                                        .font(FontId::monospace(11.0)),
                                );
                                ui.end_row();

                                ui.label(lbl("time"));
                                ui.label(val(line.timestamp.clone()));
                                ui.end_row();

                                ui.label(lbl("module"));
                                ui.label(val(line.module.clone()));
                                ui.end_row();
                            });

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("message").color(COL_FAINT).font(FontId::monospace(11.0)));
                        ui.add_space(2.0);
                        ScrollArea::vertical()
                            .id_source("detail_scroll")
                            .max_height(60.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(&line.message)
                                        .font(FontId::monospace(11.5))
                                        .color(COL_TEXT),
                                );
                            });
                    });
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // MAIN LOG AREA
        // ─────────────────────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_BASE))
            .show(ctx, |ui| {
                // ── Empty / drop zone ────────────────────────────────────────
                if self.all_lines.is_empty() {
                    let outer = ui.available_rect_before_wrap();
                    if self.drag_hover {
                        ui.painter().rect(
                            outer.shrink(4.0),
                            Rounding::same(8.0),
                            Color32::from_rgba_unmultiplied(88, 166, 255, 25),
                            Stroke::new(2.0, COL_ACCENT),
                        );
                    }
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(60.0);
                            ui.label(egui::RichText::new("◫").size(52.0).color(Color32::from_gray(45)));
                            ui.add_space(14.0);
                            ui.label(
                                egui::RichText::new("Drop a log file here")
                                    .size(22.0)
                                    .color(Color32::from_gray(170)),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(
                                    "Supports XTR · MTF · CARIAD · TLS-Attacker · DiagnosticToolBox formats",
                                )
                                .size(12.0)
                                .color(COL_FAINT),
                            );
                            ui.add_space(20.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("  Open file  (Ctrl+O)  ")
                                            .size(13.0)
                                            .color(Color32::from_gray(10)),
                                    )
                                    .fill(COL_ACCENT)
                                    .stroke(Stroke::NONE)
                                    .rounding(Rounding::same(6.0)),
                                )
                                .clicked()
                            {
                                self.open_file_dialog();
                            }
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("Ctrl+O  open  |  Ctrl+F  search  |  Esc  clear/deselect  |  Click row  →  detail")
                                    .size(11.0)
                                    .color(Color32::from_gray(50)),
                            );
                        });
                    });
                    return;
                }

                // ── No results ───────────────────────────────────────────────
                if self.filtered.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("No lines match your filters")
                                .size(16.0)
                                .color(COL_FAINT),
                        );
                    });
                    return;
                }

                // ── Column header ────────────────────────────────────────────
                let col_ln_w: f32 = 48.0;
                let col_ts_w: f32 = 82.0;
                let col_lv_w: f32 = 34.0;
                let col_mod_w: f32 = 200.0;

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let hdr = |s: &str| {
                        egui::RichText::new(s)
                            .font(FontId::monospace(10.0))
                            .color(Color32::from_gray(50))
                            .strong()
                    };
                    ui.add_space(col_ln_w - 4.0);
                    ui.label(hdr(" TIME       "));
                    ui.add_space(col_ts_w - 70.0);
                    ui.label(hdr("LVL "));
                    ui.add_space(col_lv_w - 28.0);
                    ui.label(hdr("MODULE"));
                    ui.add_space(col_mod_w - 42.0);
                    ui.label(hdr("MESSAGE"));
                });

                ui.add(egui::Separator::default().horizontal().spacing(2.0));

                // ── Virtual scroll ───────────────────────────────────────────
                let row_h = self.row_height;
                let font_sz = self.font_size;
                let n = self.filtered.len();

                ScrollArea::vertical()
                    .id_source("log_scroll")
                    .auto_shrink(false)
                    .show_rows(ui, row_h, n, |ui, row_range| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;

                        for row_idx in row_range {
                            let line_idx = match self.filtered.get(row_idx) {
                                Some(&i) => i,
                                None => continue,
                            };
                            let line = match self.all_lines.get(line_idx) {
                                Some(l) => l,
                                None => continue,
                            };

                            let is_sel = self.selected == Some(row_idx);

                            let (row_rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), row_h),
                                Sense::click(),
                            );

                            if !ui.is_rect_visible(row_rect) {
                                continue;
                            }

                            // Background
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

                            // Left accent bar for ERR/WRN
                            if matches!(line.level, Level::Error | Level::Warning) {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(row_rect.min, Vec2::new(2.0, row_h)),
                                    Rounding::ZERO,
                                    line.level.color(),
                                );
                            }

                            let paint = ui.painter();
                            let y = row_rect.center().y;
                            let fid = FontId::monospace(font_sz);
                            let fid_sm = FontId::monospace(font_sz - 1.0);
                            let mut x = row_rect.min.x;

                            // Line number
                            paint.text(
                                egui::pos2(x + col_ln_w - 6.0, y),
                                Align2::RIGHT_CENTER,
                                &format!("{}", line.num),
                                FontId::monospace(font_sz - 1.0),
                                COL_FAINT,
                            );
                            x += col_ln_w;

                            // Timestamp
                            let ts = if line.timestamp.len() > 12 { &line.timestamp[..12] } else { &line.timestamp };
                            paint.text(egui::pos2(x, y), Align2::LEFT_CENTER, ts, fid_sm.clone(), Color32::from_gray(80));
                            x += col_ts_w;

                            // Level
                            paint.text(
                                egui::pos2(x, y),
                                Align2::LEFT_CENTER,
                                line.level.label(),
                                fid_sm.clone(),
                                line.level.color(),
                            );
                            x += col_lv_w;

                            // Module (truncated)
                            let mod_str = &line.module;
                            let mod_disp: &str = if mod_str.len() > 28 {
                                &mod_str[..28]
                            } else {
                                mod_str.as_str()
                            };
                            paint.text(
                                egui::pos2(x, y),
                                Align2::LEFT_CENTER,
                                mod_disp,
                                fid_sm.clone(),
                                Color32::from_gray(100),
                            );
                            x += col_mod_w;

                            // Message (truncated to available width — painter clips automatically)
                            let max_chars = ((row_rect.max.x - x - 8.0) / (font_sz * 0.6)) as usize;
                            let msg = &line.message;
                            let msg_disp: &str = if msg.len() > max_chars.max(40) {
                                &msg[..max_chars.max(40)]
                            } else {
                                msg.as_str()
                            };

                            let msg_color = match line.level {
                                Level::Error => Color32::from_rgb(255, 130, 115),
                                Level::Warning => Color32::from_rgb(235, 185, 70),
                                _ => COL_TEXT,
                            };

                            paint.text(
                                egui::pos2(x, y),
                                Align2::LEFT_CENTER,
                                msg_disp,
                                fid.clone(),
                                msg_color,
                            );

                            // Click handler
                            if resp.clicked() {
                                if is_sel {
                                    self.detail_open = !self.detail_open;
                                } else {
                                    self.selected = Some(row_idx);
                                    self.detail_open = true;
                                }
                            }
                        }
                    });
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
