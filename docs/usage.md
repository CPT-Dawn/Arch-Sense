# Usage

The dashboard, every key and mouse action, and every command. Keys come from [`src/app.rs`](../src/app.rs) and commands from [`src/cli.rs`](../src/cli.rs) (`arch-sense --help`).

## The dashboard

- **Top:** gauge cards for CPU, GPU, Fans and Battery.
- **Below:** one settings list, grouped into **Performance**, **Battery**, **System** and **Lighting**. In Lighting, a live keyboard preview appears at the bottom.
- **Footer:** confirms each change, or tells you why it didn't work.

There is no Apply button: changes take effect the moment you make them. Only battery calibration asks first, and it needs a real <kbd>y</kbd>.

The terminal needs at least **80×24** cells; any smaller and the app asks for more room. `NO_COLOR` turns colour off, and terminals without `COLORTERM=truecolor` get a 16-colour palette.

## Keys

| Key | Action |
| --- | --- |
| <kbd>↑</kbd> <kbd>↓</kbd> or <kbd>k</kbd> <kbd>j</kbd> | Move between settings |
| <kbd>←</kbd> <kbd>→</kbd> or <kbd>h</kbd> <kbd>l</kbd> | Change the selected setting |
| <kbd>Enter</kbd> / <kbd>Space</kbd> | Toggle, run, or step forward |
| <kbd>Tab</kbd> / <kbd>Shift</kbd>+<kbd>Tab</kbd> | Jump between sections |
| <kbd>#</kbd> | Type a custom hex colour (on the Colour row) |
| <kbd>?</kbd> | Help |
| <kbd>d</kbd> | Doctor |
| <kbd>Esc</kbd> | Close any dialog |
| <kbd>q</kbd> / <kbd>Ctrl</kbd>+<kbd>C</kbd> | Quit |

In the Doctor, <kbd>↑</kbd> <kbd>↓</kbd>, <kbd>PgUp</kbd> <kbd>PgDn</kbd> and <kbd>Home</kbd> scroll.

## Mouse

Click a row to select it, click an option or swatch to set it, and click the selected row to step it forward (right-click steps back). Scroll to move.

## Commands

| Command | What it does |
| --- | --- |
| `arch-sense` | Open the dashboard |
| `arch-sense doctor` | Check your setup and explain how to fix anything broken; exits non-zero on errors |
| `arch-sense --demo` | Try the full app on a simulated laptop; touches nothing real |
| `arch-sense --apply` | Reapply Saved Lighting without opening the app (what Boot Restore runs) |
| `arch-sense --help` | Show all options |
| `arch-sense --version` | Print the version |

[Back to the README](../README.md)
