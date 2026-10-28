# Records assets/demo.gif from the fictional demo stack (crates/stack-stat-tui/examples/demo.rs).
# Run from the repository root: make demo-gif
Output "assets/demo.gif"

Require cargo

Set Shell "bash"
Set FontSize 13
Set Width 1100
Set Height 1300
Set Framerate 12
Set Margin 20
Set MarginFill "#010409"
Set BorderRadius 10
Set WindowBar Colorful
Set TypingSpeed 0
Set WaitTimeout 120s
Set Theme { "name": "stack-stat", "background": "#0d1117", "foreground": "#c9d1d9", "selection": "#264f78", "cursor": "#c9d1d9", "black": "#484f58", "red": "#ff7b72", "green": "#3fb950", "yellow": "#d29922", "blue": "#58a6ff", "magenta": "#bc8cff", "cyan": "#39c5cf", "white": "#b1bac4", "brightBlack": "#6e7681", "brightRed": "#ffa198", "brightGreen": "#56d364", "brightYellow": "#e3b341", "brightBlue": "#79c0ff", "brightMagenta": "#d2a8ff", "brightCyan": "#56d4dd", "brightWhite": "#f0f6fc" }

# The harness shell may export NO_COLOR, which would render the dashboard monochrome.
# Recording starts after the startup intro: wait for the dashboard, then for its reveal to settle.
Hide
Type "unset NO_COLOR; export COLORTERM=truecolor; cargo build -q -p stack-stat-tui --example demo && clear && target/debug/examples/demo"
Enter
Wait+Screen /Stack/
Sleep 2s
Show

# The worktree layer's running checks finish a few seconds in and flash.
Sleep 6s

# Walk down through the waiting and blocked layers.
Down@1.3s 7
Sleep 1.3s

# Up to the merged layers, then back to the queued and green layers.
Up@350ms 11
Sleep 1.3s
Down@600ms 2
Sleep 1s

# Reveal and hide the bot review on the next layer to merge.
Type "b"
Sleep 2s
Type "b"
Sleep 0.5s

# Keybinding help.
Type "?"
Sleep 2.5s
Escape
Sleep 0.5s
