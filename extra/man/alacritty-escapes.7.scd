ALACRITTY-ESCAPES(7)

# Alacritty Escape Sequence Support

This list includes all escape sequences Alacritty currently supports.

# Legend

The available statuses are *PARTIAL*, *IMPLEMENTED* and *REJECTED*. While a
status of *PARTIAL* means there is still work left to be done, a status of
*IMPLEMENTED* for something partially implemented means all other features were
rejected.

All whitespace in escape sequences is solely for formatting purposes and all
relevant spaces are denoted as *SP*. The escape parameters are omitted for
brevity.

# ESC codes - ESC

[[ *ESCAPE*
:[ *STATUS*
:[ *NOTE*
|  _ESC (_
:  Implemented
:[ Only charsets *B* and *0* are supported
|  _ESC )_
:  Implemented
:[ Only charsets *B* and *0* are supported
|  _ESC \*_
:  Implemented
:[ Only charsets *B* and *0* are supported
|  _ESC +_
:  Implemented
:[ Only charsets *B* and *0* are supported
|  _ESC =_
:  Implemented
:[
|  _ESC >_
:  Implemented
:[
|  _ESC 7_
:  Implemented
:[
|  _ESC 8_
:  Implemented
:[
|  _ESC # 8_
:  Implemented
:[
|  _ESC D_
:  Implemented
:[
|  _ESC E_
:  Implemented
:[
|  _ESC H_
:  Implemented
:[
|  _ESC M_
:  Implemented
:[
|  _ESC Z_
:  Implemented
:[

# CSI (Control Sequence Introducer) - ESC [

[[ *ESCAPE*
:[ *STATUS*
:[ *NOTE*
|  _CSI \`_
:  Implemented
:[
|  _CSI @_
:  Implemented
:[
|  _CSI A_
:  Implemented
:[
|  _CSI a_
:  Implemented
:[
|  _CSI B_
:  Implemented
:[
|  _CSI b_
:  Implemented
:[
|  _CSI C_
:  Implemented
:[
|  _CSI c_
:  Implemented
:[
|  _CSI D_
:  Implemented
:[
|  _CSI d_
:  Implemented
:[
|  _CSI E_
:  Implemented
:[
|  _CSI e_
:  Implemented
:[
|  _CSI F_
:  Implemented
:[
|  _CSI f_
:  Implemented
:[
|  _CSI G_
:  Implemented
:[
|  _CSI g_
:  Implemented
:[
|  _CSI H_
:  Implemented
:[
|  _CSI h_
:  Partial
:[ Modes: *4*, *20*
|  _CSI ? h_
:  Partial
:[ Modes: *1*, *3*, *6*, *7*, *12*, *25*, *1000*, *1002*, *1004*, *1005*, *1006*, *1007*, *1042*, *1049*, *2004*, *2026*
|  _CSI I_
:  Implemented
:[
|  _CSI J_
:  Implemented
:[
|  _CSI K_
:  Implemented
:[
|  _CSI L_
:  Implemented
:[
|  _CSI l_
:  Partial
:[ See _CSI h_ for supported modes
|  _CSI ? l_
:  Partial
:[ See _CSI ? h_ for supported modes
|  _CSI M_
:  Implemented
:[
|  _CSI m_
:  Implemented
:[ Parameters: *0*-*9*, *21*-*25*, *27*-*49*, *58*, *59*, *90*-*97*, *100*-*107*
|  _CSI m_
:  Rejected
:[ Parameters: *11*-*19*, *51*-*55*
|  _CSI n_
:  Implemented
:[
|  _CSI P_
:  Implemented
:[
|  _CSI $ p_
:  Implemented
:[
|  _CSI ? $ p_
:  Implemented
:[
|  _CSI SP q_
:  Implemented
:[
|  _CSI r_
:  Implemented
:[
|  _CSI S_
:  Implemented
:[
|  _CSI s_
:  Implemented
:[
|  _CSI T_
:  Implemented
:[
|  _CSI t_
:  Partial
:[ Parameters: *22*, *23*
|  _CSI t_
:  Rejected
:[ Parameters: *1*-*13*, *15*, *19*-*21*, *24*
|  _CSI u_
:  Implemented
:[
|  _CSI ? u_
:  Implemented
:[
|  _CSI = u_
:  Implemented
:[
|  _CSI < u_
:  Implemented
:[
|  _CSI > u_
:  Implemented
:[
|  _CSI X_
:  Implemented
:[
|  _CSI Z_
:  Implemented
:[

# OSC (Operating System Command) - ESC ]

[[ *ESCAPE*
:[ *STATUS*
:[ *NOTE*
|  _OSC 0_
:  Implemented
:[ Icon names are not supported
|  _OSC 1_
:  Rejected
:[ Icon names are not supported
|  _OSC 2_
:  Implemented
:[
|  _OSC 4_
:  Implemented
:[
|  _OSC 7_
:  Implemented
:[ Report working directory as a file URI; see *WORKING DIRECTORY*
|  _OSC 8_
:  Implemented
:[
|  _OSC 10_
:  Implemented
:[
|  _OSC 11_
:  Implemented
:[
|  _OSC 12_
:  Implemented
:[
|  _OSC 50_
:  Implemented
:[ Only *CursorShape* is supported
|  _OSC 52_
:  Implemented
:[ Only *clipboard* and *primary selection* supported
|  _OSC 104_
:  Implemented
:[
|  _OSC 110_
:  Implemented
:[
|  _OSC 111_
:  Implemented
:[
|  _OSC 112_
:  Implemented
:[
|  _OSC 133_
:  Implemented
:[ Prompt and command markers; see *SHELL INTEGRATION*

# DCS (Device Control String) - ESC P

[[ *ESCAPE*
:[ *STATUS*
:[ *NOTE*
|  _DCS = s_
:  Rejected
:[ _CSI ? 2026 h/l_ is used instead

# SEE ALSO

*alacritty*(1), *alacritty-msg*(1), *alacritty*(5), *alacritty-bindings*(5)

# BUGS

Found a bug? Please report it at _https://github.com/alacritty/alacritty/issues_.

# WORKING DIRECTORY

_OSC 7 ; file://hostname/absolute/path ST_ reports the shell's working directory.
BEL may be used instead of ST. Percent-encode reserved characters in the path
(for example, _%20_ for a space and _%3B_ for a semicolon). On Windows, use a drive
path such as _file:///C:/Users/name_.

New windows, new instances, and commands launched by Alacritty prefer the latest
reported directory when it is a usable local directory. An empty hostname,
_localhost_, or the local machine's hostname is accepted, ignoring ASCII case.
Other hosts, hostname aliases, UNC paths, queries, and fragments are not used as
local directories. No hostname lookup or remote connection is performed.

Without a usable report, Alacritty uses its existing directory selection:
foreground-process inspection on Unix and launch defaults on Windows. An empty
OSC 7 report or a terminal reset clears the reported directory. Reports belong
to the window and survive screen switches, scrolling, and resizing.

The shell should report its directory at each prompt, including after a remote
session exits. Recent fish releases and Oh My Zsh do this by default; fish 3,
bash, and plain zsh need a prompt hook. Alacritty cannot detect when a report
becomes stale or distinguish filesystems which share a hostname. Explicit
working-directory options supplied when starting Alacritty or creating a window
through IPC retain their behavior.

# SHELL INTEGRATION

OSC 133 markers tell Alacritty where the prompt, the typed command, and the
command output start. BEL may be used instead of ST. Option fields follow the
marker letter, separated by semicolons; unknown fields are ignored.

[[ *SEQUENCE*
:[ *MEANING*
|  _OSC 133 ; A ST_
:  Prompt starts at the cursor
|  _OSC 133 ; A ; k=s ST_
:  Secondary or continuation prompt starts at the cursor
|  _OSC 133 ; A ; redraw=0 ST_
:  Prompt starts; the shell does not repaint it after a resize
|  _OSC 133 ; A ; redraw=last ST_
:  Prompt starts; the shell repaints only its last line after a resize
|  _OSC 133 ; A ; click_events=1 ST_
:  Prompt starts; the shell accepts mouse click reports (recorded only)
|  _OSC 133 ; B ST_
:  Prompt ends, user input starts
|  _OSC 133 ; C ST_
:  Command runs, output starts
|  _OSC 133 ; D ; status ST_
:  Command finished; the exit status is optional and currently unused
|  _OSC 133 ; N ST_
:  Same as _A_; nested commands are not tracked
|  _OSC 133 ; P ; k=i ST_
:  Same as _A_ without the _redraw_ and _click_events_ options
|  _OSC 133 ; P ; k=s ST_
:  Secondary prompt starts, keeping the previous _redraw_ and _click_events_ settings
|  _OSC 133 ; I ST_
:  Same as _B_; the input does not end at the end of the line
|  _OSC 133 ; L ST_
:  Accepted and ignored

The row under the cursor is marked as a prompt row on _A_. Rows the cursor
moves onto by a line feed or a line wrap while the shell draws its prompt or
reads input are marked as prompt continuation rows, so multi-line prompts and
wrapped input are tracked without _k=s_. An explicit secondary prompt marker
(_A;k=s_ or _P;k=s_) marks the row under the cursor as a secondary prompt row.
The row under the cursor is marked as the start of the output on _C_ when the
cursor is in the first column.

Marks follow their rows into the scrollback, through resizing, and are dropped
with them. Clearing the screen keeps the marks of rows moved into the
scrollback. A terminal reset drops all marks and the recorded shell state.
Markers received on the alternate screen are ignored.

Prompt navigation and output selection work per row. Output which ends on the
row of the next prompt, because the command printed no trailing newline,
belongs to that prompt row. Output which starts on the input row, because _C_
arrived away from the first column, starts on the next row.

Put _A_ and _B_ into the prompt string itself, so a prompt repaint sends them
again. Send _D_ before the prompt from the precmd hook and _C_ from the preexec
hook. See _docs/features.md_ for shell snippets.

# MAINTAINERS

- Christian Duerr <contact@christianduerr.com>
- Kirill Chibisov <contact@kchibisov.com>
