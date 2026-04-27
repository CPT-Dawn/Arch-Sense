# Arch-Sense UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Overhaul the Arch-Sense TUI to achieve the "Refined Predator" aesthetic, removing clutter from the Keyboard panel and introducing a modern status-bar footer with dynamic keybind chips.

**Architecture:** We will modify the `ratatui` UI layer (`src/ui.rs`), the color theme (`src/theme.rs`), and the context hints (`src/app.rs`) without changing the underlying hardware or state logic.

**Tech Stack:** Rust, Ratatui, Crossterm

---

### Task 1: Expand the Theme Palette

**Files:**
- Modify: `src/theme.rs`

- [ ] **Step 1: Add semantic colors for row highlights and keybind chips.**
In `src/theme.rs`, locate the `Backgrounds & Surfaces` and `Semantic & Status Colors` sections. We need an `ELEVATED` background for selected rows, and specific colors for the new footer chips.

```rust
// Replace the existing ELEVATED constant with a defined color
pub(crate) const ELEVATED: Option<Color> = Some(Color::Rgb(28, 36, 51)); // Deep navy/slate

// Add new colors for footer chips (in the Accents or Semantic sections)
pub(crate) const CHIP_BG: Color = Color::Rgb(30, 41, 59); // Slate-800
pub(crate) const CHIP_FG: Color = Color::Rgb(148, 163, 184); // Slate-400
pub(crate) const CHIP_HIGHLIGHT_BG: Color = Color::Rgb(14, 165, 233); // Sky-500
pub(crate) const CHIP_HIGHLIGHT_FG: Color = Color::Rgb(15, 23, 42); // Slate-900
```

- [ ] **Step 2: Build and verify compilation.**
Run: `cargo check`
Expected: Success.

- [ ] **Step 3: Commit**
```bash
git add src/theme.rs
git commit -m "style: expand theme palette for UI redesign"
```

---

### Task 2: Implement Row Highlighting in Controls Panel

**Files:**
- Modify: `src/ui.rs`

- [ ] **Step 1: Update `draw_controls` to apply full-row backgrounds.**
In `src/ui.rs`, locate `draw_controls`. Modify the `rows` iterator to construct a `Style` for the entire row when selected, instead of just styling individual cells.

```rust
// Inside draw_controls, find the mapping of app.controls:
    let rows = app
        .controls
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let selected = app.focus == FocusPanel::Controls && index == app.selected_control;
            let pending = item.pending.is_some();
            let error = item.last_error.is_some();
            
            // Define the row background style
            let row_style = if selected {
                style_with_bg(Style::new(), Theme::ELEVATED)
            } else {
                Style::new()
            };

            let base_style = if selected {
                Style::new().fg(Theme::TEXT_PRIMARY).bold()
            } else {
                Style::new().fg(Theme::TEXT_PRIMARY)
            };
            
            let value_style = if error {
                Style::new().fg(Theme::STATE_ERROR)
            } else if pending {
                Style::new().fg(Theme::STATE_WARNING).bold()
            } else if selected {
                Style::new().fg(Theme::VALUE_SELECTED).bold()
            } else {
                Style::new().fg(Theme::VALUE_PRIMARY)
            };
            
            let marker = if selected { "▸ " } else { "  " };
            let state = if app.control_pending == Some(item.id) {
                "APPLY"
            } else if pending {
                "PREVIEW"
            } else if error {
                "ERROR"
            } else {
                ""
            };

            Row::new(vec![
                Cell::from(marker).style(base_style),
                Cell::from(item.label()).style(base_style),
                Cell::from(item.visible_value()).style(value_style),
                Cell::from(state).style(Style::new().fg(control_state_color(
                    app.control_pending == Some(item.id),
                    pending,
                    error,
                ))),
            ]).style(row_style) // Apply the background to the whole row
        })
        .collect::<Vec<_>>();
```

- [ ] **Step 2: Build and run.**
Run: `cargo run` (or `cargo check` if hardware is unavailable)
Expected: TUI renders, and navigating the Controls panel shows a distinct background highlight on the selected row.

- [ ] **Step 3: Commit**
```bash
git add src/ui.rs
git commit -m "ui: add full-row highlighting to Controls panel"
```

---

### Task 3: Cleanup Keyboard Panel & Implement Row Highlighting

**Files:**
- Modify: `src/ui.rs`

- [ ] **Step 1: Remove RGB Preview Spans and State line.**
In `src/ui.rs`, locate `draw_rgb`. Change the layout splitting to remove the preview area.

```rust
// In draw_rgb:
    let [rows_area, palette_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .spacing(SPACING)
    .areas(content_area);

    draw_rgb_rows(frame, rows_area, app);
    draw_palette(frame, palette_area, app);
    // REMOVE: draw_rgb_preview(frame, preview_area, app);
```

- [ ] **Step 2: Delete the removed functions.**
Delete the `draw_rgb_preview` and `rgb_preview_spans` functions entirely from `src/ui.rs`.

- [ ] **Step 3: Update `draw_rgb_rows` for full-row highlighting.**
Change `draw_rgb_rows` to use Ratatui's `Table` widget instead of `Paragraph` with `Line`s, so we can apply full-row backgrounds just like in the Controls panel.

```rust
// Replace draw_rgb_rows with:
fn draw_rgb_rows(frame: &mut Frame, area: Rect, app: &App) {
    let effect = app.rgb.effect();
    let fields = [
        (RgbField::Effect, effect.name.to_string()),
        (RgbField::Color, color_value(app)),
        (RgbField::Brightness, format!("{}%", app.rgb.brightness)),
        (RgbField::Speed, format!("{}%", app.rgb.speed)),
        (RgbField::Direction, direction_value(app)),
    ];

    let rows = fields
        .into_iter()
        .enumerate()
        .map(|(index, (field, value))| {
            let selected = app.focus == FocusPanel::Rgb && index == app.selected_rgb_field;
            
            let row_style = if selected {
                style_with_bg(Style::new(), Theme::ELEVATED)
            } else {
                Style::new()
            };

            let style = if selected {
                Style::new().fg(Theme::TEXT_PRIMARY).bold()
            } else {
                Style::new().fg(Theme::TEXT_PRIMARY)
            };
            
            let value_style = if selected {
                Style::new().fg(Theme::VALUE_SELECTED).bold()
            } else {
                Style::new().fg(Theme::VALUE_PRIMARY)
            };

            Row::new(vec![
                Cell::from(if selected { "▸ " } else { "  " }).style(style),
                Cell::from(field.label()).style(style),
                Cell::from(value).style(value_style),
            ]).style(row_style)
        })
        .collect::<Vec<_>>();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(40),
        Constraint::Percentage(60),
    ];

    frame.render_widget(Table::new(rows, widths), area);
}
```

- [ ] **Step 4: Build and test.**
Run: `cargo run`
Expected: The Keyboard panel no longer shows the animated "━" strip or State line. The rows (Mode, Color, etc.) are highlighted cleanly when focused.

- [ ] **Step 5: Commit**
```bash
git add src/ui.rs
git commit -m "ui: simplify Keyboard panel and add row highlights"
```

---

### Task 4: Format Context Hints as Keybind Chips

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui.rs`

- [ ] **Step 1: Structured context hints in `app.rs`.**
Instead of returning a single long string, we want `context_hint` to return a structured list of key/action pairs so the UI can render them as chips. We will return `Vec<(&'static str, String)>`.

Modify `src/app.rs`:

```rust
// Change the signature and implementation in `impl App`:

    pub(crate) fn context_hint(&self) -> Vec<(&'static str, String)> {
        match self.focus {
            FocusPanel::Controls => self.controls_context(),
            FocusPanel::Rgb => self.rgb_context(),
            FocusPanel::Sensors => {
                vec![("R", "Refresh".to_string())]
            }
        }
    }

    fn controls_context(&self) -> Vec<(&'static str, String)> {
        let Some(item) = self.selected_control() else {
            return vec![("R", "Refresh".to_string()), ("Q", "Quit".to_string())];
        };

        if let Some(choice) = item.pending_choice() {
            return vec![
                ("Enter", "Apply".to_string()),
                ("Esc", "Cancel".to_string()),
            ];
        }

        match &item.kind {
            ControlKind::Toggle => {
                vec![("Enter", "Toggle".to_string())]
            }
            ControlKind::Choice(_) => {
                vec![
                    ("Left/Right", "Select".to_string()),
                    ("Enter", "Apply".to_string()),
                ]
            }
        }
    }

    fn rgb_context(&self) -> Vec<(&'static str, String)> {
        let mut hints = vec![
            ("Left/Right", "Adjust".to_string()),
            ("Enter", "Apply".to_string()),
        ];
        if self.rgb_dirty {
            hints.push(("Unsaved", "Preview".to_string()));
        }
        hints
    }
```

- [ ] **Step 2: Render chips in `draw_footer` in `ui.rs`.**
Modify `draw_footer` to render the structured hints as stylized chips.

```rust
// Inside `draw_footer` in src/ui.rs, replace the `Paragraph::new` for `context`:
    
    let mut context_spans = vec![
        Span::styled(" [Tab] ", Style::new().bg(Theme::CHIP_BG).fg(Theme::CHIP_FG).bold()),
        Span::styled(" Switch Panel ", Style::new().fg(Theme::TEXT_SECONDARY)),
        Span::raw("   "),
        Span::styled(" [Q] ", Style::new().bg(Theme::CHIP_BG).fg(Theme::CHIP_FG).bold()),
        Span::styled(" Quit ", Style::new().fg(Theme::TEXT_SECONDARY)),
        Span::styled("  |  ", Style::new().fg(Theme::BORDER_IDLE)),
    ];

    for (key, action) in app.context_hint() {
        context_spans.push(Span::styled(format!(" [{key}] "), Style::new().bg(Theme::CHIP_HIGHLIGHT_BG).fg(Theme::CHIP_HIGHLIGHT_FG).bold()));
        context_spans.push(Span::styled(format!(" {action} "), Style::new().fg(Theme::TEXT_PRIMARY)));
        context_spans.push(Span::raw("  "));
    }

    frame.render_widget(
        Paragraph::new(Line::from(context_spans)),
        context,
    );
```

- [ ] **Step 3: Build and test.**
Run: `cargo run`
Expected: The footer displays keybinds in a blocky, modern "chip" format (`[Tab] Switch Panel`, etc.) and the hints update contextually based on the active panel.

- [ ] **Step 4: Commit**
```bash
git add src/app.rs src/ui.rs
git commit -m "ui: implement modern status bar with keybind chips"
```

---
