# Arch-Sense demo, rendered with https://github.com/charmbracelet/vhs
#
#   cargo build --release
#   vhs demo/arch-sense.tape        # run from the repo root; writes assets/demo.mp4
#   readme-assets gif assets/demo.mp4 assets/demo.gif
#
# Everything on screen comes from `arch-sense --demo`: a simulated Predator
# PH16-71 in a throwaway tree. Nothing on the recording machine is touched.

Output assets/demo.mp4

Set Shell "bash"
Set FontFamily "JetBrainsMono Nerd Font Mono"
Set FontSize 32
Set LineHeight 1.2
Set Width 2560
Set Height 1600
Set Padding 36
Set Margin 56
Set MarginFill "#070b14"
Set BorderRadius 16
Set WindowBar Colorful
Set WindowBarSize 48
Set Framerate 30
Set TypingSpeed 90ms
Set CursorBlink false
Set Theme { "name": "Arch-Sense", "background": "#0f172a", "foreground": "#e2e8f0", "cursor": "#22d3ee", "selection": "#1e293b", "black": "#0f172a", "brightBlack": "#64748b", "red": "#f87171", "brightRed": "#f87171", "green": "#4ade80", "brightGreen": "#4ade80", "yellow": "#fbbf24", "brightYellow": "#fbbf24", "blue": "#60a5fa", "brightBlue": "#60a5fa", "magenta": "#a78bfa", "brightMagenta": "#a78bfa", "cyan": "#22d3ee", "brightCyan": "#22d3ee", "white": "#e2e8f0", "brightWhite": "#ffffff" }
Env COLORTERM "truecolor"

# Launch off camera so the first frame is the populated dashboard, not a prompt.
Hide
Type "export HOME=/home/demo XDG_CONFIG_HOME=/home/demo/.config XDG_STATE_HOME=/home/demo/.local/state PATH=$PWD/target/release:$PATH && clear && arch-sense --demo"
Enter
Sleep 1.5s
Show

# Beat 1: the live dashboard, gauges drifting.
Sleep 2.5s

# Beat 2: Thermal Profile, applied the moment you press a key.
Right
Sleep 2s

# Beat 3: Fans to Max.
Down
Sleep 500ms
Right
Sleep 1.5s

# Beat 4: keyboard lighting, effect then colour, with the live preview.
Tab
Sleep 300ms
Tab
Sleep 300ms
Tab
Sleep 1s
Right
Sleep 1.5s
Right
Sleep 1.5s
Down
Sleep 500ms
Right
Sleep 1.2s
Right
Sleep 1.5s

# Beat 5: the Doctor, one keypress away.
Type "d"
Sleep 3.5s
Escape
Sleep 500ms

# Close where it opened so the loop is seamless.
Tab
Sleep 2s
