# The README demo GIF, recorded with VHS (https://github.com/charmbracelet/vhs)
# on the simulated laptop from `arch-sense --demo`. Nothing real is touched.
#
# From the repo root:
#   cargo build --release
#   vhs docs/demo.tape          # writes docs/demo.gif
#
# The window is sized for roughly 120x36 cells; nudge Width/Height if your
# font renders wider or taller.

Output docs/demo.gif

Set Shell "bash"
Set FontSize 14
Set Width 1060
Set Height 640
Set Padding 20
Set TypingSpeed 60ms
Set Theme "Catppuccin Mocha"
Env COLORTERM "truecolor"

# Put the freshly built binary on PATH without showing the plumbing.
Hide
Type "export PATH=$PWD/target/release:$PATH && clear"
Enter
Show

Type "arch-sense --demo"
Sleep 400ms
Enter

# Let the gauges drift for a moment.
Sleep 3s

# Thermal Profile: step through the choices.
Right
Sleep 900ms
Right
Sleep 900ms

# Fans: Auto -> Max.
Down
Sleep 500ms
Right
Sleep 1s

# Jump to Lighting and change the effect, then the colour.
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 800ms
Right
Sleep 1.2s
Right
Sleep 1.2s
Down
Sleep 500ms
Right
Sleep 800ms
Right
Sleep 1.5s

# The help overlay and the Doctor.
Type "?"
Sleep 2.5s
Escape
Sleep 500ms
Type "d"
Sleep 3s
Escape
Sleep 1s

Type "q"
Sleep 500ms
