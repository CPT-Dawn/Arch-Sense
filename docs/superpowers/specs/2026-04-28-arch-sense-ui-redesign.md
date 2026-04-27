# Arch-Sense UI/UX Redesign Specification

## 1. Goal
Overhaul the `arch-sense` TUI to achieve a polished, modern, and professional aesthetic (the "Refined Predator" theme). The redesign must prioritize usable space, clear visual hierarchy, consistent theming, and an intuitive user experience.

## 2. Core Requirements

### 2.1 Keyboard Panel Cleanup
- **Remove** the "State" indicator line from the top of the RGB preview area.
- **Remove** the animated RGB character strip (`rgb_preview_spans`).
- **Retain** the palette selection block, but compact it if necessary to fit alongside the mode/color/speed rows.
- **Goal:** Reclaim vertical space so the application remains fully functional and legible in smaller terminal windows.

### 2.2 Global Theme & Aesthetic ("Refined Predator")
- **Layout:** Maintain the 3-panel split (Controls, Keyboard, Sensors) but improve inner padding and spacing.
- **Borders:** Retain rounded borders (`ROUNDED`) but use them purely for the outer panel boundaries. Unfocused panels should use a subtle slate/gray (`Theme::BORDER_IDLE`), while the focused panel highlights with an electric blue/cyan (`Theme::BORDER_FOCUS`).
- **Color Palette Expansion:** Introduce new semantic colors to `src/theme.rs` to support the refined look:
    - Primary Text: Bright white/ice blue.
    - Secondary Text: Slate/muted blue.
    - Accents: Electric Cyan for primary selections, subtle Amber for warnings, Soft Green for success/normal temps.
    - Backgrounds (Optional): Surface colors for elevated rows if the terminal supports background rendering without breaking transparency.

### 2.3 Interactive Elements & Navigation
- **Selection Highlight:** The currently selected row in any active panel must be visually distinct. Instead of just a `▸` marker, the entire row should receive a background highlight (`Theme::ELEVATED` or a reversed text style) to clearly indicate focus.
- **Inactive Panels:** Items in unfocused panels must dim their selection markers to prevent confusion about where the user's input will go.

### 2.4 Footer & Keybinds Redesign
- **Status Bar Style:** Convert the footer from a plain text block into a modern "Status Bar".
- **Keybind Chips:** Format keybinds as distinct "chips" or pills. For example, instead of `Tab switch panels | r refresh | q quit`, use styled segments: `[Tab] Switch Panels  [R] Refresh  [Q] Quit  [Esc] Cancel`.
- **Dynamic Context:** Ensure the contextual hints on the right side of the footer update dynamically based on the focused panel and selected item, matching the new styling.

## 3. Implementation Details

### 3.1 Files to Modify
- `src/ui.rs`: Major overhaul of `draw_body`, `draw_rgb`, `draw_rgb_rows`, `draw_rgb_preview`, and `draw_footer`. Remove the RGB preview spans entirely. Implement row highlighting.
- `src/theme.rs`: Expand the color palette to support the "Refined Predator" aesthetic, adding specific colors for keybind chips and row highlights.
- `src/app.rs`: Ensure the `context_hint` logic outputs text that the new footer renderer can easily parse into keybind "chips".

### 3.2 Layout Adjustments (`ui.rs`)
- Remove the `[rows_area, palette_area, preview_area]` split in `draw_rgb` and replace it with a simpler `[rows_area, palette_area]` split, giving more room to the rows or allowing the overall UI to shrink.
- Update the table/row rendering in `draw_controls` and `draw_rgb_rows` to apply a background color to the entire selected row.

### 3.3 Visual Hierarchy Rules
1. **Highest Contrast:** The currently focused panel border and the currently selected item row within that panel.
2. **Medium Contrast:** Titles of unfocused panels, values of unselected items.
3. **Lowest Contrast:** Descriptions, borders of unfocused panels, static labels.

## 4. Acceptance Criteria
- [ ] The TUI compiles and runs without error.
- [ ] The RGB preview strip and State line are completely removed from the UI.
- [ ] The selected item in the focused panel has a clear background highlight (or strong visual distinction beyond just a text marker).
- [ ] The footer displays keybinds as visually distinct "chips".
- [ ] The application remains usable and looks complete when resized to smaller terminal heights (e.g., 24 lines).
- [ ] The color scheme consistently applies the "Refined Predator" aesthetic across all components.