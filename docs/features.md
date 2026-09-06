# Features

This document gives an overview over Alacritty's features beyond its terminal
emulation capabilities. To get a list with supported control sequences take a
look at the [alacritty-escapes(7) manpage].

[alacritty-escapes(7) manpage]: ../extra/man/alacritty-escapes.7.scd

## Vi Mode

The vi mode allows moving around Alacritty's viewport and scrollback using the
keyboard. It also serves as a jump-off point for other features like search and
opening URLs with the keyboard. By default you can launch it using
<kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>Space</kbd>.

### Motion

The cursor motions are setup by default to mimic vi, however they are fully
configurable. If you don't like vi's bindings, take a look at the configuration
file to change the various movements.

### Selection

One useful feature of vi mode is the ability to make selections and copy text to
the clipboard. By default you can start a selection using <kbd>v</kbd> and copy
it using <kbd>y</kbd>. All selection modes that are available with the mouse can
be accessed from vi mode, including the semantic (<kbd>Alt</kbd> <kbd>v</kbd>),
line (<kbd>Shift</kbd> <kbd>v</kbd>) and block selection (<kbd>Ctrl</kbd>
<kbd>v</kbd>). You can also toggle between them while the selection is still
active.

## Search

Search allows you to find anything in Alacritty's scrollback buffer. You can
search forward using <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>f</kbd> (<kbd>Command</kbd> <kbd>f</kbd> on macOS) and
backward using <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>b</kbd> (<kbd>Command</kbd> <kbd>b</kbd> on macOS).

### Vi Search

In vi mode the search is bound to <kbd>/</kbd> for forward and <kbd>?</kbd> for
backward search. This allows you to move around quickly and help with selecting
content. The `SearchStart` and `SearchEnd` keybinding actions can be bound if
you're looking for a way to jump to the start or the end of a match.

### Normal Search

During normal search you don't have the opportunity to move around freely, but
you can still jump between matches using <kbd>Enter</kbd> and <kbd>Shift</kbd>
<kbd>Enter</kbd>. After leaving search with <kbd>Escape</kbd> your active match
stays selected, allowing you to easily copy it.

## Hints

Terminal hints allow easily interacting with visible text without having to
start vi mode. They consist of a regex that detects these text elements and then
either feeds them to an external application or triggers one of Alacritty's
built-in actions.

Hints can also be triggered using the mouse or vi mode cursor. If a hint is
enabled for mouse interaction and recognized as such, it will be underlined when
the mouse or vi mode cursor is on top of it. Using the left mouse button or
<kbd>Enter</kbd> key in vi mode will then trigger the hint.

Hints can be configured in the `hints` and `colors.hints` sections in the
Alacritty configuration file.

## Selection expansion

After making a selection, you can use the right mouse button to expand it.
Double-clicking will expand the selection semantically, while triple-clicking
will perform line selection. If you hold <kbd>Ctrl</kbd> while expanding the
selection, it will switch to the block selection mode. With
[shell integration](#shell-integration), holding <kbd>Ctrl</kbd> while
triple-clicking selects the output of the command under the mouse cursor.

## Opening URLs with the mouse

You can open URLs with your mouse by clicking on them. The modifiers required to
be held and program which should open the URL can be setup in the configuration
file. If an application captures your mouse clicks, which is indicated by a
change in mouse cursor shape, you're required to hold <kbd>Shift</kbd> to bypass
that.

## Multi-Window

Alacritty supports running multiple terminal emulators from the same Alacritty
instance. New windows can be created either by using the `CreateNewWindow`
keybinding action, or by executing the `alacritty msg create-window` subcommand.

### Working directory

Shells can report their working directory using OSC 7. The `CreateNewWindow` and
`SpawnNewInstance` actions, as well as external commands launched by Alacritty,
prefer a usable local directory from the latest report. On Unix, Alacritty falls
back to inspecting the foreground process. On Windows it uses the usual launch
defaults when there is no usable report.

For example, this reports `/tmp/project files` on the local machine:

```sh
printf '\033]7;file:///tmp/project%%20files\033\\'
```

Configure your shell to emit a percent-encoded `file://hostname/absolute/path`
URI at each prompt to keep this information up to date. Recent fish releases and
Oh My Zsh do this by default. fish 3 only reports to terminals it recognizes,
which excludes Alacritty, and bash or plain zsh need a prompt hook. Remote hosts
are not interpreted as local paths. When the shell also sends the OSC 133
markers described under [shell integration](#shell-integration), the report is
preferred only while the shell is at its prompt; during a command the
foreground process is inspected first. See [alacritty-escapes(7) manpage] for
host matching, Windows paths, and reset behavior.

## Shell integration

Shells can mark their prompt, the typed command, and the command output with
OSC 133 escape sequences. Alacritty records these marks per row, keeps them in
the scrollback, and uses them for prompt navigation and command output
selection:

- The `ScrollToPreviousPrompt` and `ScrollToNextPrompt` actions put the previous
  or next prompt at the top of the viewport.
- The `PromptUp` and `PromptDown` vi motions move the vi cursor to the previous
  or next prompt.
- The `ToggleOutputSelection` vi action selects the output of the command under
  the vi cursor. <kbd>Ctrl</kbd> + triple click selects the output of the
  command under the mouse cursor.
- When the window width changes while the shell is at its prompt, the prompt is
  cleared before the text is reflowed, so that the shell's repaint does not
  leave fragments of a right prompt or a multi-line prompt behind. At a
  secondary prompt marked with `k=s`, previously accepted command lines are
  preserved.

None of these have default bindings. Example:

```toml
[[keyboard.bindings]]
key = "Up"
mods = "Control|Shift"
mode = "~Alt"
action = "ScrollToPreviousPrompt"

[[keyboard.bindings]]
key = "Down"
mods = "Control|Shift"
mode = "~Alt"
action = "ScrollToNextPrompt"

[[keyboard.bindings]]
key = "["
mode = "Vi|~Search"
action = "PromptUp"

[[keyboard.bindings]]
key = "]"
mode = "Vi|~Search"
action = "PromptDown"

[[keyboard.bindings]]
key = "o"
mods = "Alt"
mode = "Vi|~Search"
action = "ToggleOutputSelection"
```

Marks are kept per row. Output that ends on the row of the next prompt, because
the command printed no trailing newline, is not part of the output selection.

The shell has to send the markers. Put the prompt start (`A`) and prompt end
(`B`) markers into the prompt string, so that a repaint sends them again. Send
the command end marker (`D`) before each prompt and the command start marker
(`C`) before a command runs. Shells which send only `A` and `B` cannot separate
the output from the prompt.

zsh:

```zsh
autoload -Uz add-zsh-hook
_osc133_precmd() { print -n "\e]133;D;$?\a" }
_osc133_preexec() { print -n "\e]133;C\a" }
add-zsh-hook precmd _osc133_precmd
add-zsh-hook preexec _osc133_preexec
PS1=$'%{\e]133;A\a%}'"$PS1"$'%{\e]133;B\a%}'
PS2=$'%{\e]133;P;k=s\a%}'"$PS2"$'%{\e]133;B\a%}'
```

Mark `PS2` as a secondary prompt with `P;k=s` (or `A;k=s`) so resizing clears
only the prompt the shell will repaint. `P` keeps the redraw settings of the
preceding `A`.

If a hook sets `PS1` or `PS2` on every prompt, add the markers inside that hook.

bash repaints only the last prompt line after a resize, hence `redraw=last`:

```bash
PS1='\[\e]133;A;redraw=last\a\]'"$PS1"'\[\e]133;B\a\]'
PS2='\[\e]133;P;k=s\a\]'"$PS2"'\[\e]133;B\a\]'
PS0='\e]133;C\a'
_osc133_precmd() { printf '\e]133;D;%s\a' "$?"; }
PROMPT_COMMAND="_osc133_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
```

fish 4 sends the markers by default, but does not repaint after a resize unless
`fish_handle_reflow` is set to 1. The integration scripts shipped with kitty and
Ghostty also work. tmux handles the markers itself and does not forward them to
Alacritty. See [alacritty-escapes(7) manpage] for the accepted markers and
options.
