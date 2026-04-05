                ui.add(egui::Separator::default().horizontal().spacing(1.0));

                let row_h = self.row_height;
                let font_sz = self.font_size;
                let n = self.filtered.len();
                let visible_height = ui.available_height();

                let mut sa = ScrollArea::vertical().id_source("log_scroll").auto_shrink(false)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                if let Some(off) = self.scroll_to_offset.take() { 
                    sa = sa.scroll_offset(Vec2::new(0.0, off)); 
                }

                let out = sa.show_rows(ui, row_h, n, |ui, row_range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    for row_idx in row_range {
                        let line_idx = match self.filtered.get(row_idx) { Some(&i) => i, None => continue };
                        let line = match self.all_lines.get(line_idx) { Some(l) => l, None => continue };
                        let is_sel = self.selected == Some(row_idx);
                        let is_find_match = self.find_matches.binary_search(&row_idx).is_ok();
                        let is_current_find = is_find_match && 
                            self.find_matches.get(self.find_current_match) == Some(&row_idx);
                        let is_bookmarked = self.is_bookmarked(row_idx);
                        let nav_kind: Option<NavKind> = self.nav_entries.iter()
                            .find(|e| e.row_idx == row_idx && e.kind != NavKind::Bookmark)
                            .map(|e| e.kind);

                        let (row_rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        if !ui.is_rect_visible(row_rect) { continue; }

                        // Background
                        let bg = if is_sel {
                            BG_ROW_SEL
                        } else if is_current_find {
                            Color32::from_rgba_unmultiplied(255, 180, 40, 55)
                        } else if is_find_match {
                            Color32::from_rgba_unmultiplied(200, 150, 30, 28)
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

                        // Bookmark indicator
                        if is_bookmarked {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_h)),
                                Rounding::ZERO,
                                Color32::from_rgb(255, 140, 200)
                            );
                        }

                        // Left accent bar for errors/warnings
                        if matches!(line.level, Level::Error | Level::Warning) {
                            let x_off = if is_bookmarked { 3.0 } else { 0.0 };
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.min.x + x_off, row_rect.min.y), 
                                    Vec2::new(2.5 - x_off, row_h)),
                                Rounding::ZERO, line.level.color());
                        }

                        // Right nav landmark indicator
                        if let Some(kind) = nav_kind {
                            let c = kind.color();
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(egui::pos2(row_rect.max.x - 3.0, row_rect.min.y), Vec2::new(3.0, row_h)),
                                Rounding::ZERO, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 140),
                            );
                        }

                        let p = ui.painter();
                        let y = row_rect.center().y;
                        let fid = FontId::monospace(font_sz);
                        let fsm = FontId::monospace((font_sz - 1.0).max(8.0));
                        let fxs = FontId::monospace((font_sz - 2.0).max(7.5));
                        let mut x = row_rect.min.x + if is_bookmarked { 6.0 } else { 4.0 };

                        // Line number
                        p.text(egui::pos2(x + COL_LN - 10.0, y), Align2::RIGHT_CENTER, 
                            line.num.to_string(), fxs.clone(), COL_FAINT);
                        x += COL_LN;
                        
                        // Timestamp
                        let ts = if line.timestamp.len() > 12 { &line.timestamp[..12] } else { &line.timestamp };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, ts, fsm.clone(), Color32::from_rgb(160, 210, 255));
                        x += COL_TS;
                        
                        // Delta time
                        if let Some(dms) = line.delta_ms { 
                            if dms > 0 {
                                let dc = if dms >= 1000 { Color32::from_rgb(255, 200, 80) } 
                                    else if dms >= 100 { Color32::from_rgb(180, 180, 200) } 
                                    else { Color32::from_rgb(120, 130, 150) };
                                p.text(egui::pos2(x, y), Align2::LEFT_CENTER, format_delta(dms), fxs.clone(), dc);
                            }
                        }
                        x += COL_DT;
                        
                        // Level
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, line.level.label(), fsm.clone(), line.level.color());
                        x += COL_LV;
                        
                        // Module
                        let md = if line.module.len() > 22 { &line.module[..22] } else { &line.module };
                        p.text(egui::pos2(x, y), Align2::LEFT_CENTER, md, fsm.clone(), Color32::from_rgb(180, 185, 200));
                        x += COL_MOD;
                        
                        // Message
                        let msg = &line.message;
                        let msg_col = match line.level {
                            Level::Error => Color32::from_rgb(255, 180, 170),
                            Level::Warning => Color32::from_rgb(255, 220, 150),
                            _ => Color32::from_rgb(210, 215, 225)
                        };
                        
                        let available_width = row_rect.max.x - x - 8.0;
                        if self.wrap_lines {
                            // For wrapped lines, we'd need more complex rendering
                            // For now, just show what fits
                            let max_chars = (available_width / (font_sz * 0.6)) as usize;
                            let msg_disp = if msg.len() > max_chars.max(40) { &msg[..max_chars.max(40)] } else { msg.as_str() };
                            p.text(egui::pos2(x, y), Align2::LEFT_CENTER, msg_disp, fid.clone(), msg_col);
                        } else {
                            let max_chars = (available_width / (font_sz * 0.6)) as usize;
                            let msg_disp = if msg.len() > max_chars.max(40) { 
                                format!("{}…", &msg[..max_chars.max(40).saturating_sub(1)]) 
                            } else { 
                                msg.to_string() 
                            };
                            p.text(egui::pos2(x, y), Align2::LEFT_CENTER, msg_disp, fid.clone(), msg_col);
                        }

                        if resp.clicked() {
                            if is_sel { 
                                self.detail_open = !self.detail_open; 
                            } else { 
                                self.selected = Some(row_idx);
                                self.detail_open = true; 
                            }
                        }
                        
                        // Double-click to toggle bookmark
                        if resp.double_clicked() {
                            self.toggle_bookmark(row_idx);
                        }
                    }
                });

                self.scroll_area_height = visible_height;
                self.current_scroll_offset = out.state.offset.y;
            });
    }
}

// ─── Find Dialog Tab Renderers ───────────────────────────────────────────────

impl LogViewerApp {
    fn render_find_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 10.0;
        
        // Find what row
        ui.horizontal(|ui| {
            ui.label(RichText::new("Find what:").font(FontId::proportional(12.0)).color(COL_TEXT));
            ui.add_space(8.0);
            
            // Dropdown with history
            let response = ui.add(
                TextEdit::singleline(&mut self.find_what)
                    .desired_width(280.0)
                    .font(FontId::monospace(12.0))
            );
            if response.changed() {
                self.recompute_find_matches();
            }
        });
        
        ui.add_space(8.0);
        
        // Two column layout: options on left, buttons on right
        ui.horizontal(|ui| {
            // Left: Options
            ui.vertical(|ui| {
                ui.set_width(240.0);
                
                ui.checkbox(&mut self.find_backward, RichText::new("Backward direction").font(FontId::proportional(11.0)).color(COL_TEXT));
                ui.checkbox(&mut self.find_whole_word, RichText::new("Match whole word only").font(FontId::proportional(11.0)).color(COL_TEXT));
                ui.checkbox(&mut self.find_match_case, RichText::new("Match case").font(FontId::proportional(11.0)).color(COL_TEXT));
                ui.checkbox(&mut self.find_wrap_around, RichText::new("Wrap around").font(FontId::proportional(11.0)).color(COL_TEXT));
                
                ui.add_space(12.0);
                
                // Search mode group
                ui.label(RichText::new("Search Mode").font(FontId::monospace(10.0)).color(COL_FAINT).strong());
                ui.add_space(4.0);
                ui.radio_value(&mut self.find_search_mode, SearchMode::Normal, 
                    RichText::new("Normal").font(FontId::proportional(11.0)).color(COL_TEXT));
                ui.radio_value(&mut self.find_search_mode, SearchMode::Extended, 
                    RichText::new("Extended (\\n, \\r, \\t, \\0, \\x...)").font(FontId::proportional(11.0)).color(COL_TEXT));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.find_search_mode, SearchMode::Regex, 
                        RichText::new("Regular expression").font(FontId::proportional(11.0)).color(COL_TEXT));
                    if self.find_search_mode == SearchMode::Regex {
                        ui.checkbox(&mut self.find_regex_dotall, RichText::new(". matches newline").font(FontId::proportional(10.0)).color(COL_MUTED));
                    }
                });
            });
            
            ui.add_space(20.0);
            
            // Right: Action buttons
            ui.vertical(|ui| {
                ui.set_width(140.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                
                let nav_ok = self.find_total_matches > 0;
                
                if ui.add(Button::new(RichText::new("Find Next").color(if nav_ok { COL_TEXT } else { COL_FAINT }).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, if nav_ok { COL_ACCENT } else { COL_BORDER }))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() && nav_ok {
                    self.find_next();
                }
                
                if ui.add(Button::new(RichText::new("Find Previous").color(if nav_ok { COL_TEXT } else { COL_FAINT }).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, COL_BORDER))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() && nav_ok {
                    self.find_prev();
                }
                
                ui.add_space(4.0);
                
                if ui.add(Button::new(RichText::new("Count").color(COL_TEXT).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, COL_BORDER))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() {
                    let count = self.count_matches();
                    self.status = format!("Found {} matches", count);
                }
                
                if ui.add(Button::new(RichText::new("Find All in Current").color(COL_TEXT).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, COL_BORDER))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() {
                    self.find_all_current();
                }
            });
        });
        
        // Match counter
        if self.find_total_matches > 0 {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Match {} of {}", self.find_current_match + 1, self.find_total_matches))
                    .font(FontId::monospace(11.0)).color(COL_ACCENT));
            });
        }
    }
    
    fn render_replace_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 10.0;
        
        // Find what
        ui.horizontal(|ui| {
            ui.label(RichText::new("Find what:").font(FontId::proportional(12.0)).color(COL_TEXT));
            ui.add_space(8.0);
            ui.add(TextEdit::singleline(&mut self.find_what).desired_width(280.0).font(FontId::monospace(12.0)));
        });
        
        // Replace with
        ui.horizontal(|ui| {
            ui.label(RichText::new("Replace with:").font(FontId::proportional(12.0)).color(COL_TEXT));
            ui.add_space(8.0);
            ui.add(TextEdit::singleline(&mut self.replace_with).desired_width(280.0).font(FontId::monospace(12.0)));
        });
        
        ui.add_space(8.0);
        
        // Options and buttons
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(240.0);
                ui.checkbox(&mut self.find_match_case, RichText::new("Match case").font(FontId::proportional(11.0)).color(COL_TEXT));
                ui.checkbox(&mut self.find_whole_word, RichText::new("Match whole word only").font(FontId::proportional(11.0)).color(COL_TEXT));
            });
            
            ui.add_space(20.0);
            
            ui.vertical(|ui| {
                ui.set_width(140.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                
                if ui.add(Button::new(RichText::new("Find Next").color(COL_TEXT).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, COL_BORDER))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() {
                    self.find_next();
                }
                
                if ui.add(Button::new(RichText::new("Replace").color(COL_TEXT).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, COL_BORDER))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() {
                    // Replace functionality would go here
                }
                
                if ui.add(Button::new(RichText::new("Replace All").color(COL_TEXT).font(FontId::proportional(12.0)))
                    .fill(Color32::from_rgb(40, 50, 65))
                    .stroke(Stroke::new(0.5, COL_BORDER))
                    .rounding(Rounding::same(4.0))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() {
                    // Replace all functionality would go here
                }
            });
        });
    }
    
    fn render_find_in_files_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("Find in Files").font(FontId::proportional(14.0)).color(COL_TEXT));
            ui.add_space(8.0);
            ui.label(RichText::new("This feature searches across multiple log files.\nSelect a folder to search.").font(FontId::proportional(11.0)).color(COL_MUTED));
            ui.add_space(16.0);
            if ui.add(primary_button("Browse for Folder…")).clicked() {
                // Folder selection would go here
            }
        });
    }
    
    fn render_mark_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("Mark").font(FontId::proportional(14.0)).color(COL_TEXT));
            ui.add_space(8.0);
            ui.label(RichText::new("Bookmark all lines matching the search criteria.").font(FontId::proportional(11.0)).color(COL_MUTED));
            ui.add_space(16.0);
            if ui.add(primary_button("Bookmark All Matches")).clicked() {
                self.recompute_find_matches();
                for &row_idx in &self.find_matches {
                    if !self.is_bookmarked(row_idx) {
                        self.bookmarks.push(row_idx);
                    }
                }
                self.bookmarks.sort_unstable();
                self.recompute_nav();
            }
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
