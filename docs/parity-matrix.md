# Parity matrix — PlexusTerm v0.2.15 → PlexusTerm Native

One row per v0.2.15 test (`ts` = Vitest under `src/`, `rs` = Rust under `src-tauri/src/`), mapped to the PRD requirement it covers (`docs/PRD.md` §7, §9, §10, the §8 IDs, ST-*, INTEROP-1; `n/a` for a dropped test with no native counterpart) and its planned disposition (§11.1): `ported`, `ui`, `manual`, `dropped`. Req tokens follow PRD v4 (2026-10-21).

## Summary

| Area | Rows | ported | ui | manual | dropped |
| --- | --- | --- | --- | --- | --- |
| Terminal | 292 | 194 | 90 | 1 | 7 |
| Sessions and protocols | 447 | 380 | 55 | 0 | 12 |
| Files and viewer | 308 | 155 | 150 | 0 | 3 |
| AI, DEV mode, macros and tools | 419 | 262 | 148 | 4 | 5 |
| App shell, settings and terminal sharing | 348 | 188 | 156 | 0 | 4 |
| **Total** | **1814** | **1179** | **599** | **5** | **31** |

## 1. Terminal

| Kind | File | Test | Req | Disposition | Note |
| --- | --- | --- | --- | --- | --- |
| ts | src/features/drop/dirDrop.test.ts | keeps what would really move into the folder | FIX-3, TREE-1 | ported |  |
| ts | src/features/drop/dirDrop.test.ts | leaves out an entry already in the folder, the folder itself and a folder dropped into its own subfolder | FIX-3, TREE-1 | ported |  |
| ts | src/features/drop/dirDrop.test.ts | finds a folder row under the pointer and hands it the drop instead of the app | FIX-3, TREE-1 | ui |  |
| ts | src/features/drop/dropAction.test.ts | changes to a folder dropped on an empty line | DROP-1 | ported |  |
| ts | src/features/drop/dropAction.test.ts | types the path when something is typed, for files, and for other machines | DROP-1 | ported |  |
| ts | src/features/drop/dropAction.test.ts | types several paths separated by spaces, quoted for the shell | DROP-1 | ported |  |
| ts | src/features/drop/dropAction.test.ts | only types a POSIX path with a backslash: fish reads \\' as an escape even inside quotes | DROP-1 | ported |  |
| ts | src/features/drop/dropAction.test.ts | refuses paths with control characters | DROP-1 | ported |  |
| ts | src/features/drop/dropAction.test.ts | pairs local paths with local terminals and SFTP paths with their own SSH session | DROP-1 | ported |  |
| ts | src/features/drop/dropAction.test.ts | changes to a dropped Windows folder | DROP-1 | ported |  |
| ts | src/features/history/history.test.ts | names each kind of session | TERM-12b | ported |  |
| ts | src/features/history/history.test.ts | records a command once per host, counting repeats and keeping the latest time | TERM-12b | ported |  |
| ts | src/features/history/history.test.ts | ignores empty lines and trims trailing whitespace | TERM-12b | ported |  |
| ts | src/features/history/history.test.ts | keeps at most MAX_PER_HOST commands per host, dropping the oldest | TERM-12b | ported |  |
| ts | src/features/history/history.test.ts | keeps at most MAX_HOSTS hosts, dropping the one used longest ago | TERM-12b | ported |  |
| ts | src/features/history/history.test.ts | completes a prefix with the most recent matching command of the host | TERM-12a | ported |  |
| ts | src/features/history/history.test.ts | prefers a command used much more often over a slightly more recent one | TERM-12a | ported |  |
| ts | src/features/history/history.test.ts | uses the host's own commands first, then every host's | TERM-12a | ported |  |
| ts | src/features/history/history.test.ts | suggests nothing for an empty prefix, an exact match or no match | TERM-12a | ported |  |
| ts | src/features/history/history.test.ts | lists the host's last commands, oldest first | TERM-12c, AI-7 | ported |  |
| ts | src/features/history/history.test.ts | drops entries holding control characters, which would be typed into the terminal | TERM-12d | ported |  |
| ts | src/features/history/history.test.ts | tells an unreadable file apart from an empty one | TERM-12d | ported |  |
| ts | src/features/history/history.test.ts | round-trips with a version | DATA-1a | ported |  |
| ts | src/features/history/history.test.ts | gives an empty history for missing or invalid files and drops invalid entries | DATA-1a, DATA-2 | ported |  |
| ts | src/features/history/historyStore.test.ts | loads history.json once and suggests from it | TERM-12b, TERM-12d | ported |  |
| ts | src/features/history/historyStore.test.ts | saves recorded commands once after a pause | TERM-12d | ported |  |
| ts | src/features/history/historyStore.test.ts | keeps commands recorded before the file finished loading | TERM-12d | ported |  |
| ts | src/features/history/historyStore.test.ts | never overwrites a history file it could not read | TERM-12d | ported |  |
| ts | src/features/history/historyStore.test.ts | never overwrites a history file that is not valid JSON, until it is cleared | TERM-12d | ported |  |
| ts | src/features/history/historyStore.test.ts | counts a command recorded while an invalid file loads only once | TERM-12d | ported |  |
| ts | src/features/history/historyStore.test.ts | clears the history at once, also on disk | TERM-12b | ported |  |
| ts | src/features/notifications/App.attention.test.tsx | marks the tab and the pane that rang while another pane has the focus, until that pane is focused | TERM-9 | ui |  |
| ts | src/features/notifications/App.attention.test.tsx | never marks the pane being looked at: a bell while typing there raises nothing | TERM-9 | ui |  |
| ts | src/features/notifications/App.attention.test.tsx | shows a desktop notification with the message while the window is in the background, and clears on return | TERM-9 | ui |  |
| ts | src/features/notifications/App.attention.test.tsx | names the tab when a bell rings in the background, and lets a flood through once | TERM-9 | ui |  |
| ts | src/features/notifications/App.attention.test.tsx | keeps the tab mark but sends no desktop notification when they are turned off | TERM-9 | ui |  |
| ts | src/features/notifications/notifier.test.ts | sends the first notification of a source | TERM-9 | ported |  |
| ts | src/features/notifications/notifier.test.ts | lets one notification per source through every few seconds: a flood of bells is one notification | TERM-9 | ported |  |
| ts | src/features/notifications/notifier.test.ts | caps notifications from every source together | TERM-9 | ported |  |
| ts | src/features/notifications/systemNotification.test.ts | asks for permission once, then shows the notification through the Tauri plugin | TERM-9 | ported | Tauri plugin replaced by native notifier (O5) |
| ts | src/features/notifications/systemNotification.test.ts | shows nothing when the permission is refused, and never throws | TERM-9 | ported | Tauri plugin replaced by native notifier (O5) |
| ts | src/features/terminal/App.gitBranch.test.tsx | shows the branch of the terminal's folder in a bar under it in DEV mode, not in the tab | DEV-3 | ui |  |
| ts | src/features/terminal/App.gitBranch.test.tsx | shows nothing and reads nothing outside DEV mode (L5) | DEV-3 | ui |  |
| ts | src/features/terminal/App.localShell.test.tsx | opens the first terminal and every new one with the shell from Settings | TERM-16, SES-1a | ui |  |
| ts | src/features/terminal/App.localShell.test.tsx | starts the first terminal and every new one in the folder from Settings (S8) | TERM-16, SES-1a | ui |  |
| ts | src/features/terminal/App.localShell.test.tsx | opens plain local terminals with the system default shell | TERM-16, SES-1a | ui |  |
| ts | src/features/terminal/App.localShell.test.tsx | open a tab with their own shell, arguments and starting directory, titled with the shell | TERM-16, SES-1a | ui |  |
| ts | src/features/terminal/FindBar.test.tsx | focuses the search field when it opens | TERM-4 | ui |  |
| ts | src/features/terminal/FindBar.test.tsx | finds the next match on Enter and the previous one on Shift+Enter or the buttons | TERM-4 | ui |  |
| ts | src/features/terminal/FindBar.test.tsx | passes match case and regex | TERM-4 | ui |  |
| ts | src/features/terminal/FindBar.test.tsx | says when nothing matches | TERM-4 | ui |  |
| ts | src/features/terminal/FindBar.test.tsx | does not search for an empty text | TERM-4 | ui |  |
| ts | src/features/terminal/FindBar.test.tsx | closes on Escape and with the close button | TERM-4 | ui |  |
| ts | src/features/terminal/PasteConfirm.test.tsx | shows the line count and the first lines | TERM-7 | ui |  |
| ts | src/features/terminal/PasteConfirm.test.tsx | pastes, reporting whether to ask again | TERM-7 | ui |  |
| ts | src/features/terminal/PasteConfirm.test.tsx | cancels with the button or Escape | TERM-7 | ui |  |
| ts | src/features/terminal/TerminalContextMenu.test.tsx | lists the terminal actions at the pointer position | TERM-6 | ui |  |
| ts | src/features/terminal/TerminalContextMenu.test.tsx | runs the chosen action and closes | TERM-6 | ui |  |
| ts | src/features/terminal/TerminalContextMenu.test.tsx | offers to stop logging while the pane is logged | TERM-6 | ui |  |
| ts | src/features/terminal/TerminalContextMenu.test.tsx | closes on Escape and on a click outside | TERM-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | opens the backend session once under StrictMode (serial ports allow a single opener) | SER-1, TERM-19a | ui | invariant: one open per pane; StrictMode itself has no native meaning |
| ts | src/features/terminal/TerminalView.test.tsx | never opens a session when unmounted before the deferred start | TERM-19a | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | starts the shell once the fonts have loaded, at its final size (no resize while it starts) | TERM-19a | ui | start after first layout, no resize while starting |
| ts | src/features/terminal/TerminalView.test.tsx | starts the shell anyway when the fonts take too long | TERM-19a | dropped | web font loading timeout; native fonts resolve synchronously |
| ts | src/features/terminal/TerminalView.test.tsx | passes title changes on at most every TITLE_MS, ending with the latest title | TERM-1a | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | draws a thin bar cursor, not a block | TERM-2 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | colours error lines again when the setting or the scheme changes, and stops on close | TERM-5 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | colours error lines again once, after a window resize settles | TERM-5 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | enables the proposed API that colours the error lines | TERM-5 | dropped | xterm.js allowProposedApi flag |
| ts | src/features/terminal/TerminalView.test.tsx | starts with the font size of the settings and follows its changes | TERM-18 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | starts with the line height of the settings and follows its changes | TERM-18 | ui | default now 1.0, see Wrong-3 |
| ts | src/features/terminal/TerminalView.test.tsx | starts with the font weight of the settings, bold a step heavier, and follows its changes | TERM-18 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | starts with the colour scheme and font of the settings and follows their changes | TERM-18, THEME-1, FONT-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | pastes a single line from the clipboard on right click | TERM-7 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | sends an empty paste when the clipboard holds an image, so an AI CLI attaches it | TERM-7a | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | sends an empty paste instead of an error when the clipboard has no text and the program asked for bracketed paste | TERM-7a | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | still reports an unreadable clipboard at a shell prompt | TERM-7a | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | asks before pasting several lines and can stop asking | TERM-7 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | opens the context menu on right click when right click does not paste, or on Ctrl+right click | TERM-6, TERM-7 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | opens the menu on right click over selected text even when right click pastes (S7) | TERM-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | right click on a word still pastes when xterm selects the word on right click (macOS) | TERM-7 | dropped | xterm.js macOS right-click word-selection quirk |
| ts | src/features/terminal/TerminalView.test.tsx | has no Ask the AI item without a chat to open | TERM-6, AI-4 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | keeps Ctrl+Shift+F from the shell and opens the find bar; Escape closes it | TERM-4, KEY-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | keeps the assistant shortcut from the shell, following its setting (S3) | AI-3, KEY-5 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | lets the user's app shortcuts through and keeps the freed default for the shell | KEY-5 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | draws with WebGL when it is on, after the terminal is open | TERM-3, SET-2 | ui | WebGL → GPU renderer |
| ts | src/features/terminal/TerminalView.test.tsx | keeps xterm's own renderer when it is off, and switches live both ways | TERM-3, SET-2 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | falls back to xterm's own renderer when the WebGL context is lost | TERM-3 | manual | context loss is hard to inject; or ui with a fake device |
| ts | src/features/terminal/TerminalView.test.tsx | after the renderer changes (WebGL measures cells its own way) | TERM-3 | ui | refit after renderer switch |
| ts | src/features/terminal/TerminalView.test.tsx | once the fonts have loaded | TERM-18 | ui | refit after font change |
| ts | src/features/terminal/TerminalView.test.tsx | keeps xterm's own renderer when WebGL is drawn in software (llvmpipe) | TERM-3 | dropped | WebGL-vs-canvas choice; see webglProbe |
| ts | src/features/terminal/TerminalView.test.tsx | falls back without an error when WebGL cannot start | TERM-3 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | reports a bell and the OSC 9, 777 and 99 notifications, with their text | TERM-9 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | ignores progress reports (OSC 9;4), which Claude Code sends while it works | TERM-9 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | reads the folder Windows shells report through OSC 9;9, and asks for no attention | TERM-15, TERM-9 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | still keeps the folder reported through OSC 7 | TERM-15 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | signals the prompt when the shell reports its folder (OSC 7) or marks a prompt (OSC 133) | TERM-11 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | without shell integration, signals once the output of a command sent with Enter goes quiet | TERM-11, DEV-1 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | tells whether the prompt came from a shell mark or from quiet output | TERM-11 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | reports each command sent with Enter with the prompt it was typed after | TERM-11, DEV-1 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | reads the command off the screen when it came from history or completion | DEV-1, TERM-11 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | reports no command typed at a password prompt | TERM-11 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | reports the text before the cursor whenever output goes quiet, only when asked | TERM-11 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | reports when its output starts and when it stops for a while, not the echo of what is typed | DEV-2, VIEW-2 | ported | 1.5 s quiet |
| ts | src/features/terminal/TerminalView.test.tsx | sends a line feed (Ctrl+J), the new-line key of Claude Code, Codex, Gemini CLI and OpenCode | TERM-8 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | leaves Enter, Ctrl+Enter, Alt+Enter and Ctrl+Shift+Enter to xterm unchanged | TERM-8 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | sends Enter's carriage return like before when the setting is off | TERM-8 | ported |  |
| ts | src/features/terminal/TerminalView.test.tsx | leaves a key being composed by an input method alone | TERM-1c | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | runs the pane actions bound by the user: select all and clear | KEY-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | hands the assistant the pane's selection and a way to focus it back (S3) | AI-3, AI-4 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | links the paths it prints to the files its fileLinks find, opened with Ctrl+click | TERM-14 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | logs received output to the chosen file without escape sequences | TERM-10 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | warns that Telnet is not encrypted before connecting | TEL-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | shows connection errors without asking for a password | SSH-5, SSH-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | says it starts the server over SSH | MOSH-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | asks for the SSH password when the saved one is rejected, then retries with it | SSH-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | keeps the pane open when mosh-client fails, and closes it on a clean exit | MOSH-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | asks for the password with the checkbox ticked when it is rejected, then retries with it | SSH-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | forgets the saved password when the checkbox is unticked after it was rejected | SSH-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | connects first and asks only when no password is saved, unticked like a first login | SSH-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | does not prompt for other failures | SSH-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | asks to trust an unknown jump host, naming it, and retries with that hop's fingerprint | SSH-2 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | asks for a jump host password its saved one failed, and retries with it for that hop only | SSH-6, SSH-3 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | draws the rest of a known command after the typed prefix and accepts it with Right Arrow, like typing | TERM-12 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | draws the ghost once per frame, and never accepts the previous one before it is drawn | TERM-12 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | accepts the whole suggestion with End and one word with Ctrl+Right | TERM-12 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | leaves Tab and other keys to the shell and pauses suggestions after Tab | TERM-12a | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | hides suggestions when they are turned off or the cursor is not at the end of the typed text | TERM-12a | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | records commands sent with Enter, except lines starting with a space | TERM-12b | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | records a command inserted from the AI chat once it is sent with Enter, never broadcasting it | AI-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | runs a command from the AI chat: clears what was typed, sends it with Enter and records it | AI-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | clears the line with Ctrl+End and Ctrl+Home before running a command in a local Windows shell | TERM-13 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | refuses to run a command at a password prompt or in a full-screen program | AI-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | tells whether the command line is empty, for drops (S4) | DROP-1, TERM-11 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | reads an uncertain line from the screen: empty after Ctrl+L or arrows, not after a recalled command | TERM-11 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | refuses to run a command with line breaks or control characters | AI-6 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | never records nor suggests at a password prompt | TERM-12b | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | never records nor suggests in full-screen programs | TERM-12b | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | does not record commands other panes broadcast to this one | TERM-12b, MEXEC-1 | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | asks the AI only when the history has nothing, after a pause, and shows its answer | TERM-12c | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | never sends nor records what is typed at a password prompt in another language | TERM-12b, TERM-12c | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | never sends a line typed with a leading space (kept out of history) to the AI | TERM-12c | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | ignores an answer that arrives after the line changed, and never asks when AI suggestions are off | TERM-12c | ui |  |
| ts | src/features/terminal/TerminalView.test.tsx | learns its tap and output numbers without taking them for the end of the session | SHARE-6 | ui | share pane |
| ts | src/features/terminal/TerminalView.test.tsx | serializes the screen and the chosen history, saying which output they cover | SHARE-6 | ui | share pane |
| ts | src/features/terminal/TerminalView.test.tsx | types a viewer's keys into the session without reporting or recording them as the user's | SHARE-6 | ui | share pane |
| ts | src/features/terminal/attention.test.ts | reads the message, as Codex and Claude Code send it | TERM-9 | ported |  |
| ts | src/features/terminal/attention.test.ts | ignores ConEmu / Windows Terminal progress and other numbered OSC 9 commands | TERM-9 | ported |  |
| ts | src/features/terminal/attention.test.ts | still counts an empty message as a request for attention | TERM-9 | ported |  |
| ts | src/features/terminal/attention.test.ts | reads the title and the body, which may contain semicolons | TERM-9 | ported |  |
| ts | src/features/terminal/attention.test.ts | ignores other OSC 777 commands | TERM-9 | ported |  |
| ts | src/features/terminal/attention.test.ts | reads a one-part notification: the payload is the title by default | TERM-9a | ported |  |
| ts | src/features/terminal/attention.test.ts | joins the parts of one notification until d=1, as Claude Code's kitty channel sends them | TERM-9a | ported |  |
| ts | src/features/terminal/attention.test.ts | decodes base64 payloads (e=1), UTF-8 included | TERM-9a | ported |  |
| ts | src/features/terminal/attention.test.ts | ignores queries, icons, buttons and close requests | TERM-9a | ported |  |
| ts | src/features/terminal/attention.test.ts | keeps a bounded number of unfinished notifications and bounded text | TERM-9a | ported |  |
| ts | src/features/terminal/clearLine.test.ts | uses Ctrl+End then Ctrl+Home for local shells on Windows (PowerShell and cmd) | TERM-13 | ported |  |
| ts | src/features/terminal/clearLine.test.ts | uses Ctrl+E then Ctrl+U for local shells on Linux and macOS | TERM-13 | ported |  |
| ts | src/features/terminal/clearLine.test.ts | follows the shell of a local tab on Windows: PowerShell and cmd keys, or Emacs keys for bash and WSL | TERM-13 | ported |  |
| ts | src/features/terminal/clearLine.test.ts | uses Ctrl+E then Ctrl+U for PowerShell on Linux and macOS, where PSReadLine uses Emacs keys | TERM-13 | ported |  |
| ts | src/features/terminal/clearLine.test.ts | uses Ctrl+E then Ctrl+U for remote sessions, whatever the local platform | TERM-13 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | leaves %s alone | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | is the scheme's red, or a default red when the scheme has none in #RRGGBB form | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | reads the new output once for many writes before the next frame | TERM-5, ST-4 | ported | one pass per frame |
| ts | src/features/terminal/errorHighlight.test.ts | colours the finished lines that look like errors, across the whole width | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | leaves the cursor's line until it is finished, and never colours a line twice | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | colours every row of a wrapped error line | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | does nothing while it is off or a full-screen program runs | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | refresh recolours everything with the current colour, or clears it when off | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | never lets a failing decoration get in the way of the output | TERM-5 | ported | as: a highlight failure never touches the grid |
| ts | src/features/terminal/errorHighlight.test.ts | drops the colour of a screen row that is rewritten, and colours the errors printed after a clear | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | keeps the colours of scrollback rows, which can no longer change | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | never colours a row twice, even when it reads the buffer again | TERM-5 | ported |  |
| ts | src/features/terminal/errorHighlight.test.ts | keeps no marker behind when a decoration cannot be drawn | TERM-5 | ported | as: a highlight failure never touches the grid |
| ts | src/features/terminal/errorHighlight.test.ts | dispose removes the colours and stops listening | TERM-5 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | finds relative, absolute and home paths with a folder in them | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | drops a line and column suffix and trailing punctuation | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | finds Windows paths | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | finds bare file names with an extension, but nothing inside URLs, plain words or lone slashes | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | joins relative paths to the folder, expands the home folder and keeps absolute ones | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | cannot resolve a relative path without a folder | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | links only the paths that exist, over the cells they cover | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | opens the file on Ctrl+click or Cmd+click only | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | gives no links for a line without paths, without asking | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | reads the path of a file URI, decoded, and nothing else | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | opens a URL on Ctrl+click or Cmd+click only | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | opens web links in the browser and file links as files, on Ctrl+click only | TERM-14 | ported |  |
| ts | src/features/terminal/fileLinks.test.ts | leaves file links alone in a pane that cannot open files | TERM-14 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | reads local and SSH terminals only, once they are connected | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | reads the branch of a local folder from the backend | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | reads it again when the folder changes and when the prompt comes back, not otherwise | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | asks the shell for its folder when it does not report one (OSC 7) | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | runs git over the SSH session for remote folders, asking the server for the folder when needed | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | shows nothing when turned off, without a source, or when reading fails | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | ignores an answer for a folder the terminal has already left | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | follows a running CLI's folder and branch while watching: a worktree or a switched branch shows at once | DEV-3 | ported |  |
| ts | src/features/terminal/gitBranch.test.ts | watches an SSH terminal less often, through its session | DEV-3 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | appends printable characters, one key or a whole chunk at a time | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | removes the last character on Backspace (DEL or ^H) | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | clears the line on Ctrl+U and Ctrl+C | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | deletes the previous word on Ctrl+W | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | commits the line on Enter and starts a new one | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | becomes uncertain on arrows, Ctrl+R, Tab, Alt keys and other control keys, until Enter | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | treats a bracketed paste like typing | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | makes the line uncertain, and starts fresh after an Enter in the data | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | finds the prompt before the echoed text | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | recognises password, passphrase and PIN prompts but not ordinary shell prompts | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | also recognises verification codes, OTPs and tokens | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | recognises sudo, ssh and PIN prompts in the interface languages, with ASCII or full-width colons | TERM-11, TERM-12b | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | still leaves ordinary prompts and output alone | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | splits a prompt row into the prompt and the command after its last prompt sign | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | is null without a prompt sign or a command | TERM-11 | ported |  |
| ts | src/features/terminal/lineTracker.test.ts | keeps the line certain through focus reports and cursor position reports (ConPTY asks for both) | TERM-11 | ported | terminal replies keep the line certain |
| ts | src/features/terminal/lineTracker.test.ts | keeps the line certain through device attribute and mode replies | TERM-11 | ported | terminal replies keep the line certain |
| ts | src/features/terminal/lineTracker.test.ts | still treats keys as making the line uncertain (arrows, Shift+Tab) | TERM-11 | ported |  |
| ts | src/features/terminal/localBackend.test.ts | starts the user's shell in the home directory when the target names neither | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localBackend.test.ts | starts the target's shell with its arguments in its starting directory | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localBackend.test.ts | leaves out a blank program, no arguments and a blank directory | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localBackend.test.ts | writes, resizes and closes through the PTY commands | TERM-1 | ported |  |
| ts | src/features/terminal/localShell.roundtrip.test.ts | never sends an environment to pty_spawn, from a tab or a saved session | TERM-1b | ported | env never comes from the UI |
| ts | src/features/terminal/localShell.test.ts | splits on spaces, keeping quoted text and Windows paths as one argument | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localShell.test.ts | drops the directory and a .exe extension | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localShell.test.ts | is a plain local target with the system default shell | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localShell.test.ts | carries the chosen shell and its arguments | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localShell.test.ts | starts in the folder chosen in Settings (S8) | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localShell.test.ts | names the local tab's shell, or the default shell when the tab has none | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/localShell.test.ts | is unknown before the shells are known and for remote sessions | TERM-16, SES-1a | ported |  |
| ts | src/features/terminal/output.test.ts | removes CSI sequences (colors, cursor moves, private modes) | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | removes OSC sequences ended by BEL or ST | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | removes charset designations and other bare ESC sequences | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | removes DCS strings and bells, and turns CRLF into LF | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | keeps plain text, tabs and UTF-8 as they are | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | holds back an escape sequence cut at the end of a chunk | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | keeps complete text whole | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | trims each line on the right, joins wrapped lines and drops trailing empty lines | TERM-10 | ported |  |
| ts | src/features/terminal/output.test.ts | returns an empty string for an empty buffer | TERM-10 | ported |  |
| ts | src/features/terminal/paste.test.ts | pastes a single line directly | TERM-7 | ported |  |
| ts | src/features/terminal/paste.test.ts | does not count a final line break as another line | TERM-7 | ported |  |
| ts | src/features/terminal/paste.test.ts | asks before pasting more than one line, with the line count and the first 5 lines | TERM-7 | ported |  |
| ts | src/features/terminal/paste.test.ts | pastes several lines directly when confirmation is turned off | TERM-7 | ported |  |
| ts | src/features/terminal/paste.test.ts | pastes an empty clipboard as an empty paste, so a program can read an image from the clipboard itself | TERM-7a | ported |  |
| ts | src/features/terminal/sessionLog.test.ts | appends the received output in batches, without escape sequences | TERM-10 | ported |  |
| ts | src/features/terminal/sessionLog.test.ts | keeps an escape sequence cut between chunks for the next batch | TERM-10 | ported |  |
| ts | src/features/terminal/sessionLog.test.ts | flushes everything when stopped and ignores later output | TERM-10 | ported |  |
| ts | src/features/terminal/sessionLog.test.ts | reports a failed append once and stops logging | TERM-10a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | follows the local shell and treats remote sessions as POSIX | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | leaves plain paths alone | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | quotes the rest for each shell | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | doubles every quote PowerShell reads as a single quote, typographic ones too | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | quotes commas and at signs for PowerShell, which read them as arrays and splats | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | refuses control characters | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | changes folder in each shell, drive included for cmd | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | is the flavor of a local bash on Windows only | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/shellPath.test.ts | changes to the POSIX form of a Windows path | DROP-1, SFTP-1a | ported |  |
| ts | src/features/terminal/themes.test.ts | ships the requested schemes plus PlexusTerm, each with a unique id | SCHEME-1 | ported |  |
| ts | src/features/terminal/themes.test.ts | leaves out the paid schemes | SCHEME-1 | ported |  |
| ts | src/features/terminal/themes.test.ts | gives every theme a background, a foreground, a cursor and the 16 ANSI colours | SCHEME-1 | ported |  |
| ts | src/features/terminal/themes.test.ts | marks light and dark themes by their background | SCHEME-1 | ported |  |
| ts | src/features/terminal/themes.test.ts | uses Catppuccin Mocha by default and for unknown ids | SCHEME-1 | ported |  |
| ts | src/features/terminal/webglProbe.test.ts | is true for software renderers, which make every frame slow | TERM-3 | dropped | WebGL-vs-canvas choice; natively the software adapter (lavapipe) is the fallback itself |
| ts | src/features/terminal/webglProbe.test.ts | is false for a GPU, and when it cannot tell | TERM-3 | dropped | WebGL-vs-canvas choice; natively the software adapter (lavapipe) is the fallback itself |
| ts | src/features/terminal/webglProbe.test.ts | asks once and remembers the answer | TERM-3 | dropped | WebGL-vs-canvas choice; natively the software adapter (lavapipe) is the fallback itself |
| rs | src-tauri/src/history.rs | missing_history_loads_as_none | DATA-2 | ported |  |
| rs | src-tauri/src/history.rs | save_then_load_round_trips | DATA-2 | ported |  |
| rs | src-tauri/src/history.rs | invalid_or_oversized_contents_are_refused_and_keep_the_file | DATA-2 | ported |  |
| rs | src-tauri/src/history.rs | history_file_is_private_to_the_user | DATA-2 | ported |  |
| rs | src-tauri/src/pty/mod.rs | extra_environment_reaches_the_process | MOSH-1 | ported | MOSH_KEY |
| rs | src-tauri/src/pty/mod.rs | extra_environment_is_never_read_from_the_frontend_nor_shown_in_debug | TERM-1b | ported |  |
| rs | src-tauri/src/pty/mod.rs | a_starting_directory_with_a_tilde_starts_in_the_home_directory | SES-1a | ported |  |
| rs | src-tauri/src/pty/mod.rs | a_blank_program_or_directory_keeps_the_defaults | TERM-16 | ported |  |
| rs | src-tauri/src/pty/mod.rs | a_program_that_does_not_exist_is_an_error | TERM-16 | ported |  |
| rs | src-tauri/src/pty/mod.rs | cwd_follows_the_shell_folder | TERM-15 | ported |  |
| rs | src-tauri/src/pty/mod.rs | program_cwd_follows_the_program_in_the_foreground | TERM-15a | ported |  |
| rs | src-tauri/src/pty/mod.rs | streams_output_and_reports_exit_code | TERM-19 | ported |  |
| rs | src-tauri/src/pty/mod.rs | writes_input_to_the_process | TERM-1 | ported |  |
| rs | src-tauri/src/pty/mod.rs | a_large_paste_never_blocks_while_the_process_does_not_read | TERM-1b | ported |  |
| rs | src-tauri/src/pty/mod.rs | writes_reach_the_process_in_order | TERM-1 | ported |  |
| rs | src-tauri/src/pty/mod.rs | resize_changes_the_terminal_size | TERM-1 | ported |  |
| rs | src-tauri/src/pty/mod.rs | close_kills_the_process_and_forgets_the_session | TERM-19 | ported |  |
| rs | src-tauri/src/pty/mod.rs | unknown_session_is_an_error | TERM-1 | ported |  |
| rs | src-tauri/src/pty/shell_integration.rs | powershell_gets_the_prompt_wrapper_only_without_arguments_of_the_user | TERM-15 | ported |  |
| rs | src-tauri/src/pty/shell_integration.rs | cmd_reports_its_folder_in_front_of_its_own_prompt | TERM-15 | ported |  |
| rs | src-tauri/src/pty/shell_integration.rs | other_shells_are_left_alone | TERM-15 | ported |  |
| rs | src-tauri/src/pty/shell_integration.rs | git_bash_reports_its_folder_after_any_prompt_command_of_its_own | TERM-15 | ported |  |
| rs | src-tauri/src/pty/shell_integration.rs | the_git_bash_report_prints_the_windows_form_of_the_folder | TERM-15 | ported |  |
| rs | src-tauri/src/terminal/coalesce.rs | output_already_waiting_goes_out_as_one_message_in_order | NFR-2a | ported | PTY reader batching |
| rs | src-tauri/src/terminal/coalesce.rs | the_first_output_after_a_quiet_moment_is_sent_at_once | NFR-2a | ported | PTY reader batching |
| rs | src-tauri/src/terminal/coalesce.rs | a_burst_is_gathered_into_few_messages_without_losing_or_reordering_text | NFR-2a | ported | PTY reader batching |
| rs | src-tauri/src/terminal/coalesce.rs | a_batch_never_grows_past_its_limit | NFR-2a | ported | PTY reader batching |
| rs | src-tauri/src/terminal/utf8.rs | passes_ascii_through | TERM-1 | ported | alacritty parser; keep as term-engine test |
| rs | src-tauri/src/terminal/utf8.rs | joins_multibyte_char_split_across_chunks | TERM-1 | ported | alacritty parser; keep as term-engine test |
| rs | src-tauri/src/terminal/utf8.rs | replaces_invalid_bytes_and_continues | TERM-1 | ported | alacritty parser; keep as term-engine test |

## 2. Sessions and protocols

| Kind | File | Test | Req | Disposition | Note |
| --- | --- | --- | --- | --- | --- |
| ts | src/features/mosh/App.telnetMosh.test.tsx | opens a Telnet tab from the More menu | UI-1, TEL-1 | ui |  |
| ts | src/features/mosh/App.telnetMosh.test.tsx | opens a Mosh tab over a saved SSH session from the More menu | UI-1, MOSH-1 | ui |  |
| ts | src/features/mosh/App.telnetMosh.test.tsx | hides Mosh from the More menu on Windows (S18.3) | MOSH-1 | ui |  |
| ts | src/features/mosh/App.telnetMosh.test.tsx | opens saved Telnet and Mosh sessions | SES-1, TEL-1, MOSH-1 | ui |  |
| ts | src/features/mosh/App.telnetMosh.test.tsx | explains when the SSH session of a Mosh session no longer exists | SES-1e | ui |  |
| ts | src/features/mosh/MoshDialog.test.tsx | connects over the chosen saved SSH session with the default server command | MOSH-1 | ui |  |
| ts | src/features/mosh/MoshDialog.test.tsx | passes a custom server command | MOSH-1 | ui |  |
| ts | src/features/mosh/MoshDialog.test.tsx | explains that a saved SSH session is needed when there is none | SES-1e | ui |  |
| ts | src/features/mosh/moshBackend.test.ts | starts mosh over the SSH login of the target, with the server command | MOSH-1 | ported |  |
| ts | src/features/mosh/moshBackend.test.ts | lets a retry replace the login, the jump hosts and the saved-password request | MOSH-1, SSH-6 | ported |  |
| ts | src/features/mosh/moshBackend.test.ts | drives the client through the PTY commands | MOSH-1 | dropped | Tauri command wrapper; native calls the service directly |
| ts | src/features/platform/programsDir.test.ts | is where each system keeps its programs | PLAT-1 | ported | Browse dialogs start in the system programs folder |
| ts | src/features/platform/usePlatform.test.tsx | returns the platform reported by the backend | PLAT-1 | ported | becomes Platform::current(); see platform.rs current_matches_the_build_target |
| ts | src/features/platform/usePlatform.test.tsx | assumes linux until the backend answers, and when it answers nonsense or fails | PLAT-1 | dropped | Tauri IPC platform probe; native knows the platform at compile time |
| ts | src/features/platform/usePlatform.test.tsx | is linux without a provider, so components render alone in tests | PLAT-1 | dropped | Tauri IPC platform probe; native knows the platform at compile time |
| ts | src/features/serial/SerialDialog.test.tsx | suggests a device path that fits the platform (S18.3) | SER-1 | ui |  |
| ts | src/features/serial/SerialDialog.test.tsx | lists detected ports and connects with 115200 8N1 by default | SER-1 | ui |  |
| ts | src/features/serial/SerialDialog.test.tsx | accepts a typed path and a custom baud rate | SER-1 | ui |  |
| ts | src/features/serial/SerialDialog.test.tsx | refreshes the port list and shows validation errors | SER-1 | ui |  |
| ts | src/features/serial/serialBackend.test.ts | opens the device with the target settings | SER-1 | ported |  |
| ts | src/features/serial/serialBackend.test.ts | writes and closes through the serial commands; resize is a no-op | SER-1 | ported | resize-ignored assertion kept; command wrapper part has no native meaning |
| ts | src/features/serial/serialBackend.test.ts | lists the detected ports | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | defaults to 115200 8N1 without flow control | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | offers the common baud rates | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | builds serial settings | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | accepts a custom baud rate | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | reports validation errors | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | round-trips through formFromSettings, including custom baud rates | SER-1 | ported |  |
| ts | src/features/serial/serialSettings.test.ts | shows the backend message | SER-1, SSH-5 | ported | describeSerialError |
| ts | src/features/serial/serialSettings.test.ts | falls back to the raw text | SER-1, SSH-5 | ported |  |
| ts | src/features/sessions/IniImportDialog.test.tsx | previews the counts and imports on confirm | SES-2 | ui |  |
| ts | src/features/sessions/IniImportDialog.test.tsx | only offers Close when there is nothing to import | SES-2 | ui |  |
| ts | src/features/sessions/IniImportDialog.test.tsx | shows a read error | SES-2 | ui |  |
| ts | src/features/sessions/PasswordPrompt.test.tsx | has no keyring checkbox when the session cannot be saved (quick connect) | SSH-4, SSH-6 | ui |  |
| ts | src/features/sessions/PasswordPrompt.test.tsx | offers the checkbox unticked by default and reports it when ticked | SSH-6 | ui |  |
| ts | src/features/sessions/PasswordPrompt.test.tsx | starts ticked when asked to (a saved password was rejected) and reports unticking | SSH-6 | ui |  |
| ts | src/features/sessions/PasswordPrompt.test.tsx | disables the checkbox with an explanation when no keyring is available | SSH-6 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | offers to forget a saved password and deletes it from the keyring | SSH-6 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | shows why forgetting failed | SSH-6 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | does not offer it when nothing is saved | SSH-6 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | does not query the keyring for a new session | SSH-6 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | adds, reorders and removes jump hosts, and saves them in order | SSH-3 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | offers at most 3 jump hosts | SSH-3 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | shows jump hosts that no longer exist so they can be removed | SSH-3 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | offers every session type | SES-1 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | does not offer Mosh on Windows, except to keep an existing Mosh session (S18.3) | MOSH-1 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | saves a Telnet session | SES-1, TEL-1 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | saves a Mosh session over a saved SSH session | SES-1, MOSH-1 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | never queries the keyring for Telnet or Mosh sessions | SSH-6 | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | saves a local session with its shell, arguments and starting directory | SES-1a | ui |  |
| ts | src/features/sessions/SessionEditDialog.test.tsx | chooses the starting directory with Browse… | SES-1d | ui | GPUI prompt_for_paths (directory) |
| ts | src/features/sessions/SessionsPanel.test.tsx | opens a session on double-click and on Enter | SES-1 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | opens a serial session on double-click; its tooltip shows the device | SES-1, SER-1 | ui | tooltip = device path |
| ts | src/features/sessions/SessionsPanel.test.tsx | shows an icon for each session type and for folders | COMP-1 | ui | tree-row protocol icon tints |
| ts | src/features/sessions/SessionsPanel.test.tsx | copies an SSH session as the ssh command that connects to it, with its own key | SES-1c | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | opens a session menu on right click: connect, edit, duplicate, delete | SES-1a, SES-1c | ui | menu now has Copy SSH command |
| ts | src/features/sessions/SessionsPanel.test.tsx | opens a folder menu on right click: new session, new folder, connect all, delete | SES-1a | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | offers only Connect and copying in the session menu while the library cannot be edited | SES-1a, SES-1c | ui | PRD says only Connect; see wrong statement W1 |
| ts | src/features/sessions/SessionsPanel.test.tsx | collapses a folder when its row is clicked | SES-1 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | filters sessions by host | SES-1 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | imports new sessions from a sessions .ini file after the preview | SES-2 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | reads the file picked in the native dialog through the backend | SES-2, UI-5 | ui | GPUI prompt_for_paths + read_bounded |
| ts | src/features/sessions/SessionsPanel.test.tsx | imports the hosts of ~/.ssh/config, keeping key auth and jump hosts | SES-2 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | shows the read error when ~/.ssh/config cannot be read | SES-2 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | does nothing when the dialog is cancelled | SES-2 | ui |  |
| ts | src/features/sessions/SessionsPanel.test.tsx | disables editing and explains why when the file could not be read | SES-1a | ui |  |
| ts | src/features/sessions/iniImport.test.ts | decodes UTF-8 and strips the BOM | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | falls back to Windows-1252 when the bytes are not valid UTF-8 | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | imports SSH and Telnet sessions from every Bookmarks section with their SubRep folder | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | defaults the Telnet port to 23 and skips Telnet lines without a host | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | counts skipped lines per type and reason (Telnet is imported, not skipped) | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | never reads other sections such as [Passwords] | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | accepts LF line endings and rejects out-of-range ports | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | returns nothing for a file without bookmarks | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | adds new sessions with password auth and skips duplicates of saved sessions | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | de-duplicates repeated lines inside the same file | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | does not compare imported hosts against serial sessions | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | skips Telnet duplicates of saved Telnet sessions only | SES-2 | ported |  |
| ts | src/features/sessions/iniImport.test.ts | treats a different port or user as a different session | SES-2 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | parse, serialize and survive a save | SSH-3, DATA-1a | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | are left out when empty, so older files load and save unchanged | SSH-3, DATA-1a | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | reject a file whose jump hosts are not a list of ids | SSH-3, DATA-1a | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | returns the jump sessions in order | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | is empty for sessions without jump hosts | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | goes through the jump hosts of a jump host first | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | rejects a session that is its own jump host | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | rejects cycles | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | rejects a session used twice in the same chain | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | rejects deleted and non-SSH jump sessions | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | rejects more than 3 jump hosts in total | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | lists the sessions that use it | SES-1a, SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | removes it from every chain | SES-1a, SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | removes the jump hosts of a deleted folder from every chain | SES-1a, SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | names jump hosts by position | SSH-2 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | names the target as user@host | SSH-2 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | uses stored passwords, asks for the others one by one and keeps key/agent auth | SSH-6, SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | without prompts, gives every password hop its saved password slot so the backend tries keys first | SSH-1, SSH-6 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | stops at the first cancelled prompt | SSH-6 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | puts the jump hosts, with their own logins, in front of the target | SSH-3 | ported |  |
| ts | src/features/sessions/jumpHosts.test.ts | adds no jumps for a single hop | SSH-3 | ported |  |
| ts | src/features/sessions/library.test.ts | trims segments and drops empty ones | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | adds a session and registers its folder and ancestors | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | duplicates a session next to it with a new id and a (copy) name | SES-1a | ported |  |
| ts | src/features/sessions/library.test.ts | updates an existing session in place by id | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | saves several sessions in one action, like saving them one by one | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | deletes a session | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | adds folders without duplicates | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | deleting a folder removes its subfolders and sessions only | SES-1a | ported |  |
| ts | src/features/sessions/library.test.ts | treats a missing file as an empty library | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | round-trips through JSON | SES-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | reports corrupt or unsupported files instead of returning an empty library | SES-1a, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | loads files written before serial sessions existed (no kind) as SSH sessions | SES-1a, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | round-trips serial sessions and writes their kind | SER-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | rejects serial sessions with invalid fields | SER-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | round-trips Telnet and Mosh sessions and writes their kind | TEL-1, MOSH-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | drops an empty Mosh server command and stray fields | MOSH-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | rejects Telnet and Mosh sessions with invalid fields | TEL-1, MOSH-1, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | treats only SSH sessions as SSH (Telnet and Mosh are not jump hosts) | SSH-3 | ported |  |
| ts | src/features/sessions/library.test.ts | never serializes passwords | DATA-3, SSH-6 | ported |  |
| ts | src/features/sessions/library.test.ts | nests folders first, then sessions, each sorted by name | SES-1 | ported |  |
| ts | src/features/sessions/library.test.ts | filters by name or host and keeps only folders with matches | SES-1 | ported |  |
| ts | src/features/sessions/library.test.ts | matches serial sessions by device path | SES-1 | ported |  |
| ts | src/features/sessions/library.test.ts | adds the password typed at connect time only for password sessions | SSH-6 | ported |  |
| ts | src/features/sessions/library.test.ts | turns a Telnet session into a Telnet target | TEL-1, SES-1 | ported |  |
| ts | src/features/sessions/library.test.ts | turns a serial session into a serial target | SER-1, SES-1 | ported |  |
| ts | src/features/sessions/library.test.ts | round-trips a local session and writes its kind | SES-1a, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | keeps a local session with the system default shell as just its name and folder | SES-1a, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | rejects local sessions with invalid fields | SES-1a, DATA-1a | ported |  |
| ts | src/features/sessions/library.test.ts | finds local sessions by name or program, and opens them as local targets | SES-1, SES-1a | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | names the session as PlexusTerm: user@host:port | DATA-3 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | connects with a reference to the saved password, never the password itself | SSH-6, DATA-3 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | connects with the typed password and carries the storage request | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | omits storage when there is nothing to save or forget | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | leaves a saved password alone when the keyring state is still unknown | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | remembers the password when the checkbox is ticked | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | forgets a saved password when the checkbox is unticked | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | does nothing when unticked and nothing was saved | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | asks for the password when the saved one is missing or rejected | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | does not ask again for other failures | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | never loops on a typed password | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | is true when the server rejected the saved password | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | is false when no saved password could be used (asked like a first login) | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | lists the deleted SSH session | SSH-6, SES-1a | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | lists every SSH session inside a deleted folder, but no serial session | SSH-6, SES-1a | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | lists nothing for actions that delete no session | SSH-6, SES-1a | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | forgets the saved password when a session stops using password authentication | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | forgets it when the host, port or username changes | SSH-6 | ported |  |
| ts | src/features/sessions/passwordFlow.test.ts | keeps it while the session still uses a password for the same endpoint, and ignores other sessions | SSH-6 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | builds a saved session with a normalized folder and no password | SES-1a, DATA-3 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | defaults the name to user@host | SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | keeps the key path but not the passphrase | SSH-6 | ported | passphrase never saved |
| ts | src/features/sessions/sessionForm.test.ts | reports validation errors | SES-1a | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | builds a serial session named after the device by default | SER-1, SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | reports serial validation errors | SER-1 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | round-trips a serial session | SER-1 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | builds a Telnet session on port 23 by default, named after the host | TEL-1, SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | reports Telnet validation errors | TEL-1 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | builds a Mosh session over a saved SSH session, keeping a custom server command | MOSH-1 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | requires an existing SSH session for Mosh | MOSH-1, SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | round-trips Telnet and Mosh sessions | TEL-1, MOSH-1 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | round-trips a saved session | SES-1a | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | leaves the name empty when it was made from the address, so it follows a new host | SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | keeps a name the user chose, even one that looks like an address | SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | keeps the ordered jump hosts of an SSH session | SSH-3 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | leaves jumpIds out when there are none | SSH-3 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | rejects a chain that loops back through another session's jump hosts | SSH-3 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | rejects more than 3, repeated or self jump hosts | SSH-3 | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | saves the shell, its split arguments and the starting directory | SES-1a | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | saves the system default shell without program or arguments, named Local shell | SES-1a, SES-1e | ported |  |
| ts | src/features/sessions/sessionForm.test.ts | fills the form back, quoting arguments that have spaces | SES-1a | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | reads each concrete Host with its HostName, User, Port and IdentityFile | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | uses the alias as the host when there is no HostName, and expands %h | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | applies wildcard blocks and global options, keeping the first value of each option like ssh | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | imports every alias of a multi-pattern Host but skips wildcard and negated patterns | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | keeps a quoted value with spaces and is case-insensitive about keywords | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | replaces %d in IdentityFile with the home folder | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | falls back to port 22 for an invalid Port | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | links ProxyJump hops to the imported hosts with those names | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | counts ProxyJump hops that are not imported hosts, and imports the host without them | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | ignores ProxyJump none | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | counts Include and Match lines and ProxyCommand hosts, which are not followed | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | returns nothing for an empty file | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | keeps the key auth and turns ProxyJump names into the ids of the saved sessions | SES-2 | ported |  |
| ts | src/features/sessions/sshConfigImport.test.ts | links a jump to the session saved by an earlier import when that host is a duplicate | SES-2 | ported |  |
| ts | src/features/sessions/types.test.ts | names serial tabs after the device and baud rate | SER-1 | ported |  |
| ts | src/features/sessions/types.test.ts | names Telnet tabs after the host (port only when not 23) and Mosh tabs after the SSH login | TEL-1, MOSH-1, SES-1e | ported |  |
| ts | src/features/sessions/types.test.ts | finds the SSH login of SSH and Mosh targets only | MOSH-1, TERM-12b | ported | history key and in-memory credentials |
| ts | src/features/sessions/types.test.ts | keeps the local and SSH titles | SES-1a, TERM-16 | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | builds a password target with the default port | SSH-4 | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | accepts user@host:port in the host field | SSH-4 | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | parses the IPv6 address %s | SSH-4, SSH-4a | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | an explicit port field wins over the host field | SSH-4, SSH-4a | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | builds key and agent targets | SSH-4 | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | keeps a key passphrase only when given | SSH-4, SSH-6 | ported |  |
| ts | src/features/ssh/quickConnect.test.ts | rejects invalid input %o | SSH-4 | ported |  |
| ts | src/features/ssh/sshErrors.test.ts | passes structured backend errors through | SSH-5 | ported | native: error enum consumed directly |
| ts | src/features/ssh/sshErrors.test.ts | wraps plain strings and unknown values as connection errors | SSH-5 | ported |  |
| ts | src/features/ssh/sshErrors.test.ts | explains a changed host key without offering to trust it | SSH-2, SSH-5 | ported |  |
| ts | src/features/ssh/sshErrors.test.ts | warns when keys of another type are already known for the host | SSH-2, SSH-5 | ported |  |
| ts | src/features/ssh/sshErrors.test.ts | describes the other kinds | SSH-5 | ported |  |
| ts | src/features/ssh/sshErrors.test.ts | name the hop that failed | SSH-3, SSH-5 | ported |  |
| ts | src/features/telnet/TelnetDialog.test.tsx | connects to the host on port 23 by default | TEL-1 | ui |  |
| ts | src/features/telnet/TelnetDialog.test.tsx | uses the typed port and reports invalid input | TEL-1 | ui |  |
| ts | src/features/telnet/TelnetDialog.test.tsx | cancels | TEL-1 | ui |  |
| ts | src/features/telnet/telnetBackend.test.ts | connects to the host and port with the terminal size | TEL-1 | ported |  |
| ts | src/features/telnet/telnetBackend.test.ts | writes, resizes and closes through the telnet commands | TEL-1 | dropped | Tauri command wrapper; native calls the service directly |
| ts | src/features/tunnels/TunnelsPanel.test.tsx | shows each tunnel with its route, session and a Start toggle while stopped | TUN-1 | ui |  |
| ts | src/features/tunnels/TunnelsPanel.test.tsx | shows the warning of a running tunnel | SSH-6, TUN-1 | ui |  |
| ts | src/features/tunnels/TunnelsPanel.test.tsx | offers Stop while running and shows error messages | TUN-1 | ui |  |
| ts | src/features/tunnels/TunnelsPanel.test.tsx | disables editing when the file could not be read | TUN-1a | ui |  |
| ts | src/features/tunnels/tunnelForm.test.ts | defaults the name and the listen host | TUN-1, TUN-1a | ported |  |
| ts | src/features/tunnels/tunnelForm.test.ts | keeps a typed name and listen host, trimmed | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnelForm.test.ts | requires ports from 1 to 65535 | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnelForm.test.ts | requires a destination except for dynamic tunnels | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnelForm.test.ts | requires an existing saved session | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnelForm.test.ts | round-trips through the form | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnelForm.test.ts | leaves the destination empty for dynamic tunnels | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | adds, replaces and deletes tunnels by id | TUN-1, DATA-1a | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | treats a missing file as an empty list | TUN-1, DATA-1a | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | round-trips and drops fields that are not part of the saved shape | TUN-1, DATA-1a | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | reports corrupt files as errors instead of emptying them | TUN-1a, DATA-1a | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | accepts dynamic tunnels without a destination | TUN-1, DATA-1a | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | summarizes each type | TUN-1, DATA-1a | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | accepts only loopback listen addresses | TUN-1 | ported |  |
| ts | src/features/tunnels/tunnels.test.ts | covers the whole 127.0.0.0/8 range and bracketed ::1, but not look-alikes | TUN-1 | ported |  |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | does not retry a tunnel deleted while its password prompt was open | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | does not retry a tunnel edited into another definition while its password prompt was open | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | retries the tunnel after the password prompt when it is unchanged | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | does not retry a tunnel deleted while the host key prompt was open | TUN-1, SSH-2 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | reports a start failure that never reached the tunnel manager | TUN-1 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | starts with the saved password, without prompting, when one exists | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | connects before asking when no password is saved, so the server can accept a key | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | asks after connecting when no password is saved, unticked, and saves it when ticked | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | asks again with the checkbox ticked when the saved password is rejected, then replaces it | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | forgets the saved password when the checkbox is unticked on the retry | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | does not ask again when a typed password is rejected | TUN-1, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | leaves failures the backend already shows as the tunnel status alone | TUN-1 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | asks for a jump host password after connecting, naming the hop, and retries through it | TUN-1, SSH-3, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | asks to trust an unknown jump host and retries with that hop's fingerprint | TUN-1, SSH-2, SSH-3 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | asks again for a jump host whose saved password was rejected | TUN-1, SSH-3, SSH-6 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnelLauncher.test.tsx | reports a broken chain without starting | TUN-1, SSH-3 | ported | launcher hook → tunnel view-model |
| ts | src/features/tunnels/useTunnels.test.ts | keeps a live status event when an older snapshot arrives after it | TUN-1 | ported |  |
| rs | src-tauri/src/launch.rs | finds_an_absolute_workspace_file | WS-1 | ported |  |
| rs | src-tauri/src/launch.rs | takes_a_relative_workspace_file_from_the_launch_folder | WS-1, WS-1a | ported |  |
| rs | src-tauri/src/launch.rs | accepts_a_file_uri_and_any_case_of_the_extension | WS-1, WS-1a | ported |  |
| rs | src-tauri/src/launch.rs | ignores_the_program_name_and_other_arguments | WS-1, WS-1a | ported |  |
| rs | src-tauri/src/launch.rs | hands_the_start_file_out_once | WS-1, WS-1a | ported |  |
| rs | src-tauri/src/launch.rs | hands_a_workspace_file_to_the_listening_plexusterm_only | WS-1, DATA-5, WS-1a | ported | stale socket replaced |
| rs | src-tauri/src/mosh/tests.rs | a_missing_client_is_explained_per_platform | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | parses_the_mosh_connect_line | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | rejects_malformed_mosh_connect_lines | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | debug_never_shows_the_key | MOSH-1, NFR-11 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | requests_deserialize_from_camel_case | MOSH-1 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/mosh/tests.rs | mosh_client_gets_the_key_by_environment_only | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | the_ssh_connection_is_closed_once_the_server_started | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | a_custom_server_command_is_run | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | a_missing_mosh_client_fails_before_logging_in | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | a_missing_mosh_server_is_a_clear_error | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | a_server_command_without_mosh_connect_is_an_error | MOSH-1 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | a_server_that_never_answers_times_out | MOSH-1, MOSH-1a | ported | 20 s SERVER_TIMEOUT |
| rs | src-tauri/src/mosh/tests.rs | ssh_login_errors_are_returned_as_is | MOSH-1, SSH-5 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | the_server_is_started_through_jump_hosts | MOSH-1, SSH-3 | ported |  |
| rs | src-tauri/src/mosh/tests.rs | a_mangled_connect_line_never_echoes_the_key | MOSH-1, NFR-11 | ported |  |
| rs | src-tauri/src/platform.rs | webkit_frames_go_through_shared_memory_only_with_nvidias_driver | n/a | dropped | WebKitGTK DMABUF env tuning; no webview natively |
| rs | src-tauri/src/platform.rs | home_dir_comes_from_home_on_unix_and_userprofile_on_windows | PLAT-1 | ported |  |
| rs | src-tauri/src/platform.rs | expands_tilde_slash_on_both_and_tilde_backslash_only_on_windows | PLAT-1 | ported |  |
| rs | src-tauri/src/platform.rs | unix_shells_are_dollar_shell_then_the_executable_entries_of_etc_shells | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | unix_shells_skip_duplicates_nologin_false_and_comments | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | windows_without_comspec_uses_cmd_on_path_and_skips_missing_git_bash | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | unix_shells_without_etc_shells_or_dollar_shell_fall_back_to_bash | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | windows_shells_are_powershell_cmd_wsl_and_git_bash_when_installed | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | shell_names_drop_the_directory_and_the_exe_extension | TERM-16, SES-1a | ported |  |
| rs | src-tauri/src/platform.rs | unix_shell_is_dollar_shell_or_bash | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | windows_shell_prefers_pwsh_then_powershell_then_comspec | TERM-16 | ported |  |
| rs | src-tauri/src/platform.rs | agent_endpoint_uses_ssh_auth_sock_then_the_windows_pipe | SSH-1, PLAT-1 | ported |  |
| rs | src-tauri/src/platform.rs | program_names_get_exe_on_windows | MOSH-1, PLAT-1 | ported |  |
| rs | src-tauri/src/platform.rs | expand_home_uses_this_machines_home | PLAT-1 | ported |  |
| rs | src-tauri/src/platform.rs | current_matches_the_build_target | PLAT-1 | ported |  |
| rs | src-tauri/src/platform.rs | private_files_are_0600_on_unix | DATA-2 | ported |  |
| rs | src-tauri/src/project.rs | saves_and_reads_both_colours_in_one_file_in_the_project_folder | DEV-2, DATA-1b | ported |  |
| rs | src-tauri/src/project.rs | the_file_is_readable_like_the_project_files | DATA-1b | ported | 0644; contradicts DATA-1 'also private' (W2) |
| rs | src-tauri/src/project.rs | the_folder_ignores_itself_in_git_without_touching_the_project_gitignore | DEV-2, DATA-1b | ported |  |
| rs | src-tauri/src/project.rs | colours_saved_at_the_same_time_are_both_kept | DEV-2, DATA-1b | ported |  |
| rs | src-tauri/src/project.rs | keeps_the_other_settings | DEV-2, DATA-1b | ported |  |
| rs | src-tauri/src/project.rs | clearing_a_colour_never_creates_the_folder | DEV-2, DATA-1b | ported |  |
| rs | src-tauri/src/project.rs | refuses_invalid_colours_a_missing_project_and_a_settings_file_it_cannot_read | DEV-2, DATA-1b | ported |  |
| rs | src-tauri/src/project.rs | the_kind_comes_from_the_frontend_as_tab_or_toolbar | DEV-2, DATA-1b | ported | file keys tabColor/toolbarColor kept; ColorKind IPC part moot |
| rs | src-tauri/src/secrets.rs | run_uses_the_blocking_pool_on_a_current_thread_runtime | DATA-3 | ported | keyring calls on the Tokio blocking pool via the runtime bridge |
| rs | src-tauri/src/secrets.rs | only_secret_service_platforms_get_a_label_modifier | DATA-3 | ported |  |
| rs | src-tauri/src/secrets.rs | no_store_is_unavailable_and_never_stores | SSH-6, DATA-3 | ported |  |
| rs | src-tauri/src/secrets.rs | errors_serialize_with_a_kind_tag_and_no_secret_bytes | NFR-11, DATA-3 | ported | keep the no-secret-bytes assertion on Display/Debug |
| rs | src-tauri/src/secrets.rs | real_keyring_round_trip | DATA-3 | ported | #[ignore]; run by hand with a live keyring |
| rs | src-tauri/src/serial/ports.rs | lists_only_hardware_backed_serial_devices_sorted | SER-1 | ported |  |
| rs | src-tauri/src/serial/ports.rs | missing_sysfs_directory_lists_nothing | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | streams_device_output_to_the_sink | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | writes_input_to_the_device | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | rejects_invalid_settings_before_opening | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | missing_device_is_not_found | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | device_already_open_is_busy | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | errors_serialize_with_kind_and_message | SER-1 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/serial/tests.rs | permission_errors_map_to_permission_denied | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | unplugged_device_reports_the_error_and_ends | SER-1 | ported |  |
| rs | src-tauri/src/serial/tests.rs | close_stops_the_reader_and_forgets_the_session | SER-1 | ported |  |
| rs | src-tauri/src/ssh/cwd.rs | parses_the_printed_folder | SSH-5 | ported |  |
| rs | src-tauri/src/ssh/cwd.rs | no_folder_is_none | SSH-5 | ported |  |
| rs | src-tauri/src/ssh/cwd.rs | the_script_runs_in_sh_and_prints_at_most_a_folder | SSH-5 | ported |  |
| rs | src-tauri/src/ssh/cwd.rs | shell_cwd_runs_the_command_over_the_session | SSH-5 | ported |  |
| rs | src-tauri/src/ssh/interop_tests.rs | openssh_shell_resize_and_known_hosts | INTEROP-1 | ported | Docker OpenSSH interop job |
| rs | src-tauri/src/ssh/interop_tests.rs | openssh_sftp_round_trip | INTEROP-1 | ported | Docker OpenSSH interop job |
| rs | src-tauri/src/ssh/interop_tests.rs | openssh_local_tunnel_reaches_a_service_behind_the_server | INTEROP-1 | ported | Docker OpenSSH interop job |
| rs | src-tauri/src/ssh/interop_tests.rs | openssh_through_a_jump_host | INTEROP-1 | ported | Docker OpenSSH interop job |
| rs | src-tauri/src/ssh/mod.rs | password_login_opens_an_interactive_shell | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | wrong_password_is_rejected | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | private_key_login_works | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | unknown_host_is_rejected_with_its_fingerprint_and_not_recorded | SSH-2 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | unknown_key_type_reports_the_key_types_already_known_for_the_host | SSH-2 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | unknown_host_error_serializes_with_camel_case_fields | SSH-2 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/ssh/mod.rs | debug_output_never_contains_secrets | NFR-11 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | large_paste_is_fully_echoed_without_stalling | SSH-1, NFR-2 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | agent_login_uses_the_agent_identities | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | trusting_the_fingerprint_connects_and_records_the_host | SSH-2 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | trusting_a_different_fingerprint_is_still_rejected | SSH-2 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | changed_host_key_is_rejected_even_when_trusted | SSH-2 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | requests_the_initial_size_and_forwards_resizes | SSH-1, TERM-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | remote_exit_reports_the_exit_status | SSH-1, TERM-19 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | close_ends_the_session_and_forgets_it | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | stored_password_login_reads_the_secret_from_the_store | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_secret_stored_for_another_port_is_never_sent | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_secret_stored_for_another_user_is_never_sent | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_secret_stored_for_another_host_is_never_sent | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_legacy_plain_text_secret_is_never_sent | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | an_unsupported_secret_format_is_never_sent | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | endpoints_are_normalized | DATA-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | stored_password_without_a_secret_is_secret_missing | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_wrong_stored_password_is_auth_failed_and_the_secret_is_kept | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | remember_stores_the_typed_password_after_a_successful_login | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | remember_stores_nothing_when_the_login_fails | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | remember_only_stores_typed_passwords | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_password_the_keyring_cannot_save_is_reported_in_the_terminal | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_successful_save_writes_nothing_to_the_terminal | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_keyring_that_does_not_answer_does_not_hold_the_login_back | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | forget_deletes_the_secret_only_after_a_successful_login | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | stored_password_and_storage_requests_deserialize_from_camel_case | SSH-6 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/ssh/mod.rs | debug_of_stored_password_requests_never_contains_the_password | NFR-11, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | unreachable_host_is_a_connection_error | SSH-5 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | one_jump_host_reaches_the_final_shell | SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | two_jump_hosts_reach_the_final_shell | SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | an_unknown_jump_host_names_the_hop_and_trusting_it_connects | SSH-2, SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | an_unknown_target_behind_a_jump_host_is_not_wrapped | SSH-2, SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_wrong_jump_password_names_the_hop_and_never_reaches_the_target | SSH-3, SSH-5 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_failing_second_jump_names_its_position_and_closes_the_first | SSH-3, SSH-5 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | stored_jump_passwords_are_bound_to_the_jump_endpoint | SSH-3, DATA-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | remember_on_a_jump_host_saves_its_password_for_its_endpoint | SSH-3, SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | closing_a_chained_session_closes_every_hop | SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_remote_exit_behind_jump_hosts_closes_every_hop | SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | sftp_works_over_a_chained_session | SSH-3, SFTP-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | more_than_three_jump_hosts_are_refused | SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | jump_hosts_deserialize_from_camel_case | SSH-3 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/ssh/mod.rs | debug_of_jump_hosts_never_contains_their_passwords | NFR-11, SSH-3 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | exec_returns_the_stdout_of_a_command_over_the_existing_connection | SSH-5, TOOL-4, DEV-3 | ported | exec service used by remote cwd, monitor, git branch, Install |
| rs | src-tauri/src/ssh/mod.rs | exec_reports_a_failing_command_with_its_stderr | SSH-5, TOOL-4, DEV-3 | ported | exec service used by remote cwd, monitor, git branch, Install |
| rs | src-tauri/src/ssh/mod.rs | exec_on_an_unknown_session_fails | SSH-5, TOOL-4, DEV-3 | ported | exec service used by remote cwd, monitor, git branch, Install |
| rs | src-tauri/src/ssh/mod.rs | a_password_login_uses_an_accepted_default_key_without_sending_the_password | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_stored_password_login_uses_an_accepted_default_key_without_reading_the_keyring | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_password_login_uses_an_accepted_agent_key_without_sending_the_password | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | an_encrypted_default_key_is_skipped_and_the_password_logs_in | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | rejected_default_keys_fall_back_to_the_stored_password | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | rejected_default_keys_without_a_stored_password_are_secret_missing | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | rejected_default_keys_then_a_wrong_password_is_auth_failed | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | the_password_still_gets_a_chance_when_the_server_drops_after_too_many_keys | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | remember_saves_nothing_when_a_default_key_logged_in | SSH-6 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_key_file_login_does_not_try_the_default_keys | SSH-1 | ported |  |
| rs | src-tauri/src/ssh/mod.rs | a_password_jump_host_logs_in_with_a_default_key | SSH-3 | ported |  |
| rs | src-tauri/src/storage.rs | read_bounded_returns_the_bytes_of_small_files | DATA-2, SES-2 | ported |  |
| rs | src-tauri/src/storage.rs | read_bounded_refuses_files_over_the_limit_and_missing_files | DATA-2, SES-2 | ported | 5 MiB import cap |
| rs | src-tauri/src/storage.rs | write_text_creates_or_replaces_the_file | DATA-2, TERM-10 | ported |  |
| rs | src-tauri/src/storage.rs | write_text_refuses_contents_over_the_limit_and_leaves_no_file | DATA-2, TERM-10 | ported |  |
| rs | src-tauri/src/storage.rs | append_text_creates_the_file_then_appends | DATA-2, TERM-10 | ported |  |
| rs | src-tauri/src/storage.rs | missing_file_loads_as_none | DATA-2 | ported |  |
| rs | src-tauri/src/storage.rs | save_then_load_round_trips_and_creates_parent_dirs | DATA-2 | ported |  |
| rs | src-tauri/src/storage.rs | save_overwrites_and_leaves_no_temp_files | DATA-2 | ported |  |
| rs | src-tauri/src/storage.rs | invalid_json_is_rejected_and_keeps_the_previous_file | DATA-2 | ported |  |
| rs | src-tauri/src/storage.rs | concurrent_saves_never_corrupt_the_file | DATA-2 | ported |  |
| rs | src-tauri/src/storage.rs | saved_file_is_private_to_the_user | DATA-2, NFR-6 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | start_offers_terminal_type_and_naws | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | an_unterminated_subnegotiation_is_bounded | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | plain_data_passes_through | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | escaped_iac_in_output_is_one_data_byte | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | commands_are_stripped_from_output | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | nul_after_cr_is_dropped_from_output | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | server_echo_and_sga_are_accepted_once | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | other_server_options_are_refused | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | unsupported_do_is_refused | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | do_naws_after_our_offer_sends_the_size_without_repeating_will | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | unsolicited_do_naws_is_accepted_with_will_and_the_size | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | resize_sends_naws_only_once_it_is_enabled | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | dont_for_an_enabled_option_is_acknowledged_once | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | terminal_type_send_is_answered_with_xterm_256color | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | terminal_type_send_is_ignored_while_ttype_is_off | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | negotiation_split_across_reads_is_handled | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | subnegotiation_data_is_not_terminal_output | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | input_iac_is_doubled | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/protocol.rs | input_bare_cr_becomes_cr_nul_and_cr_lf_is_kept | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | naws_is_sent_with_the_initial_size_and_again_on_resize | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | terminal_type_is_answered | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | server_echo_and_sga_are_accepted_and_other_options_refused | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | input_iac_is_escaped_and_enter_is_sent_as_cr_nul | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | output_is_decoded_with_iac_unescaped_and_commands_stripped | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | server_closing_ends_the_session | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | close_disconnects_and_forgets_the_session | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | refused_connection_is_a_clear_error | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | connect_times_out | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | errors_serialize_with_kind_and_message | TEL-1 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/telnet/tests.rs | request_port_defaults_to_23 | TEL-1 | ported |  |
| rs | src-tauri/src/telnet/tests.rs | connects_to_an_ipv6_literal | TEL-1, SSH-4a | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | greeting_needs_the_whole_method_list | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | greeting_accepts_no_auth_among_other_methods | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | greeting_without_no_auth_or_with_another_version_is_rejected | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | request_parses_ipv4_targets | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | request_parses_domain_targets | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | request_parses_ipv6_targets | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | request_rejects_other_commands_and_address_types | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | handshake_closes_a_silent_client_without_a_reply | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | handshake_closes_a_client_that_stops_mid_greeting | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | handshake_limit_covers_greeting_and_request_together | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/socks.rs | reply_reports_the_code_with_an_unspecified_address | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | local_forward_round_trips_bytes_and_reports_running | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | stop_closes_the_listener_and_open_connections | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | remote_forward_reaches_a_local_service_until_stopped | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | dynamic_forward_connects_by_domain_and_ipv4 | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | dynamic_forward_rejects_unsupported_commands_and_methods | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | stop_while_starting_releases_the_listener_at_once | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | port_in_use_is_an_error_and_sets_the_error_status | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | unknown_host_reports_its_fingerprint_like_ssh_sessions | TUN-1, SSH-2 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | starting_a_running_tunnel_is_refused | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | definitions_and_statuses_use_the_frontend_field_names | TUN-1, DATA-1a | ported | definition keys are the tunnels.json schema; status part is IPC |
| rs | src-tauri/src/tunnels/tests.rs | a_stop_racing_the_running_notification_is_reported_last | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_dropped_ssh_connection_sets_the_error_status_and_releases_the_listener | TUN-1 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_stored_password_starts_the_tunnel | TUN-1, SSH-6 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_tunnel_start_can_remember_the_typed_password | TUN-1, SSH-6 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_stored_password_for_another_endpoint_is_never_sent | TUN-1, DATA-3 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_password_the_keyring_cannot_save_is_a_warning_of_the_running_tunnel | TUN-1, SSH-6 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_keyring_that_does_not_answer_does_not_hold_the_tunnel_back | TUN-1, SSH-6 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | running_without_a_warning_serializes_like_before | TUN-1 | dropped | Tauri IPC wire shape; §6.1 has no IPC serialization |
| rs | src-tauri/src/tunnels/tests.rs | a_local_forward_through_a_jump_host_round_trips_and_stop_closes_every_hop | TUN-1, SSH-3 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_remote_forward_through_a_jump_host_reaches_a_local_service | TUN-1, SSH-3 | ported |  |
| rs | src-tauri/src/tunnels/tests.rs | a_failing_jump_host_is_named_in_the_tunnel_error | TUN-1, SSH-3 | ported |  |

## 3. Files and viewer

| Kind | File | Test | Req | Disposition | Note |
| --- | --- | --- | --- | --- | --- |
| ts | src/features/files/fileIcons.test.ts | gives the files of a project the icons VS Code shows | TREE-1 | ported |  |
| ts | src/features/files/fileIcons.test.ts | finds the language of an extension, ignoring case, as VS Code's language registry does | TREE-1 | ported |  |
| ts | src/features/files/fileIcons.test.ts | prefers the longest known extension and exact names | TREE-1 | ported |  |
| ts | src/features/files/fileIcons.test.ts | draws a glyph of the Seti font, with VS Code's colours for dark and light schemes | TREE-1 | ui | glyph + scheme colours drawn by GPUI |
| ts | src/features/files/fileIcons.test.ts | uses the default file glyph for unknown types; folders and links keep their own icons | TREE-1 | ported |  |
| ts | src/features/files/gitStatus.test.ts | gives files VS Code's letter, colour and label | TREE-1, TREE-3 | ported |  |
| ts | src/features/files/gitStatus.test.ts | picks the highest priority when the index and the working tree differ | TREE-3 | ported | priority: conflict > ignored > modified > others |
| ts | src/features/files/gitStatus.test.ts | colours a folder like its most important descendant, without a letter, never for deleted or ignored ones | TREE-3 | ported |  |
| ts | src/features/files/gitStatus.test.ts | treats a folder entry as covering everything inside it | TREE-3 | ported |  |
| ts | src/features/files/gitStatus.test.ts | is a dot coloured by the most important change inside, as a folder's | TREE-3, WS-2 | ported | workspace root dot |
| ts | src/features/files/gitStatus.test.ts | is none when nothing changed, or only ignored files are listed | TREE-3 | ported |  |
| ts | src/features/files/useGitStatus.test.ts | reads the folder when it loads and when it changes | TREE-3 | ported | view-model |
| ts | src/features/files/useGitStatus.test.ts | reads at most once every 2 s while the terminal prints, and never when idle | TREE-3 | ported | 2 s cap |
| ts | src/features/files/useGitStatus.test.ts | never reads for the echo of what is typed | TREE-3 | ported |  |
| ts | src/features/files/useGitStatus.test.ts | keeps the same result when the status did not change (the list is not drawn again) | TREE-3 | ported |  |
| ts | src/features/files/useGitStatus.test.ts | reads again when the prompt comes back or the list is loaded again | TREE-3 | ported |  |
| ts | src/features/files/useGitStatus.test.ts | reads nothing while hidden or turned off, and nothing outside a folder | TREE-3 | ported |  |
| ts | src/features/files/useGitStatus.test.ts | a failed read clears the decorations instead of showing an error | TREE-3 | ported |  |
| ts | src/features/files/useGitStatus.test.ts | stops listening to the terminal when unmounted | TREE-3 | ported | subscription released on drop |
| ts | src/features/files/workspaceFile.test.ts | writes a new file as VS Code does: paths relative to the file, empty settings, tabs | WS-1, WS-2 | ported |  |
| ts | src/features/files/workspaceFile.test.ts | writes Windows paths with backslashes, absolute on another drive | WS-1 | ported |  |
| ts | src/features/files/workspaceFile.test.ts | rewrites only the folders of an existing file, keeping its comments, settings, names and remote folders | WS-1, WS-2 | ported | JSONC preserved |
| ts | src/features/files/workspaceFile.test.ts | writes a new file over one that is not a workspace | WS-1 | ported |  |
| ts | src/features/files/workspaceFile.test.ts | reads VS Code's files: comments, trailing commas, names and paths relative to the file | WS-1 | ported |  |
| ts | src/features/files/workspaceFile.test.ts | resolves Windows paths | WS-1 | ported |  |
| ts | src/features/files/workspaceFile.test.ts | keeps each folder once and leaves out remote folders and broken entries | WS-1 | ported |  |
| ts | src/features/files/workspaceFile.test.ts | finds no folders in a file that is not a workspace | WS-1 | ported |  |
| ts | src/features/local/App.explorer.test.tsx | opens a file Ctrl+clicked in the terminal in the editor and shows it in the Explorer | TERM-14a | ui |  |
| ts | src/features/local/App.explorer.test.tsx | is enabled for a local terminal and shows the folder its shell is in | LOC-1, UI-3a | ui |  |
| ts | src/features/local/App.explorer.test.tsx | drops a row on the terminal: cd into a folder on an empty line, otherwise types the path | DROP-1 | ui |  |
| ts | src/features/local/App.explorer.test.tsx | runs cd in the terminal for a folder opened in the Explorer, on an empty line only (L16) | SFTP-4 | ui | terminalFollowsBrowser |
| ts | src/features/local/App.explorer.test.tsx | leaves the terminal's folder alone when Follow terminal folder is off | SFTP-4 | ui |  |
| ts | src/features/local/App.explorer.test.tsx | opens on the Explorer for a local terminal, and leaves another chosen tab alone (L18) | UI-3a | ui |  |
| ts | src/features/local/App.openInTerminal.test.tsx | opens a local terminal tab that starts in the Explorer's folder | LOC-1 | ui | same shell, starts in folder |
| ts | src/features/local/App.openInTerminal.test.tsx | opens another tab of the same SSH session and changes to the folder once its shell is ready | SFTP-1a | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | starts in the home folder, enters folders and goes up | LOC-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows size and date only when Settings ask for them (L15) | SET-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | reports the folders the user opens, never those it follows (L16) | SFTP-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | opens a file in its default program on double-click | LOC-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | follows the terminal folder only while following is on | SFTP-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | creates, renames and deletes entries, then lists the folder again | LOC-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | refuses names with a separator | LOC-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows the command's error | LOC-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | copies a path and hands the raw path to the terminal | SFTP-1a, DROP-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | opens a file with the text editor chosen in Settings | SFTP-1a | ui | textEditor |
| ts | src/features/local/LocalPanel.test.tsx | colours entries like VS Code: the letter and tooltip on files, the colour and a dot on folders | TREE-1, TREE-3 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | reads and shows nothing when the setting is off | TREE-3 | ui | gitStatus setting off |
| ts | src/features/local/LocalPanel.test.tsx | prefers OSC 7 and otherwise asks the backend every second while enabled | SFTP-4, TERM-15 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | asks the given reader instead, e.g. the remote shell of an SSH session | SFTP-4 | ui | reader injected (SSH) |
| ts | src/features/local/LocalPanel.test.tsx | shows an icon for each entry's type at its left, keeping the icon's colour when git colours the name | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | expands and collapses a folder in place with a chevron, indenting its children | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | collapses every open folder and lists everything again from the DEV toolbar, as VS Code's Explorer | TREE-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | has no collapse or refresh icons outside DEV mode | TREE-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | loads a folder's children once, when it is first expanded, with a loading and an error state | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | keeps the open folders when the list is refreshed | TREE-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | moves with the keyboard: Right opens and enters, Left leaves and closes, Up and Down move | TREE-1 | ui | keyboard navigation |
| ts | src/features/local/LocalPanel.test.tsx | shows a dot on folders holding changes and the letter on the file | TREE-3 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | keeps the entry actions on nested entries: renaming stays in the entry's own folder | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | offers Open in terminal right after Open on folders, never on files | SFTP-1a, LOC-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | keeps the plain list: generic icons, double-click enters folders, a click does not expand, no git call | LOC-1, DEV-2 | ui | non-DEV plain list |
| ts | src/features/local/LocalPanel.test.tsx | adds a folder chosen in the system dialog, only in DEV mode | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | collapses the workspace roots too when the workspace has several folders, as VS Code's Explorer | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | does not add a folder when the dialog is cancelled or it is already there | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows every folder as a VS Code workspace root once one is added: folds, git dot, remove from its menu | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | gives each root title the menu and the drag of a folder row | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | removes the main folder from the workspace: the first added one becomes the main folder | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows the folder as a root even while no folder is added, without Remove folder from workspace | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows no added folders outside DEV mode | WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | saves the workspace (its folder first) to a VS Code workspace file | WS-1, WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | saves over an existing workspace file keeping what it has besides the folders | WS-1, WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | opens the folders of a workspace file, and says so when it lists none | WS-1, WS-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | opens the tree's folders down to a file under the listed folder and selects it | TERM-14a | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | goes to the folder of a file outside the listed folders and selects it | TERM-14a | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | lists the folder again while the terminal prints and when its prompt comes back, keeping the selection | TREE-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows its actions, and DEV mode's workspace ones, as icons named by their tooltips | VIEW-2a | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | views a file from its menu; folders have no View item | VIEW-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | views a double-clicked file in DEV mode, and opens it as before outside it | VIEW-1, DEV-2 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows the selected file's project and branch under the list in DEV mode, with * for changes | TREE-1, TREE-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | shows nothing outside a repository, and reads nothing outside DEV mode or with git status off | TREE-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | keeps showing the repository while it is read again, without flickering | TREE-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | marks folders, files and the list as drop targets in DEV mode only | TREE-1 | ui | drag-to-move |
| ts | src/features/local/LocalPanel.test.tsx | asks before moving, then moves the entry and lists the folder again | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | moves nothing when the question is cancelled | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | says why a move failed | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | ignores drops that would move nothing and drops from an SSH session | TREE-1 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | lists the recent folders from the toolbar, only in DEV mode, and opens the one chosen | DEV-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | lists the recent workspace files first, apart, and opens the one chosen | DEV-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | enables the button with only recent workspace files | DEV-4 | ui |  |
| ts | src/features/local/LocalPanel.test.tsx | disables the button while there are no recent folders | DEV-4 | ui |  |
| ts | src/features/local/localPath.test.ts | join with the separator the folder uses | LOC-1 | ported |  |
| ts | src/features/local/localPath.test.ts | go up to the root and stay there | LOC-1 | ported |  |
| ts | src/features/local/localPath.test.ts | tell roots and names | LOC-1 | ported |  |
| ts | src/features/local/localPath.test.ts | refuse names with separators | LOC-1 | ported |  |
| ts | src/features/sftp/EditUploads.test.tsx | uploads saved copies of every session, whichever panel is shown, asking once per file | SFTP-2 | ui |  |
| ts | src/features/sftp/EditUploads.test.tsx | does nothing when the upload is declined, and reports failures | SFTP-2 | ui |  |
| ts | src/features/sftp/EditUploads.test.tsx | asks about two saved files one after the other | SFTP-2 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | starts in the home folder and enters a folder on double-click | SFTP-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | follows the terminal folder only while following is on | SFTP-4 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | shows errors inline and keeps the current listing | SFTP-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | deletes the selected folder only after confirmation | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | creates a folder with the typed name, and nothing when cancelled | SFTP-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | creates an empty file with the typed name, and nothing when cancelled | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | asks before replacing files on upload and uploads only when accepted | SFTP-1 | ui | upload replace confirmation |
| ts | src/features/sftp/SftpPanel.test.tsx | offers the common and file-only items on a file | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | offers only the common items on a folder, plus Open in terminal, without Download | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Open enters a folder | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Open opens a file with the default program; Download… downloads it | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Delete asks first and removes the entry | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Rename… renames the clicked entry, starting from its name, and nothing when cancelled | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Copy file path puts the absolute path on the clipboard | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Copy file path to terminal sends the quoted path, and middle click does the same | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | disables Copy file path to terminal without a terminal | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Properties shows the entry details | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Permissions… keeps checkboxes and octal in sync and applies chmod | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Permissions… shows chmod errors and rejects invalid octal values | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | opens a file with the default text editor or program | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | opens a file with the text editor chosen in Settings | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Open with… opens the file with the chosen program | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | Compare file with… compares with the chosen local file and shows errors | SFTP-3 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | reads the status over the SSH session and colours files and folders | TREE-1, TREE-3 | ui | git over SSH |
| ts | src/features/sftp/SftpPanel.test.tsx | reads and shows nothing when the setting is off | TREE-3 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | shows an icon for each entry's type at its left, keeping the icon's colour when git colours the name | TREE-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | expands and collapses a folder in place with a chevron, indenting its children | TREE-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | loads a folder's children once, when it is first expanded, with a loading and an error state | TREE-2 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | keeps the open folders when the list is refreshed | TREE-2 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | moves with the keyboard: Right opens and enters, Left leaves and closes, Up and Down move | TREE-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | shows a dot on folders holding changes and the letter on the file | TREE-3 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | keeps the entry actions on nested entries: renaming stays in the entry's own folder | TREE-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | offers Open in terminal right after Open on folders, never on files | SFTP-1a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | keeps the plain list: generic icons, double-click enters folders, a click does not expand, no git call | SFTP-1, DEV-2 | ui | non-DEV plain list |
| ts | src/features/sftp/SftpPanel.test.tsx | shows its actions as icons named by their tooltips | VIEW-2a | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | views a file from its menu; folders have no View item | VIEW-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | views a double-clicked file in DEV mode, and opens it as before outside it | VIEW-1, DEV-2 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | collapses the open folders and lists again with its icons, only in DEV mode | TREE-2 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | asks before moving an entry of its session, then moves it and lists the folder again | TREE-1 | ui |  |
| ts | src/features/sftp/SftpPanel.test.tsx | moves nothing when cancelled or when the entry comes from elsewhere | TREE-1 | ui |  |
| ts | src/features/sftp/osc7.test.ts | extracts the path from file://host/path | TERM-15, SFTP-4 | ported |  |
| ts | src/features/sftp/osc7.test.ts | accepts an empty host | TERM-15 | ported |  |
| ts | src/features/sftp/osc7.test.ts | percent-decodes the path | TERM-15 | ported |  |
| ts | src/features/sftp/osc7.test.ts | rejects anything that is not a file URL with a path | TERM-15 | ported |  |
| ts | src/features/sftp/osc7.test.ts | rejects malformed percent escapes | TERM-15 | ported |  |
| ts | src/features/sftp/osc7.test.ts | keeps a Windows drive path without the leading slash | TERM-15 | ported |  |
| ts | src/features/sftp/osc7.test.ts | reads the Windows path, with or without quotes | TERM-15 | ported | OSC 9;9 |
| ts | src/features/sftp/osc7.test.ts | ignores other OSC 9 payloads and empty paths | TERM-15 | ported |  |
| ts | src/features/sftp/permissions.test.ts | shows the rwx triplets | SFTP-1a | ported |  |
| ts | src/features/sftp/permissions.test.ts | shows setuid, setgid and sticky in the execute positions | SFTP-1a | ported |  |
| ts | src/features/sftp/permissions.test.ts | uses four octal digits | SFTP-1a | ported |  |
| ts | src/features/sftp/permissions.test.ts | accepts one to four octal digits | SFTP-1a | ported |  |
| ts | src/features/sftp/permissions.test.ts | rejects anything else | SFTP-1a | ported |  |
| ts | src/features/sftp/remotePath.test.ts | joins a directory and a name with one separator | SFTP-1 | ported |  |
| ts | src/features/sftp/remotePath.test.ts | returns the containing folder | SFTP-1 | ported |  |
| ts | src/features/sftp/remotePath.test.ts | keeps the root as its own parent | SFTP-1 | ported |  |
| ts | src/features/sftp/remotePath.test.ts | returns the last segment | SFTP-1 | ported |  |
| ts | src/features/sftp/remotePath.test.ts | leaves plain paths unquoted | SFTP-1a | ported | shell quoting |
| ts | src/features/sftp/remotePath.test.ts | single-quotes other paths and escapes single quotes | SFTP-1a | ported |  |
| ts | src/features/sftp/remotePath.test.ts | refuses names with control characters, which a shell could run as commands | DROP-1 | ported | NFR-6 §10.1h |
| ts | src/features/sftp/remotePath.test.ts | quotes every other name like shellQuote | DROP-1 | ported |  |
| ts | src/features/viewer/App.viewer.test.tsx | opens an Explorer file at the left of the terminal, and shows the next file in the same viewer | PANE-2, VIEW-7 | ui |  |
| ts | src/features/viewer/App.viewer.test.tsx | reads files of the SFTP browser through the tab's session | VIEW-2 | ui |  |
| ts | src/features/viewer/App.viewer.test.tsx | shows the file's new text when its terminal prints while it is open | VIEW-2 | ui |  |
| ts | src/features/viewer/App.viewer.test.tsx | asks before the × closes it; Cancel keeps the edits, Discard closes it | VIEW-2 | ui | close guard |
| ts | src/features/viewer/App.viewer.test.tsx | asks before another file replaces it and before its tab closes | VIEW-2 | ui |  |
| ts | src/features/viewer/App.viewer.test.tsx | closes without asking when nothing is unsaved | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | shows the file read-only, with line numbers and long lines wrapped | VIEW-1 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | highlights the language its name says, and leaves unknown files plain | VIEW-1 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | says when only the start of a long file is shown, and why a file cannot be shown | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | closes with its button | VIEW-1 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | reads the file again when it is given another one | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | shows a Markdown file rendered, with a switch to its code | VIEW-3 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | offers no switch for other files | VIEW-3 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | opens web links in the system browser and never navigates the window | VIEW-3 | ui | never navigates: webview part dropped |
| ts | src/features/viewer/FileViewer.test.tsx | ignores broken web links and middle clicks on links | VIEW-3 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | reads the file's own images next to it, keeps web ones, and lets the blob addresses go when closed | VIEW-3a | ui | blob URL release is webview-only |
| ts | src/features/viewer/FileViewer.test.tsx | reads the file again when the terminal prints, at most once a second, keeping the same view | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | never reads the file again for the echo of what is typed | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | reads it again when the terminal's prompt comes back, and keeps the text when a read fails | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | edits the file and saves it with the header button, over the text that was opened | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | saves with Ctrl+S | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | keeps the edits and says why when saving fails | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | does not replace unsaved edits when the file is read again | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | saves by itself shortly after typing stops when auto-save is on | VIEW-2 | ui | auto-save |
| ts | src/features/viewer/FileViewer.test.tsx | keeps the file's own line endings (CRLF), and undoing every edit leaves nothing to save | VIEW-2, VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | leaves the text and the cursor of a CRLF file alone once it is saved | VIEW-2, VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | keeps a lone LF in a CRLF file and a lone CR in an LF file | VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | keeps a file shown only in part read-only, and has no save button without a way to save | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | marks added, modified and deleted lines in the gutter, as VS Code does | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | turns the marks on and off with its button next to Save | VIEW-2a | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | marks nothing for a file outside git, and has no button without a commit to compare with | VIEW-2, VIEW-2a | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | marks the lines again as the text is edited | VIEW-2 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | shows the file's encoding, line endings and language | VIEW-4 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | highlights the text with the language chosen in the list, which a filter narrows | VIEW-4 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | converts the line endings, which is an edit to save | VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | offers no line ending change for a file shown only in part | VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | offers no line ending change while a Markdown file shows rendered | VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | reopens the file with another encoding | VIEW-5 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | saves the file with another encoding, and keeps it for the next saves | VIEW-5 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | keeps a clean file clean when it is read again with other line endings | VIEW-6 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | keeps the shown text when reopening with an encoding fails, and reads it as before afterwards | VIEW-5 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | highlights the last language chosen, even when an earlier one loads later | VIEW-4 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | saves with the encoding chosen while another save runs, even after more typing | VIEW-5 | ui |  |
| ts | src/features/viewer/FileViewer.test.tsx | closes a list with Escape | VIEW-4 | ui |  |
| ts | src/features/viewer/changedLines.test.ts | finds nothing when the file is as committed | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | marks added lines | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | marks changed lines as modified, each one on its own | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | marks where lines were deleted on the line after them, or the last line at the end | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | marks a replaced block as modified, the lines beyond the old ones as added | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | reads Windows line breaks as the same lines | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | marks every line of a new or emptied file | VIEW-2 | ported |  |
| ts | src/features/viewer/changedLines.test.ts | still answers for large files with many changes | VIEW-2 | ported |  |
| ts | src/features/viewer/markdown.test.ts | renders GitHub-flavoured Markdown | VIEW-3 | ported | GFM |
| ts | src/features/viewer/markdown.test.ts | removes scripts, event handlers, javascript: links, frames, forms and styles | VIEW-3, VIEW-3a | ported | inline HTML sanitised, not disabled |
| ts | src/features/viewer/markdown.test.ts | tells Markdown files by their name | VIEW-3 | ported |  |
| ts | src/features/viewer/markdown.test.ts | keeps the file's element ids from clobbering the window's globals | n/a | dropped | DOM clobbering of window globals: webview-only |
| ts | src/features/viewer/markdown.test.ts | points a Markdown file's own images at its folder, keeps web and inline ones, and drops the rest | VIEW-3a | ported |  |
| ts | src/features/viewer/markdown.test.ts | joins images to a Windows folder with its own separator | VIEW-3a | ported |  |
| ts | src/features/viewer/markdown.test.ts | names each image type for the browser | n/a | dropped | MIME type for blob URLs: webview-only |
| ts | src/features/viewer/markdown.test.ts | never trusts an image path the file writes itself | VIEW-3a | ported | security §10.1h |
| ts | src/features/viewer/markdown.test.ts | shows the image of a <picture> source that matches, as GitHub does for light and dark logos | VIEW-3a | ported |  |
| ts | src/features/viewer/markdown.test.ts | keeps web sources of a <picture>, and drops srcset anywhere else | VIEW-3a | ported |  |
| ts | src/features/viewer/markdown.test.ts | drops the sources of a <picture> with no image of its own | VIEW-3a | ported |  |
| rs | src-tauri/src/git.rs | reads_the_branch_of_a_repository | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | finds_the_repository_from_a_nested_folder | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | a_detached_head_shows_the_short_commit | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | follows_a_git_file_of_a_worktree_or_submodule | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | no_repository_is_none | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | an_unreadable_or_garbage_head_is_none_not_an_error | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | the_remote_command_quotes_the_folder_and_always_succeeds | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | the_remote_command_runs_in_sh | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | parses_the_remote_output | DEV-3 | ported |  |
| rs | src-tauri/src/git.rs | repo_of_names_the_repository_its_branch_and_whether_it_has_changes | TREE-4 | ported |  |
| rs | src-tauri/src/git.rs | maps_the_xy_codes_like_vs_code | TREE-1, TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | parses_renames_prefixes_and_drops_a_cut_record | TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | caps_the_number_of_entries | TREE-3 | ported | 10 000 entries |
| rs | src-tauri/src/git.rs | reads_modified_staged_untracked_ignored_deleted_and_renamed_files | TREE-1, TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | a_nested_folder_gets_paths_relative_to_it_and_only_its_own | TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | reads_a_conflict | TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | head_text_is_the_committed_text_of_a_tracked_file | VIEW-2 | ported | HEAD text for gutter |
| rs | src-tauri/src/git.rs | outside_a_repository_or_without_the_folder_is_empty | TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | status_never_runs_the_repository_fsmonitor_command | TREE-3 | ported |  |
| rs | src-tauri/src/git.rs | the_remote_status_command_quotes_the_folder | TREE-3 | ported | §10.1h |
| rs | src-tauri/src/git.rs | the_login_shell_only_sees_base64_letters | TREE-3 | ported | §10.1h |
| rs | src-tauri/src/git.rs | a_hostile_folder_name_runs_nothing_when_the_login_shell_is_fish | TREE-3 | ported | §10.1h |
| rs | src-tauri/src/git.rs | the_remote_status_command_runs_in_sh_and_a_hostile_name_runs_nothing | TREE-3 | ported | §10.1h |
| rs | src-tauri/src/localfs.rs | lists_folders_first_then_by_case_insensitive_name | LOC-1 | ported |  |
| rs | src-tauri/src/localfs.rs | listing_a_missing_folder_names_it | LOC-1 | ported |  |
| rs | src-tauri/src/localfs.rs | stat_reports_a_folder_and_a_file | LOC-1 | ported |  |
| rs | src-tauri/src/localfs.rs | a_symlink_is_reported_and_removed_without_its_target | LOC-2 | ported |  |
| rs | src-tauri/src/localfs.rs | a_trailing_slash_never_deletes_through_a_symlink | LOC-2 | ported |  |
| rs | src-tauri/src/localfs.rs | creates_renames_and_removes | LOC-1, SFTP-1a | ported |  |
| rs | src-tauri/src/localfs.rs | reads_text_files_up_to_the_limit_and_refuses_binary_ones | DROP-1 | ported | AI attachments 64 KB |
| rs | src-tauri/src/localfs.rs | reads_images_for_the_markdown_preview_and_refuses_other_files | VIEW-3a | ported |  |
| rs | src-tauri/src/localfs.rs | an_image_name_on_something_that_is_not_a_file_is_not_read | VIEW-3a | ported |  |
| rs | src-tauri/src/localfs.rs | views_text_files_up_to_the_view_limit_and_refuses_folders_and_binary_ones | VIEW-2 | ported |  |
| rs | src-tauri/src/localfs.rs | a_folder_is_attached_as_its_listing | DROP-1 | ported |  |
| rs | src-tauri/src/localfs.rs | write_text_saves_over_the_text_that_was_opened | VIEW-2, FIX-1 | ported |  |
| rs | src-tauri/src/localfs.rs | write_text_refuses_a_file_that_changed_since_it_was_opened | VIEW-2 | ported |  |
| rs | src-tauri/src/localfs.rs | write_text_refuses_a_missing_file | VIEW-2 | ported |  |
| rs | src-tauri/src/localfs.rs | write_text_keeps_a_symlink_and_the_file_mode | FIX-1 | ported | atomic save must keep symlink + mode |
| rs | src-tauri/src/localfs.rs | viewing_plain_utf8_reports_utf8_as_before | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | viewing_detects_a_bom_and_leaves_it_out | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | viewing_with_an_encoding_decodes_with_it | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | viewing_refuses_unknown_encodings_and_malformed_text | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | a_character_cut_by_the_view_limit_is_dropped_in_any_encoding | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | writing_keeps_the_encoding_and_its_bom | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | writing_with_another_encoding_compares_with_the_one_it_was_opened_with | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | writing_refuses_characters_the_encoding_cannot_hold | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | writing_with_an_encoding_still_refuses_a_file_that_changed | VIEW-5 | ported |  |
| rs | src-tauri/src/localfs.rs | move_into_moves_a_file_and_a_folder_into_a_folder | TREE-1, FIX-2 | ported |  |
| rs | src-tauri/src/localfs.rs | move_into_never_replaces_an_entry_with_the_same_name | TREE-1, FIX-2 | ported |  |
| rs | src-tauri/src/localfs.rs | move_into_refuses_a_folder_into_itself_or_its_subfolders | TREE-1 | ported |  |
| rs | src-tauri/src/localfs.rs | move_into_its_own_folder_is_refused | TREE-1 | ported |  |
| rs | src-tauri/src/localfs.rs | chmod_sets_the_mode_and_rejects_invalid_ones | SFTP-1a, LOC-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | home_is_the_canonical_working_directory | SFTP-4 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | the_sftp_session_is_opened_once_and_reused | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | list_puts_directories_first_then_names_case_insensitively | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | entries_serialize_in_camel_case | n/a | dropped | camelCase serde for Tauri IPC |
| rs | src-tauri/src/sftp/tests.rs | upload_then_download_round_trips_a_large_binary_file | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | upload_into_the_root_does_not_double_the_slash | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | a_failed_download_keeps_the_existing_local_file_and_leaves_no_temp_files | SFTP-1b | ported |  |
| rs | src-tauri/src/sftp/tests.rs | create_file_makes_an_empty_file_and_never_replaces_one | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | mkdir_then_remove_an_empty_directory | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | removing_a_non_empty_directory_fails_and_keeps_it | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | remove_deletes_a_file | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | rename_moves_an_entry | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | read_image_reads_remote_images_only | VIEW-3a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | write_text_saves_over_the_text_that_was_opened | VIEW-2 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | write_text_refuses_a_file_that_changed_since_it_was_opened | VIEW-2 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | move_into_moves_an_entry_into_a_folder | TREE-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | move_into_never_replaces_an_entry_with_the_same_name | TREE-1, FIX-2 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | move_into_refuses_a_folder_into_itself_or_its_subfolders | TREE-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | listing_a_missing_path_is_an_sftp_error | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | sftp_on_an_unknown_session_is_an_unknown_session_error | SFTP-1 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | list_entries_carry_the_owner_ids | SFTP-1a | ported | owner/group in Properties |
| rs | src-tauri/src/sftp/tests.rs | chmod_sets_the_permission_bits | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | chmod_rejects_modes_above_0o7777_and_keeps_the_file | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | chmod_of_a_missing_path_is_an_sftp_error | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | edited_copy_is_uploaded_back_to_the_same_remote_path | SFTP-2 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | copies_live_in_a_private_folder_one_subfolder_per_remote_file | SFTP-2 | ported | §10.1e |
| rs | src-tauri/src/sftp/tests.rs | removing_a_session_deletes_its_copies | SFTP-2 | ported | §10.1e |
| rs | src-tauri/src/sftp/tests.rs | the_watcher_reports_each_save_and_stops_with_the_session | SFTP-2 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | desktop_exec_puts_the_file_in_place_of_its_field_code | SFTP-1a | ported | Open with default program (Linux .desktop) |
| rs | src-tauri/src/sftp/tests.rs | the_chosen_text_editor_gets_the_file_as_its_own_argument | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | an_existing_program_path_with_spaces_is_one_program | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | open_with_text_editor_takes_an_optional_command | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | desktop_exec_is_read_from_the_desktop_entry_group | SFTP-1a | ported |  |
| rs | src-tauri/src/sftp/tests.rs | the_first_installed_diff_tool_wins_in_order | SFTP-3 | ported |  |
| rs | src-tauri/src/sftp/tests.rs | copy_names_never_leave_the_private_folder | SFTP-2 | ported | §10.1e/h |
| rs | src-tauri/src/sftp/tests.rs | a_compare_target_must_be_an_absolute_local_path | SFTP-3 | ported | §10.1h |

## 4. AI, DEV mode, macros and tools

| Kind | File | Test | Req | Disposition | Note |
| --- | --- | --- | --- | --- | --- |
| ts | src/features/ai/App.ai.test.tsx | opens with the shortcut as a pane below the focused terminal, without restarting it, focused on its input | AI-3, AI-2, PANE-2 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | splits the focused terminal, not another one | AI-2 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | follows a changed assistant shortcut | AI-3, KEY-5 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | opens from the X button, whose tooltip names the shortcut | AI-3 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | the X closes the open chat with a spin, whether the chat or its terminal has the focus | AI-4 | ui | spin: see W2 |
| ts | src/features/ai/App.ai.test.tsx | focuses the terminal's existing chat instead of opening another, and from the chat goes back to the terminal | AI-4 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | says so when the tab is full instead of opening a chat | AI-4 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | spins the X once on every open, restarting it on a new trigger | AI-6a | ui | spin on open/focus, not per reply (W2) |
| ts | src/features/ai/App.ai.test.tsx | does not spin the X when the system asks for reduced motion | SHAPE-1, AI-6a | ui |  |
| ts | src/features/ai/App.ai.test.tsx | shows the provider and model in its header | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | switches provider and model from the header's model list, for the next question (S10) | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | asks with the context and inserts the answer's command into the linked terminal without Enter | AI-6, AI-7 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | keeps the conversation when the chat is closed and opened again, in the same AI session | AI-8, AI-1b | ui |  |
| ts | src/features/ai/App.ai.test.tsx | New conversation clears the chat and starts another AI session | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | forgets a terminal's conversation when the terminal closes | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | sends the earlier turns with a follow-up question, in one AI session per chat pane | AI-2, AI-1b | ui |  |
| ts | src/features/ai/App.ai.test.tsx | Run sends the command with Enter to the linked terminal only, or says why it could not | AI-6 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | shows a general answer with no command box, Insert or Copy when no command fits (S8) | AI-6 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | renders fenced code in an answer as monospace blocks and never as HTML | AI-6a | ui | HTML half is moot natively; monospace block stays |
| ts | src/features/ai/App.ai.test.tsx | sends no history when command history is off, and Shift+Enter does not send | AI-7, AI-5, TERM-12b | ui |  |
| ts | src/features/ai/App.ai.test.tsx | attaches the terminal selection to the first question as a chip shown before sending | AI-7 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | attaches files dropped on the chat to the next question only, and refuses binary ones (S6) | DROP-1 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | attaches at most five files to one question (S6) | DROP-1 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | Ask the AI from a terminal's menu opens its chat with the selection, or puts it in the open chat (S7) | AI-3, TERM-6 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | sends an attached selection alone, asking to explain it (L13) | AI-5 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | lists the conversations, reopens one to continue it, deletes one, and saves them (S9) | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | deleting the open conversation starts a new one, so it is not saved again (S9) | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | saves no conversation when Settings say not to keep them (S9) | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | sends no selection once its chip is removed, and cuts it to 2000 characters | AI-7 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | shows no chip without a selection | AI-7 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | says when no key is saved and opens Settings on the AI section from there | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | Stop cancels the request in flight and puts the question back; errors show in their turn | AI-5 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | is never a multi-execution target and gets no exclude toggle | MEXEC-1 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | closes with Ctrl+Shift+W or the fixed × in its header (no hover × in its corner), leaving the terminal running | AI-8, KEY-4 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | closes with its terminal | AI-8 | ui |  |
| ts | src/features/ai/App.ai.test.tsx | is not offered to the panes while AI suggestions are off | TERM-12c | ui |  |
| ts | src/features/ai/App.ai.test.tsx | asks the provider with only the line, OS, folder and history, returning the whole line | TERM-12c, AI-7a | ui |  |
| ts | src/features/ai/App.ai.test.tsx | keeps one AI session per pane for its inline suggestions | AI-1b | ui |  |
| ts | src/features/ai/App.ai.test.tsx | gives null for an empty completion | TERM-12c | ported | suggestion view-model |
| ts | src/features/ai/App.ai.test.tsx | keeps one request in flight per pane and spaces them out | TERM-12c | ported | limiter service |
| ts | src/features/ai/App.ai.test.tsx | is silent on errors except a small hint in the pane, cleared by the next answer | TERM-12c | ui |  |
| ts | src/features/ai/App.ai.test.tsx | sends the default shell's name with inline suggestions and chat questions | AI-7a | ported |  |
| ts | src/features/ai/App.ai.test.tsx | sends the shell chosen in Settings | AI-7a | ported |  |
| ts | src/features/ai/ModelPicker.test.tsx | shows the current provider and model, closed | AI-8 | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | lists the models of every provider with a saved key, each naming its provider | AI-8 | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | asks the OpenAI-compatible server only when its base URL is set | AI-8, AI-1b | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | leaves out an OpenAI-compatible server on opencode.ai that has no key of its own | AI-1b | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | lists an OpenAI-compatible server on opencode.ai once it has its own key | AI-1b | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | choosing a model selects it with its provider and closes | AI-8 | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | does not select a model PlexusTerm cannot use yet | AI-1 | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | shows a provider's error without hiding the other providers | AI-8 | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | filters by name, id or provider and closes with Escape | AI-8 | ui |  |
| ts | src/features/ai/ModelPicker.test.tsx | says when no provider has a key | AI-8 | ui |  |
| ts | src/features/ai/aiClient.test.ts | asks whether a key is saved without ever receiving it | AI-1, DATA-3 | ported | service exposes has_key only |
| ts | src/features/ai/aiClient.test.ts | saves and deletes a provider key | AI-1, DATA-3 | ported |  |
| ts | src/features/ai/aiClient.test.ts | requests a completion with the config and only the given context | TERM-12c, AI-7 | ported |  |
| ts | src/features/ai/aiClient.test.ts | requests an assistant answer with an optional command | AI-6 | ported |  |
| ts | src/features/ai/aiClient.test.ts | passes backend errors through unchanged | n/a | dropped | IPC pass-through; native calls the service directly |
| ts | src/features/ai/aiClient.test.ts | uses the backend message of a typed error | AI-9 | ported |  |
| ts | src/features/ai/aiClient.test.ts | falls back to a per-kind text when the message is empty | AI-9 | ported |  |
| ts | src/features/ai/aiClient.test.ts | stringifies anything that is not a typed error | n/a | dropped | JS unknown-error shape; Rust errors are typed |
| ts | src/features/ai/aiContext.test.ts | sends the base URL only for an OpenAI-compatible endpoint | AI-1 | ported |  |
| ts | src/features/ai/aiContext.test.ts | names the reason in a few words | TERM-12c | ported | hint text |
| ts | src/features/ai/aiContext.test.ts | is true for every provider but an OpenAI-compatible endpoint (local servers need none) | AI-1 | ported |  |
| ts | src/features/ai/aiContext.test.ts | keeps short selections and cuts long ones | AI-7 | ported |  |
| ts | src/features/ai/aiLimiter.test.ts | keeps at most one request in flight per pane, sending the newest waiting one next | AI-2, TERM-12c | ported |  |
| ts | src/features/ai/aiLimiter.test.ts | waits the minimum interval between two requests of a pane, but not across panes | AI-2, TERM-12c | ported |  |
| ts | src/features/ai/aiLimiter.test.ts | passes errors on and keeps serving the pane | AI-2, TERM-12c | ported |  |
| ts | src/features/ai/chatHistory.test.ts | keeps answered conversations newest first, titled by their first question, and saves them | AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | never keeps questions still waiting, nor empty conversations | AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | reads the saved conversations and drops the oldest beyond the limit | AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | deletes a conversation | AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | neither reads nor writes the file when saving is off | AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | keeps earlier runs' conversations when saving is turned on later | AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | does not overwrite a file it could not read | DATA-1a, AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | does not overwrite an ai-chats.json it cannot parse | DATA-1a, AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | does not overwrite an ai-chats.json of another version | DATA-1a, AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | keeps entries it does not understand when it saves | DATA-1a, AI-8 | ported |  |
| ts | src/features/ai/chatHistory.test.ts | ignores a damaged file and invalid entries | DATA-1a, AI-8 | ported |  |
| ts | src/features/devmode/App.devMode.test.tsx | leaves the interface as before while no tab is in DEV mode | DEV-2 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | shows only the DEV option, without the bar, under a terminal with another one below it | PANE-3a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | turned on in one pane of a split tab, shows only that terminal until it is turned off | DEV-2a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | lists the hidden terminals in the column and shows the one clicked | DEV-2a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | keeps the whole bar under the terminal on screen when DEV mode hides the one below it | PANE-3a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | turns on for each listed CLI and not for other commands | DEV-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | follows the commands listed in Settings | DEV-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | turns on from a CLI's terminal title when the command was not seen | DEV-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | turns on by title only when a CLI's title appears, so turning it off by hand sticks | DEV-1a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | shows Claude Code's icon in a DEV mode on by hand when the DEV commands leave claude out | DEV-1a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | shows the icon of a CLI started before DEV mode was turned on by hand, after it retitled | DEV-1a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | knows no CLI when DEV mode is turned on after the CLI ended | DEV-1a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | shows the DEV look while on: tab mark, right column with the CLI's icon, branch and VS Code-style lists | DEV-2, DEV-3, TAB-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | turns off when the shell's prompt comes back, not on quiet output inside the CLI | DEV-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | is turned on and off with the button, and a hand-made DEV mode survives the prompt | DEV-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | shows the CLI's icon in the column once a CLI starts in a DEV tab turned on by hand | DEV-1a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | learns the running CLI again when DEV mode is turned back on by hand while the CLI runs | DEV-1a | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | opens the column's new terminal in DEV mode, in the default folder; the + on top still opens a plain one | DEV-2 | ui | column + starts in localCwd, not the project |
| ts | src/features/devmode/App.devMode.test.tsx | marks a tab as working while its CLI prints, and only while a CLI runs there | DEV-2 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | duplicates a DEV tab into a new DEV tab, and a plain tab into a plain one | DEV-2 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | keeps the folders added to a DEV tab's workspace with that tab | WS-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | opens a folder in a new DEV terminal with Open in terminal while in DEV mode | DEV-2 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | remembers the folders DEV mode was used in and opens one in a new DEV terminal from the Explorer | DEV-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | names the terminal's folder first in the bar under it, before the branch, only in DEV mode and while Settings allow it | PANE-3b | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | keeps only as many recent folders as Settings ask | DEV-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | folds the DEV tab column to its icons from the button next to Settings, shown only in DEV mode | DEV-2b | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | hides the folded DEV tab column completely when Settings ask, and opens folded as saved | DEV-2b | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | opens a workspace file in a new DEV tab: the terminal in its first folder, the others added | WS-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | starts in DEV mode with the workspace file PlexusTerm was opened with, instead of the usual terminal | WS-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | starts with the usual terminal when the workspace file it was opened with lists no folders | WS-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | opens a workspace file given to a second launch as a new DEV tab of the running window | WS-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | lists the workspace files opened last above the recent folders and opens the one chosen again | DEV-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | saves a DEV tab's colour in its project's .plexusterm folder, and outside DEV mode nowhere | DEV-2, UI-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | gives a tab and the toolbar the colours its project saved when it goes into DEV mode | DEV-2, UI-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | saves the toolbar colour in the active DEV tab's project, in the same file as the tab's | DEV-2, UI-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | is toggled by its shortcut once one is set | KEY-1 | ui | toggleDevMode |
| ts | src/features/devmode/App.devMode.test.tsx | lists the recent folders with its shortcut once one is set, and opens the one chosen | DEV-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | folds and opens the DEV tab column with its shortcut once one is set in Settings | DEV-2b | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | opens the Explorer at the folder the tool was opened in and follows the terminal while Follow terminal folder is on | DEV-2, SFTP-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | keeps the Explorer at the folder the tool was opened in while Follow terminal folder is off | DEV-2, SFTP-4 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | sends desktop notifications for a DEV tab even when they are off in Settings | TERM-9 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | styles the DEV button as plain text pushed to the right edge, a pill only when on | PANE-3, COMP-1 | ui |  |
| ts | src/features/devmode/App.devMode.test.tsx | remembers the width the tab column is dragged to, and starts with it | DEV-2 | ui | tabColumnWidth persisted |
| ts | src/features/devmode/CliIcon.test.tsx | draws each known CLI with its own mark and name | DEV-2 | ui |  |
| ts | src/features/devmode/CliIcon.test.tsx | draws a terminal for any other program or none | DEV-2 | ui |  |
| ts | src/features/devmode/CliIcon.test.tsx | is the official favicon's arch, without its background, with ids of its own in every copy | DEV-2, VIS-1 | manual | icon shape sign-off; SVG id uniqueness is DOM-only |
| ts | src/features/devmode/CliIcon.test.tsx | is the three blocks of pi.dev's favicon, in the text colour | DEV-2, VIS-1 | manual | icon shape sign-off |
| ts | src/features/devmode/CliIcon.test.tsx | is opencode.ai's icon, its frame and lower block, without its background, in the text colour | DEV-2, VIS-1 | manual | icon shape sign-off |
| ts | src/features/devmode/CliIcon.test.tsx | is gentle-ai's rose, traced to two paths, named gentle-shell | DEV-2, VIS-1 | manual | icon shape sign-off |
| ts | src/features/devmode/devMode.test.ts | turns on for each listed command run as the first word | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | accepts a path to the program | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | ignores other commands and the names further in the line | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | follows the edited list | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | recognises a CLI's own terminal title | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | ignores shell titles and lookalikes | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | keeps names, drops blanks, duplicates and invalid entries | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | reads and writes the list as text | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | turns a tab on when one of the CLIs runs, not for other commands | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | turns it off when the shell marks its prompt in the pane that ran the CLI | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | turns it off when output goes quiet on the same prompt the CLI was started from | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | ignores an empty starting prompt for the quiet check | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | ignores commands typed inside the running CLI | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | turns on by title when nothing turned it on | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | learns the CLI from its title in a hand-made DEV mode, keeping it manual | DEV-1a | ported |  |
| ts | src/features/devmode/devMode.test.ts | knows the default CLIs in a DEV mode already on, even when the list leaves them out | DEV-1a | ported |  |
| ts | src/features/devmode/devMode.test.ts | ends the CLI when the title stops being a CLI's: off when automatic, back to no CLI when by hand | DEV-1, DEV-1a | ported |  |
| ts | src/features/devmode/devMode.test.ts | toggles by hand, and a hand-made DEV mode survives the prompt coming back | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | keeps the panes a hand-made DEV mode hides, and none otherwise | DEV-2a | ported |  |
| ts | src/features/devmode/devMode.test.ts | shows another of its terminals, hiding the one shown | DEV-2a | ported |  |
| ts | src/features/devmode/devMode.test.ts | turns an automatic DEV mode off by hand | DEV-1 | ported |  |
| ts | src/features/devmode/devMode.test.ts | records the project folder of the DEV mode it was looked up for only | DEV-2 | ported | anchor = project folder of the row |
| ts | src/features/devmode/devMode.test.ts | forgets closed tabs and automatic DEV modes whose pane closed | DEV-1 | ported |  |
| ts | src/features/devmode/recentFolders.test.ts | puts the latest folder first, once, and keeps the last ten by default | DEV-4 | ported |  |
| ts | src/features/devmode/recentFolders.test.ts | keeps as many folders as asked | DEV-4 | ported |  |
| ts | src/features/devmode/recentFolders.test.ts | returns the same list when the folder is already first | DEV-4 | ported |  |
| ts | src/features/devmode/recentFolders.test.ts | survives a restart in local storage and ignores a damaged value | DEV-4 | ported | localStorage → config file (DATA-6) |
| ts | src/features/devmode/recentFolders.test.ts | keeps the recent workspace files apart from the folders | DEV-4 | ported |  |
| ts | src/features/licenses/LicensesDialog.test.tsx | lists every shipped library with its version and license, loaded when opened | UI-7 | ui |  |
| ts | src/features/licenses/LicensesDialog.test.tsx | filters by name or license | UI-7 | ui |  |
| ts | src/features/licenses/LicensesDialog.test.tsx | shows a library's license text and its source, opened in the browser | UI-7 | ui |  |
| ts | src/features/licenses/LicensesDialog.test.tsx | closes with its button and with Escape | UI-7 | ui |  |
| ts | src/features/licenses/licenses.test.ts | lists every npm production dependency of package.json | n/a | dropped | no npm dependencies in the native build |
| ts | src/features/licenses/licenses.test.ts | lists every Rust dependency of Cargo.toml | UI-7 | ported | every workspace crate |
| ts | src/features/licenses/licenses.test.ts | gives every package its declared license and at least one license text | UI-7 | ported |  |
| ts | src/features/macros/App.macros.test.tsx | records typing in the focused pane and saves it under a name (S16.2) | MAC-1 | ui |  |
| ts | src/features/macros/App.macros.test.tsx | plays the hotkey's macro through the input path, multi-execution included (S16.3, S16.4) | MAC-1, MEXEC-1 | ui |  |
| ts | src/features/macros/App.macros.test.tsx | never plays a macro into an AI chat pane (S9) | MAC-1 | ui |  |
| ts | src/features/macros/App.macros.test.tsx | shows progress, stops on Escape, and reports a waitFor timeout in the pane (S16.3) | MAC-1, UI-2 | ui |  |
| ts | src/features/macros/MacroBars.test.tsx | warns that everything typed is saved and stops recording | MAC-1, UI-2 | ui |  |
| ts | src/features/macros/MacroBars.test.tsx | saves the recording under a name, or discards it | MAC-1, UI-2 | ui |  |
| ts | src/features/macros/MacroBars.test.tsx | shows the running step and stops the macro | MAC-1, UI-2 | ui |  |
| ts | src/features/macros/MacroEditDialog.test.tsx | edits name, hotkey and steps: add, edit, reorder, remove | MAC-1 | ui |  |
| ts | src/features/macros/MacroEditDialog.test.tsx | shows validation errors instead of saving | MAC-1 | ui |  |
| ts | src/features/macros/MacroEditDialog.test.tsx | cancels | MAC-1 | ui |  |
| ts | src/features/macros/MacrosPanel.test.tsx | lists macros with their hotkey and filters them by name | MAC-1, UI-3 | ui |  |
| ts | src/features/macros/MacrosPanel.test.tsx | plays, edits, duplicates and deletes a macro | MAC-1 | ui |  |
| ts | src/features/macros/MacrosPanel.test.tsx | disables playing without a terminal to play into | MAC-1a | ui |  |
| ts | src/features/macros/MacrosPanel.test.tsx | disables editing when the file could not be read | MAC-1 | ui |  |
| ts | src/features/macros/MacrosPanel.test.tsx | names the default recording shortcut | MAC-1, KEY-1 | ui |  |
| ts | src/features/macros/MacrosPanel.test.tsx | names the user's recording shortcut, or the button when there is none | KEY-1 | ui |  |
| ts | src/features/macros/macroForm.test.ts | round-trips a macro | MAC-1 | ported |  |
| ts | src/features/macros/macroForm.test.ts | new rows are empty sends, and a new waitFor defaults to 10 seconds | MAC-1 | ported |  |
| ts | src/features/macros/macroForm.test.ts | requires a name and valid numbers | MAC-1 | ported |  |
| ts | src/features/macros/macroForm.test.ts | moves rows up and down, ignoring moves past the ends | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | turns \\r \\n \\t \\e \\\\ and \\xHH into characters | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | turns ^A..^Z, ^[ and ^? into control characters | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | leaves invalid escapes literal | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | shows control characters as escapes | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | escapes a caret that would otherwise read as a control character | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | round-trips every ASCII character and non-ASCII text | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | reads a missing file as an empty list | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | round-trips every step kind and the hotkey | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | never silently empties a corrupt file | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | rejects values out of range | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | rejects two macros with the same hotkey | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | writes only the known fields | DATA-1a, MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | adds and replaces macros on save | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | deletes a macro | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | duplicates a macro right after the original, without its hotkey | MAC-1a | ported |  |
| ts | src/features/macros/macros.test.ts | moves a hotkey that another macro used | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | labels hotkeys | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | merges consecutive keystrokes and starts a new send step after Enter | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | records a paste as one chunk, merged like typing | MAC-1 | ported |  |
| ts | src/features/macros/macros.test.ts | ignores empty input | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | sends steps with their escapes parsed, in order, reporting progress | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | waits the given milliseconds before the next step | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | waitFor continues once the output since the step started contains the text, ANSI stripped | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | waitFor fails after its timeout with a clear message | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | stop() ends a running macro without sending the remaining steps | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | finishes at once for an empty macro | MAC-1 | ported |  |
| ts | src/features/macros/player.test.ts | returns the registered target and never removes a newer one | MAC-1 | ported | target registry detail |
| ts | src/features/macros/useMacros.test.ts | loads macros.json and saves every change | MAC-1, DATA-1a | ported | view-model |
| ts | src/features/macros/useMacros.test.ts | disables editing when the file cannot be read, never overwriting it | MAC-1, DATA-1a | ported | view-model |
| ts | src/features/tools/WakeOnLanDialog.test.tsx | sends the magic packet and confirms it | TOOL-3 | ui |  |
| ts | src/features/tools/WakeOnLanDialog.test.tsx | shows form errors without sending, and backend errors after sending | TOOL-3 | ui |  |
| ts | src/features/tools/WakeOnLanDialog.test.tsx | closes on Close and on Escape | TOOL-3 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | refuses to generate when the passphrases differ | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | generates a key, shows only its public half and copies it | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | generates the chosen type and size, RSA 3072 and ECDSA P-256 by default | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | saves the public and the private key separately, asking before overwriting | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | saves the key as a PuTTY .ppk file | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | loads a key file, asking for its passphrase until it is right | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | applies a changed comment and passphrase before saving a loaded key | TOOL-1 | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | installs the key on a saved session, asking for its password when none is saved | TOOL-1, TOOL-1a | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | asks to trust an unknown host, then installs | TOOL-1, TOOL-1a | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | shows install errors | TOOL-1, TOOL-1a | ui |  |
| ts | src/features/tools/keygen/KeyGenDialog.test.tsx | forgets the generated key when closed | TOOL-1 | ui |  |
| ts | src/features/tools/monitor/RemoteMonitorBar.test.tsx | polls the session and shows its stats, with CPU and network rates from the second sample | TOOL-4 | ui |  |
| ts | src/features/tools/monitor/RemoteMonitorBar.test.tsx | never overlaps requests: the next one waits for the previous answer | TOOL-4 | ui |  |
| ts | src/features/tools/monitor/RemoteMonitorBar.test.tsx | shows Monitoring unavailable for a non-Linux server or a failed read | TOOL-4 | ui |  |
| ts | src/features/tools/monitor/RemoteMonitorBar.test.tsx | does not poll without a session or while hidden, and stops when unmounted | TOOL-4 | ui |  |
| ts | src/features/tools/monitor/monitorStats.test.ts | shows memory, disk and users from one sample, but no CPU or network rates yet | TOOL-4 | ported |  |
| ts | src/features/tools/monitor/monitorStats.test.ts | computes CPU % and network rates from two consecutive samples | TOOL-4 | ported |  |
| ts | src/features/tools/monitor/monitorStats.test.ts | gives no rates when the counters went backwards or no time passed | TOOL-4 | ported |  |
| ts | src/features/tools/monitor/monitorStats.test.ts | formats byte sizes with binary units | TOOL-4 | ported |  |
| ts | src/features/tools/monitor/monitorStats.test.ts | formats rates and percentages, with a placeholder when unknown | TOOL-4 | ported |  |
| ts | src/features/tools/scan/NetworkScannerDialog.test.tsx | lists live hosts with name, MAC and services, offering SSH only where it is open | TOOL-2 | ui |  |
| ts | src/features/tools/scan/NetworkScannerDialog.test.tsx | shows invalid input errors from the backend | TOOL-2 | ui |  |
| ts | src/features/tools/scan/NetworkScannerDialog.test.tsx | asks for the range in separate fields, starting with 192.168.1.1 to 254 | TOOL-2 | ui |  |
| ts | src/features/tools/scan/NetworkScannerDialog.test.tsx | scans the edited fields and checks them before starting | TOOL-2 | ui |  |
| ts | src/features/tools/scan/NetworkScannerDialog.test.tsx | keeps digits only and moves to the next field after a dot or three digits | TOOL-2 | ui |  |
| ts | src/features/tools/scan/NetworkScannerDialog.test.tsx | Stop cancels the running scan | TOOL-2 | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | scans the entered hosts and ports and fills the table as open ports arrive | TOOL-2 | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | scans the known ports by default, or a range from two fields | TOOL-2 | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | checks the range before starting | TOOL-2 | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | scans the known ports when the custom list is left empty, and says so | TOOL-2, TOOL-2a | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | shows invalid input errors from the backend | TOOL-2 | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | Stop cancels the running scan | TOOL-2 | ui |  |
| ts | src/features/tools/scan/PortScannerDialog.test.tsx | closes on Close and Escape, stopping a running scan | TOOL-2, TOOL-2a | ui |  |
| ts | src/features/tools/scan/ipRange.test.ts | joins the fields into the a.b.c.start-end form the backend scans | TOOL-2 | ported |  |
| ts | src/features/tools/scan/ipRange.test.ts | scans one address when start and end are equal | TOOL-2 | ported |  |
| ts | src/features/tools/scan/ipRange.test.ts | rejects empty or out-of-range parts and a start after the end | TOOL-2 | ported |  |
| ts | src/features/tools/scan/ipRange.test.ts | fills every field from an address, a range or a /24 network | TOOL-2 | ported |  |
| ts | src/features/tools/scan/ipRange.test.ts | ignores anything else, so a plain number is typed normally | TOOL-2 | ported |  |
| ts | src/features/tools/scan/portChoice.test.ts | uses the known ports by default | TOOL-2 | ported |  |
| ts | src/features/tools/scan/portChoice.test.ts | turns the range fields into from-to, or one port when they are equal | TOOL-2 | ported |  |
| ts | src/features/tools/scan/portChoice.test.ts | rejects a range outside 1..65535 or reversed | TOOL-2 | ported |  |
| ts | src/features/tools/scan/portChoice.test.ts | passes a custom list as typed, and the known ports when it is empty | TOOL-2, TOOL-2a | ported |  |
| ts | src/features/tools/scan/scanBackend.test.ts | orders IPv4 addresses numerically and other hosts after them | TOOL-2a | ported |  |
| ts | src/features/tools/wolForm.test.ts | accepts a MAC with the default broadcast address and port | TOOL-3 | ported |  |
| ts | src/features/tools/wolForm.test.ts | keeps a given broadcast address and port | TOOL-3 | ported |  |
| ts | src/features/tools/wolForm.test.ts | rejects a malformed MAC, broadcast address or port | TOOL-3 | ported |  |
| ts | src/features/updates/App.updates.test.tsx | tells about a newer release once and opens its page from Download | UI-6 | ui |  |
| ts | src/features/updates/App.updates.test.tsx | says nothing when PlexusTerm is up to date or the check fails | UI-6 | ui |  |
| ts | src/features/updates/App.updates.test.tsx | never checks when the startup check is turned off | UI-6 | ui |  |
| rs | src-tauri/src/ai/tests.rs | anthropic_completion_request_and_answer | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | anthropic_fallbacks_only_for_claude_opus_5_5 | AI-1a | ported |  |
| rs | src-tauri/src/ai/tests.rs | anthropic_assist_request_and_answer | AI-1, AI-6 | ported |  |
| rs | src-tauri/src/ai/tests.rs | anthropic_refusal_is_refused | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | anthropic_max_tokens_gives_no_completion | TERM-12c | ported |  |
| rs | src-tauri/src/ai/tests.rs | gemini_completion_request_and_answer | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | gemini_assist_asks_for_json | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | gemini_blocked_prompt_and_safety_stop_are_refused | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | gemini_invalid_key_is_auth | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | gemini_model_names_must_be_path_safe | AI-1, SEC-h | ported |  |
| rs | src-tauri/src/ai/tests.rs | openai_completion_request_and_answer | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | openai_assist_uses_a_strict_json_schema | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | openai_refusal_is_refused | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | compatible_server_without_key_sends_no_authorization | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | compatible_server_with_key_sends_a_bearer_token | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | compatible_base_url_is_required_and_must_be_http | AI-1, AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_empty_model_is_a_config_error | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_uses_chat_completions_with_a_bearer_key_and_the_session | AI-1, AI-1b | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_assist_reads_the_json_answer | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | the_default_opencode_endpoints_are_the_go_api_and_models_dev | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_refuses_a_missing_or_invalid_session_id_before_any_request | AI-1b | ported |  |
| rs | src-tauri/src/ai/tests.rs | session_ids_of_1_to_128_safe_characters_are_accepted | AI-1b | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_compatible_server_at_opencode_ai_gets_the_session_header | AI-1b | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_compatible_server_elsewhere_gets_no_session_header | AI-1b | ported |  |
| rs | src-tauri/src/ai/tests.rs | every_provider_identifies_itself_as_plexusterm | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_parses_with_its_id_and_name | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_messages_models_get_the_anthropic_shape_with_both_key_headers | AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_responses_models_post_instructions_and_input_to_responses | AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | responses_answers_cut_off_or_refused_are_reported | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_google_models_are_not_supported_yet_and_send_nothing | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | the_opencode_go_catalogue_joins_the_public_list_with_models_dev_formats | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_fresh_cache_file_is_used_without_asking_and_refresh_or_age_fetches_again | AI-1 | ported |  |
| rs | src-tauri/src/ai/tests.rs | without_models_dev_every_listed_model_is_chat_completions_with_a_note | AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | without_the_public_list_models_dev_alone_gives_the_models | AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | when_both_sources_fail_the_saved_list_is_kept_with_a_note_or_it_is_a_network_error | AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | models_missing_from_the_catalogue_fall_back_to_chat_completions | AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | redirects_are_not_followed_and_are_a_config_error | SEC-c | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_assistant_command_over_2000_characters_is_dropped_and_the_answer_kept | AI-6 | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_answer_without_a_command_serializes_command_as_null | AI-8, DATA-1a | ported | answer shape reused by ai-chats.json |
| rs | src-tauri/src/ai/tests.rs | every_provider_reads_an_answer_without_a_command | AI-6 | ported |  |
| rs | src-tauri/src/ai/tests.rs | every_provider_sends_the_general_assistant_prompt | AI-6 | ported |  |
| rs | src-tauri/src/ai/tests.rs | plain_text_replies_become_the_answer_with_no_command | AI-6 | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_empty_assistant_reply_is_a_bad_response | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | http_401_is_auth_and_never_carries_the_key | AI-9, SEC-c | ported |  |
| rs | src-tauri/src/ai/tests.rs | http_429_is_rate_limit | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | http_400_keeps_the_provider_message_without_the_key | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | http_529_overloaded_is_network | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_slow_provider_times_out | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_unreachable_server_is_network | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_non_json_answer_is_bad_response | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | hosted_providers_without_a_key_fail_before_any_request | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_empty_line_is_not_sent | TERM-12c | ported |  |
| rs | src-tauri/src/ai/tests.rs | errors_serialize_as_kind_and_message | n/a | dropped | IPC error wire shape; native UI matches the typed error |
| rs | src-tauri/src/ai/tests.rs | config_deserializes_from_camel_case | SET-1 | ported | settings.json keys |
| rs | src-tauri/src/ai/tests.rs | the_default_client_builds_with_tls | SEC-c | ported |  |
| rs | src-tauri/src/ai/tests.rs | keys_are_saved_reported_and_deleted_in_the_keyring | DATA-3 | ported |  |
| rs | src-tauri/src/ai/tests.rs | key_accounts_follow_the_provider_id | DATA-3 | ported |  |
| rs | src-tauri/src/ai/tests.rs | empty_keys_and_unknown_providers_are_rejected | DATA-3 | ported |  |
| rs | src-tauri/src/ai/tests.rs | an_unavailable_keyring_reports_no_key_and_refuses_to_save | DATA-3 | ported |  |
| rs | src-tauri/src/ai/tests.rs | api_keys_are_redacted_in_debug_output | SEC-c | ported |  |
| rs | src-tauri/src/ai/tests.rs | follow_ups_replay_earlier_turns_as_alternating_messages_for_every_provider | AI-2 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_follow_ups_replay_turns_in_each_model_format | AI-2, AI-1c | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_first_question_sends_no_earlier_turns | AI-2 | ported |  |
| rs | src-tauri/src/ai/tests.rs | the_conversation_deserializes_from_camel_case_and_defaults_to_empty | AI-2, DATA-1a | ported | turn shape reused by ai-chats.json |
| rs | src-tauri/src/ai/tests.rs | anthropic_models_come_from_its_model_list_with_the_key_headers | AI-8 | ported |  |
| rs | src-tauri/src/ai/tests.rs | gemini_models_keep_only_those_that_generate_content | AI-8 | ported |  |
| rs | src-tauri/src/ai/tests.rs | openai_models_drop_non_chat_families_and_are_sorted | AI-8 | ported |  |
| rs | src-tauri/src/ai/tests.rs | openai_compatible_models_use_the_base_url_and_need_no_key | AI-8 | ported |  |
| rs | src-tauri/src/ai/tests.rs | listing_models_without_a_key_asks_for_one_and_sends_nothing | AI-8 | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_rejected_key_while_listing_models_is_an_auth_error | AI-9 | ported |  |
| rs | src-tauri/src/ai/tests.rs | opencode_go_models_come_from_its_catalogue | AI-1, AI-8 | ported |  |
| rs | src-tauri/src/ai/tests.rs | a_rejected_key_message_names_each_provider_once | AI-9 | ported |  |
| rs | src-tauri/src/ai/text.rs | complete_prompt_carries_only_the_given_fields | TERM-12c, AI-7a | ported |  |
| rs | src-tauri/src/ai/text.rs | prompts_keep_only_the_last_twenty_history_commands | TERM-12c | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_prompt_includes_the_request_and_the_selection_only_when_given | AI-7 | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_prompt_includes_attached_files_within_their_limits | DROP-1 | ported |  |
| rs | src-tauri/src/ai/text.rs | completion_strips_the_echoed_line | TERM-12e | ported |  |
| rs | src-tauri/src/ai/text.rs | completion_strips_fences_backticks_quotes_and_prompt_markers | TERM-12e | ported |  |
| rs | src-tauri/src/ai/text.rs | completion_keeps_only_the_first_line | TERM-12e | ported |  |
| rs | src-tauri/src/ai/text.rs | completion_is_empty_when_the_reply_does_not_continue_the_line | TERM-12e | ported |  |
| rs | src-tauri/src/ai/text.rs | completion_never_carries_control_characters_or_reasoning | TERM-12e | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_parses_an_answer_with_a_command_and_one_without | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_parses_json_wrapped_in_fences_or_text | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_reads_the_older_explanation_field_as_the_answer | AI-6 | ported | older `explanation` field compat |
| rs | src-tauri/src/ai/text.rs | a_plain_text_reply_is_the_answer_without_a_command | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_commands_are_one_line_without_control_characters | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | answers_keep_lines_and_tabs_but_drop_other_control_and_format_characters | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | long_answers_are_cut_with_a_note | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_rejects_empty_answers | AI-9 | ported |  |
| rs | src-tauri/src/ai/text.rs | a_command_without_an_answer_is_kept | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | error_detail_reads_the_provider_message_and_redacts_the_key | AI-9, SEC-c | ported |  |
| rs | src-tauri/src/ai/text.rs | completions_drop_format_and_separator_characters | TERM-12e | ported |  |
| rs | src-tauri/src/ai/text.rs | assist_drops_format_and_separator_characters_from_command_and_answer | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | commands_longer_than_the_cap_are_dropped_and_the_answer_kept | AI-6 | ported |  |
| rs | src-tauri/src/ai/text.rs | turns_are_replayed_as_the_question_and_the_json_answer | AI-2 | ported |  |
| rs | src-tauri/src/ai/text.rs | replayed_turns_lose_control_characters_but_keep_line_breaks | AI-2 | ported |  |
| rs | src-tauri/src/ai/text.rs | only_the_last_ten_turns_are_replayed | AI-2 | ported |  |
| rs | src-tauri/src/ai/text.rs | the_oldest_turns_go_until_the_conversation_fits_16000_characters | AI-2 | ported |  |
| rs | src-tauri/src/ai/text.rs | empty_questions_are_not_replayed | AI-2 | ported |  |
| rs | src-tauri/src/tools/keygen/ppk.rs | unencrypted_files_match_puttygen_byte_for_byte | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen/ppk.rs | encrypted_files_match_puttygen_byte_for_byte | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen/ppk.rs | random_salt_and_padding_still_read_back | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | generates_an_ed25519_key_whose_halves_parse_back | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | a_passphrase_encrypts_the_private_key | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | rejects_comments_that_do_not_fit_on_one_line | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | saves_the_private_key_0600_and_the_public_key_0644 | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | never_overwrites_without_being_asked_to | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | unknown_and_discarded_keys_cannot_be_saved | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | the_install_command_only_accepts_supported_public_key_lines | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/keygen.rs | the_install_command_appends_the_key_once_with_safe_modes | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/keygen.rs | the_install_command_survives_every_login_shell | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/keygen.rs | the_install_command_keeps_existing_keys_and_fixes_modes | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/keygen.rs | reads_the_install_outcome | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/keygen.rs | install_logs_in_and_runs_the_install_command | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/keygen.rs | the_key_type_names_match_the_frontend | TOOL-1 | ported | frontend = GPUI key-type enum |
| rs | src-tauri/src/tools/keygen.rs | every_type_and_size_generates_a_key_ssh_keygen_accepts | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | loads_openssh_keys_asking_for_the_passphrase_of_encrypted_ones | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | loads_keys_made_by_ssh_keygen | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | rejects_files_that_are_not_private_keys | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | changes_the_comment_and_passphrase_of_a_loaded_key | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | saves_the_public_and_the_private_key_separately | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | exports_putty_keys_that_load_back_with_their_passphrase | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | loads_puttygen_files_of_both_versions | TOOL-1 | ported |  |
| rs | src-tauri/src/tools/keygen.rs | the_install_command_appends_rsa_and_ecdsa_keys | TOOL-1, TOOL-1a | ported |  |
| rs | src-tauri/src/tools/monitor.rs | parses_a_linux_sample | TOOL-4 | ported |  |
| rs | src-tauri/src/tools/monitor.rs | a_non_linux_server_is_unavailable | TOOL-4 | ported |  |
| rs | src-tauri/src/tools/monitor.rs | missing_proc_sections_are_unavailable | TOOL-4 | ported |  |
| rs | src-tauri/src/tools/monitor.rs | garbled_output_is_unavailable | TOOL-4 | ported |  |
| rs | src-tauri/src/tools/monitor.rs | serializes_with_a_status_tag | n/a | dropped | IPC status-tag wire shape |
| rs | src-tauri/src/tools/monitor.rs | sample_runs_the_monitor_command_over_the_session | TOOL-4 | ported |  |
| rs | src-tauri/src/tools/scan.rs | parses_a_single_ipv4_address | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | parses_a_last_octet_range | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | parses_cidr_without_network_and_broadcast_addresses | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | rejects_invalid_ipv4_targets_with_a_clear_error | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | targets_accept_hostnames_and_ipv4_forms | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | parses_port_lists_sorted_and_deduplicated | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | rejects_invalid_ports_with_a_clear_error | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | caps_the_total_number_of_probes | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | names_well_known_services | TOOL-2, TOOL-2a | ported |  |
| rs | src-tauri/src/tools/scan.rs | parses_the_linux_arp_table | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | parses_getent_hosts_output | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | probes_report_open_and_closed_ports | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | cancelling_stops_the_scan_before_completion | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | port_scan_streams_open_ports_then_done | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | a_cancelled_port_scan_reports_done_as_cancelled | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/scan.rs | network_scan_reports_live_hosts_with_their_open_services | TOOL-2 | ported |  |
| rs | src-tauri/src/tools/wol.rs | parses_colon_and_dash_separated_macs | TOOL-3 | ported |  |
| rs | src-tauri/src/tools/wol.rs | rejects_malformed_macs | TOOL-3 | ported |  |
| rs | src-tauri/src/tools/wol.rs | the_magic_packet_is_six_ff_then_the_mac_sixteen_times | TOOL-3 | ported |  |
| rs | src-tauri/src/tools/wol.rs | sends_the_packet_over_udp | TOOL-3 | ported |  |
| rs | src-tauri/src/updates.rs | versions_are_read_with_or_without_v_and_suffixes | UI-6, UI-6a | ported |  |
| rs | src-tauri/src/updates.rs | only_a_newer_release_is_returned | UI-6 | ported |  |
| rs | src-tauri/src/updates.rs | drafts_pre_releases_and_bad_answers_are_never_updates | UI-6, UI-6a | ported |  |
| rs | src-tauri/src/updates.rs | the_check_asks_github_for_json_and_reports_the_newer_release | UI-6 | ported |  |
| rs | src-tauri/src/updates.rs | a_failed_check_is_an_error_message | UI-6, SET-3a | ported |  |

## 5. App shell, settings and terminal sharing

| Kind | File | Test | Req | Disposition | Note |
| --- | --- | --- | --- | --- | --- |
| ts | src/features/dialogs/App.dialogs.test.tsx | deletes a session only when accepted, naming the sessions that use it as a jump host | SES-1a, SES-1b | ui | |
| ts | src/features/dialogs/App.dialogs.test.tsx | deletes a folder and its sessions only when accepted | SES-1a, SES-1b | ui | |
| ts | src/features/dialogs/App.dialogs.test.tsx | creates a folder with the typed name, and nothing when cancelled | SES-1a, UI-5 | ui | |
| ts | src/features/dialogs/App.dialogs.test.tsx | deletes a tunnel only when accepted | TUN-1, SES-1b | ui | |
| ts | src/features/dialogs/App.dialogs.test.tsx | deletes a macro only when accepted | MAC-1, SES-1b | ui | |
| ts | src/features/dialogs/DialogProvider.test.tsx | shows the message as paragraphs and resolves true when accepted | UI-5 | ui | |
| ts | src/features/dialogs/DialogProvider.test.tsx | resolves false on Cancel and on Escape | UI-5, SES-1b | ui | |
| ts | src/features/dialogs/DialogProvider.test.tsx | focuses the safe button for destructive confirmations and the action otherwise | SES-1b | ui | |
| ts | src/features/dialogs/DialogProvider.test.tsx | shows one dialog at a time, in the order they were asked | UI-5 | ui | queue in the dialog service |
| ts | src/features/dialogs/DialogProvider.test.tsx | submits the typed text with Enter and with the button | UI-5 | ui | prompt dialog |
| ts | src/features/dialogs/DialogProvider.test.tsx | resolves null on Cancel and on Escape | UI-5 | ui | |
| ts | src/features/dialogs/DialogProvider.test.tsx | needs a DialogProvider | UI-5 | dropped | React context precondition; no native equivalent |
| ts | src/features/menu/ContextMenu.test.tsx | runs the chosen item and closes | UI-5 | ui | |
| ts | src/features/menu/ContextMenu.test.tsx | shows disabled items and separators without running them | UI-5, SHAPE-1 | ui | |
| ts | src/features/menu/ContextMenu.test.tsx | focuses the first item and closes on Escape or a click outside | UI-5 | ui | |
| ts | src/features/menu/ContextMenu.test.tsx | opens at the pointer and stays inside the window | UI-5, SHAPE-1 | ui | |
| ts | src/features/panes/App.panes.test.tsx | splits the tab into a second pane without restarting the first one | PANE-1 | ui | |
| ts | src/features/panes/App.panes.test.tsx | offers a close button on each pane only while the tab is split, closing that pane | PANE-1 | ui | |
| ts | src/features/panes/App.panes.test.tsx | closes only the focused pane, and the tab with its last pane | PANE-1, TERM-19 | ui | |
| ts | src/features/panes/App.panes.test.tsx | multi-execution mirrors the focused pane to the other panes except excluded ones | MEXEC-1 | ui | |
| ts | src/features/panes/App.shortcuts.test.tsx | opens a terminal on the user's combination, and no longer on the default one | KEY-5 | ui | |
| ts | src/features/panes/App.shortcuts.test.tsx | an action without a default works once bound | KEY-2 | ui | |
| ts | src/features/panes/App.shortcuts.test.tsx | an unbound action does nothing | KEY-2 | ui | |
| ts | src/features/panes/App.shortcuts.test.tsx | a cleared default stops triggering and leaves the title without a shortcut | KEY-1 | ui | |
| ts | src/features/panes/App.shortcuts.test.tsx | moves between tabs and closes the tab with bound shortcuts | KEY-1, TAB-1 | ui | |
| ts | src/features/panes/App.shortcuts.test.tsx | shows the effective shortcut in the pane close button and the toolbar | KEY-1, UI-1 | ui | |
| ts | src/features/panes/MultiExecBanner.test.tsx | shows how many terminals receive the input and turns multi-execution off | MEXEC-1, UI-2 | ui | |
| ts | src/features/panes/MultiExecBanner.test.tsx | uses the singular for one terminal | MEXEC-1, UI-9 | ui | plural rules |
| ts | src/features/panes/PaneCloseButton.test.tsx | shows the × only while the pointer is in the corner, and closes the pane | PANE-1 | ui | |
| ts | src/features/panes/PaneGrid.test.tsx | renders one pane per leaf and highlights the focused one | PANE-1 | ui | |
| ts | src/features/panes/PaneGrid.test.tsx | hides panes without unmounting them: the others fill the grid, and they come back as they were | DEV-2a | ui | DEV single-terminal view |
| ts | src/features/panes/PaneGrid.test.tsx | shows every pane when all of them would be hidden | DEV-2a | ported | `shownLayout` rule |
| ts | src/features/panes/PaneGrid.test.tsx | focuses a pane on click | PANE-1 | ui | |
| ts | src/features/panes/PaneGrid.test.tsx | splitting or closing a sibling never remounts the other panes | PANE-1 | ui | terminal entities survive relayout |
| ts | src/features/panes/PaneGrid.test.tsx | dragging a divider reports the new ratio of its split | PANE-1 | ui | |
| ts | src/features/panes/PaneGrid.test.tsx | has no divider for a single pane | PANE-1 | ui | |
| ts | src/features/panes/PaneGrid.test.tsx | places panes and dividers on whole pixels, edge to edge, so 1px focus borders always show | PANE-1, SHAPE-1 | ui | |
| ts | src/features/panes/multiExec.test.ts | contains the connected panes that are not excluded | MEXEC-1 | ported | |
| ts | src/features/panes/multiExec.test.ts | sends to every other connected, non-excluded pane | MEXEC-1 | ported | |
| ts | src/features/panes/multiExec.test.ts | sends nothing from an excluded or unknown pane | MEXEC-1 | ported | |
| ts | src/features/panes/multiExec.test.ts | writes to the registered panes only | MEXEC-1 | ported | |
| ts | src/features/panes/multiExec.test.ts | unregistering an old writer keeps a newer one for the same pane | MEXEC-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | splits a pane in two halves, the new pane after the old one | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | can put the new pane before the old one (left of it, or above) | PANE-1, PANE-2 | ported | |
| ts | src/features/panes/paneLayout.test.ts | splits a nested pane in place | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | refuses to grow a tab beyond ${MAX_PANES} panes | PANE-1 | ported | max 4 |
| ts | src/features/panes/paneLayout.test.ts | leaves the layout unchanged for an unknown pane | PANE-1 | ported | split |
| ts | src/features/panes/paneLayout.test.ts | returns null when the last pane closes | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | lets the sibling take the parent's place | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | leaves the layout unchanged for an unknown pane | PANE-1 | ported | close |
| ts | src/features/panes/paneLayout.test.ts | changes the ratio of one split, clamped to 15%-85% | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | places panes in a unit square following the ratios | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | moves focus to the closest overlapping pane in the direction | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | returns null at the edge or for a single pane | PANE-1 | ported | |
| ts | src/features/panes/paneLayout.test.ts | ignores panes that do not overlap on the perpendicular axis | PANE-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | maps the Ctrl+Shift shortcuts, whatever the letter case | KEY-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | maps Alt+Arrow to focus moves | KEY-1, PANE-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | maps find, copy and paste (S15.1, S15.3) | KEY-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | maps Ctrl+= / Ctrl++ / Ctrl+- / Ctrl+0 to font size changes (S15.2) | KEY-1, TERM-18 | ported | includes Ctrl+Shift+- / Ctrl+Shift+0 (W5) |
| ts | src/features/panes/shortcuts.test.ts | maps Ctrl+Shift+R to recording and Ctrl+Alt+1..9 to macro hotkeys (S16.2, S16.4) | KEY-4, MAC-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | leaves every other key to the terminal | KEY-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | Ctrl+Shift+B folds or opens the sidebar | KEY-1, UI-3 | ported | |
| ts | src/features/panes/shortcuts.test.ts | an override replaces every default combination of its action | KEY-5 | ported | |
| ts | src/features/panes/shortcuts.test.ts | an empty override leaves the action unbound | KEY-5 | ported | |
| ts | src/features/panes/shortcuts.test.ts | actions without a default do nothing until bound | KEY-2 | ported | |
| ts | src/features/panes/shortcuts.test.ts | keeps Meta combinations, AltGr characters and the macro keys out of the defaults | KEY-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | shows the first effective combination of an action | KEY-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | names what already uses a combination | KEY-1 | ported | |
| ts | src/features/panes/shortcuts.test.ts | is an unbound terminal action that answers to the combination it is given | KEY-5, DEV-1 | ported | toggleDevMode |
| ts | src/features/settings/App.settings.test.tsx | changes the font size of every terminal with Ctrl+= / Ctrl+- / Ctrl+0 and saves it | TERM-18, KEY-1 | ui | |
| ts | src/features/settings/App.settings.test.tsx | changes only the AI chat text with Ctrl+= / Ctrl+- / Ctrl+0 while the chat has the focus | AI-8 | ui | |
| ts | src/features/settings/App.settings.test.tsx | sets the interface and the chat text sizes from Settings, apart from the terminal's | SET-3a, FONT-1 | ui | |
| ts | src/features/settings/App.settings.test.tsx | edits the settings from the toolbar gear | SET-3, UI-1 | ui | |
| ts | src/features/settings/App.settings.test.tsx | colours the whole interface with the chosen scheme and swaps the logo for light schemes | THEME-1, SCHEME-1 | ui | |
| ts | src/features/settings/App.settings.test.tsx | shows an icon next to every toolbar action and sidebar tab | UI-1, UI-3 | ui | More/DEV column button: W3, W4 |
| ts | src/features/settings/App.settings.test.tsx | opens Wake On LAN from the Tools menu | TOOL-3, UI-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows one section at a time from the side menu, starting with Appearance | SET-3 | ui | Sharing section added (W2) |
| ts | src/features/settings/SettingsDialog.test.tsx | keeps the edits of every section when saving | SET-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | keeps the dragged sidebar width, which has no field, when saving | SET-1, UI-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows the logo, the version and the project in About, light for light schemes | SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows the current settings | SET-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | chooses the text editor with Browse…, resets it to the system default and saves it | SFTP-1a, SET-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | lists dark and light schemes separately and previews the selected one | SET-3a, SCHEME-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | picks the font from the catalogue: default, bundled, installed, or typed by hand | SET-3a, FONT-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | saves the edited settings, clamping the font size | SET-1, TERM-18 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | edits the commands that turn DEV mode on (S10) | DEV-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | restores the default font when the font family is left empty | SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | keeps the font size when the field is not a number | SET-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | cancels without saving, also on Escape | SET-3, UI-5 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows the defaults: suggestions and history on, AI off, Anthropic with its model | SET-3a, AI-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | saves the AI settings, clamping the history context | TERM-12c, SET-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | switches a provider's default model to the new provider's default, but keeps a custom one | SET-3a, AI-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | saves and removes the provider's API key in the keyring, showing only whether one is saved | SET-3a, DATA-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows the backend's message when the key cannot be saved | SET-3a, DATA-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | offers OpenCode Go models in a list fetched by the backend, with a refresh and a usage note | AI-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | keeps a saved OpenCode Go model that is not in the list, and shows the list's note | AI-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | falls back to typing the OpenCode Go model when the list cannot be fetched | AI-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | hides file sizes and dates by default and saves the choice (L15) | SET-1 | ui | `fileDetails`; no §7.4 wording |
| ts | src/features/settings/SettingsDialog.test.tsx | chooses the terminal line height, 1.0 by default, warns what a taller one does, and saves it | TERM-18 | ui | default 1.0 now (W1) |
| ts | src/features/settings/SettingsDialog.test.tsx | chooses the weight of the terminal font, regular by default, and saves it | TERM-18 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows git status in file lists by default and saves the choice | TREE-1, SET-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | leaves auto-save of edited files off by default and saves the choice | VIEW-2 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | opens the file viewer at the left by default and saves another side | VIEW-7 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | chooses how the sidebar opens at startup (L17) | UI-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | chooses how DEV mode's tab column folds and how it opens at startup | DEV-2b | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | chooses how many recent DEV mode folders to keep, and whether the bar under a DEV terminal names its folder | DEV-4, PANE-3b | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | changes the terminal's folder with the file browser by default, and saves the choice (L16) | SFTP-4 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | keeps AI conversations by default and saves the choice (S9) | AI-8 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | saves the OpenCode Go key under its own provider | AI-1, DATA-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | clears the command history at once | TERM-12b | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | no longer edits the assistant shortcut, which moved to Shortcuts | SET-3a, KEY-5 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | lists the actions by group, with the fixed keys for reference | KEY-3, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | captures a new combination; Escape cancels without closing; saving emits the overrides | KEY-5, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | refuses a combination another action, a fixed key or the assistant uses, naming it | KEY-4 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | binds a shortcut to fold DEV mode's tab column, which has none by default | DEV-2b | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | binds a shortcut to list the recent DEV mode folders, which has none by default | DEV-4 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | lets a freed default be taken by another action | KEY-5 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | clears a shortcut, and resets one row to its default | KEY-5, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | refuses a reset whose default another action took meanwhile | KEY-5 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | restores every default, the assistant's too | KEY-5, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | captures the assistant shortcut, refusing app shortcuts | KEY-5, AI-3 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | lists the system default and the detected shells, and saves the chosen one with its arguments | TERM-16 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | Other… takes any program, typed or chosen with Browse… | TERM-16 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows a saved shell that was not detected under Other… | TERM-16 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | going back to the system default forgets the program and its arguments | TERM-16 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | saves the starting folder of local terminals, typed or chosen (S8) | TERM-16 | ui | `localCwd` |
| ts | src/features/settings/SettingsDialog.test.tsx | offers the system default and Other… before the shells are known | TERM-16 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | says when PlexusTerm is up to date | UI-6, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | offers the download page of a newer release | UI-6, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | shows why a check failed | UI-6, SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | saves the startup check choice | UI-6, SET-1 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | opens the bug report form with the version filled in | SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | opens the feature request form | SET-3a | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | lists the shipped libraries in their own dialog and goes back to Settings | UI-7 | ui | |
| ts | src/features/settings/SettingsDialog.test.tsx | links to the relay's project to install it | SHARE-1 | ui | |
| ts | src/features/settings/fontCatalog.test.ts | lists the fonts shipped with the app, each with a CSS family list that ends in monospace | FONT-1, SET-3a | ported | family list, not CSS |
| ts | src/features/settings/fontCatalog.test.ts | finds the candidates whose text is drawn differently from every fallback | FONT-1 | ported | probe via platform font enumeration |
| ts | src/features/settings/fontCatalog.test.ts | finds none when nothing can be measured | FONT-1 | ported | |
| ts | src/features/settings/fontCatalog.test.ts | only tries known monospaced families, each once | FONT-1 | ported | |
| ts | src/features/settings/fontCatalog.test.ts | names the catalogue entry a font family setting matches | SET-3a | ported | |
| ts | src/features/settings/fontCatalog.test.ts | is empty for anything typed by hand | SET-3a | ported | |
| ts | src/features/settings/settings.test.ts | drops OpenCode's own opencode-go/ prefix from OpenCode Go model ids | AI-1 | ported | |
| ts | src/features/settings/settings.test.ts | returns the defaults when there is no settings file | SET-1 | ported | 55 keys now (W1) |
| ts | src/features/settings/settings.test.ts | opens the sidebar as Settings choose at startup, or as it was left (L17) | UI-3 | ported | |
| ts | src/features/settings/settings.test.ts | opens the DEV tab column as Settings choose at startup, or as it was left | DEV-2b | ported | |
| ts | src/features/settings/settings.test.ts | keeps between 1 and 50 recent DEV mode folders, 10 by default | DEV-4 | ported | |
| ts | src/features/settings/settings.test.ts | reads where the file viewer opens, the left when unknown | VIEW-7 | ported | |
| ts | src/features/settings/settings.test.ts | returns the defaults for a file that is not a JSON object | SET-1 | ported | |
| ts | src/features/settings/settings.test.ts | reads saved values | SET-1 | ported | |
| ts | src/features/settings/settings.test.ts | clamps the font size to 8..32 and rounds it | SET-1, TERM-18 | ported | |
| ts | src/features/settings/settings.test.ts | defaults invalid values and ignores unknown keys | SET-1 | ported | unknown keys preserved on write |
| ts | src/features/settings/settings.test.ts | uses the default theme for unknown or invalid theme ids | SET-1, SCHEME-1 | ported | |
| ts | src/features/settings/settings.test.ts | uses the default font for an empty, overlong or invalid font family and trims it | SET-1, FONT-1 | ported | |
| ts | src/features/settings/settings.test.ts | uses the default for an invalid remote monitor value | SET-1, TOOL-4 | ported | |
| ts | src/features/settings/settings.test.ts | uses the system default text editor for an invalid or overlong value and trims it | SET-1, SFTP-1a | ported | |
| ts | src/features/settings/settings.test.ts | defaults invalid suggestion and AI switches | SET-1, TERM-12 | ported | |
| ts | src/features/settings/settings.test.ts | knows Anthropic, Gemini, OpenAI and OpenAI-compatible providers, each with its default model | AI-1 | ported | |
| ts | src/features/settings/settings.test.ts | knows OpenCode Go, whose model is left for the user to pick | AI-1 | ported | |
| ts | src/features/settings/settings.test.ts | accepts only http(s) base URLs | AI-1 | ported | |
| ts | src/features/settings/settings.test.ts | keeps 0..20 history commands as AI context, rounding and clamping | TERM-12c | ported | |
| ts | src/features/settings/settings.test.ts | uses the default assistant shortcut for an invalid one | KEY-5 | ported | |
| ts | src/features/settings/settings.test.ts | keeps only usable shortcut overrides | KEY-5 | ported | |
| ts | src/features/settings/settings.test.ts | lets the assistant take a combination an override freed | KEY-5 | ported | |
| ts | src/features/settings/settings.test.ts | keeps valid values next to invalid ones | SET-1 | ported | |
| ts | src/features/settings/settings.test.ts | is ignored in older settings files: the branch only depends on DEV mode | SET-1, DEV-3 | ported | removed `gitBranch` key |
| ts | src/features/settings/settings.test.ts | round-trips through parseSettings with a version | SET-1, DATA-4 | ported | |
| ts | src/features/settings/settings.test.ts | grows, shrinks and resets the font size within 8..32 | TERM-18 | ported | |
| ts | src/features/settings/settings.test.ts | default to an open sidebar that folds to icons, and keep saved booleans only | UI-3 | ported | |
| ts | src/features/settings/settings.test.ts | keeps a dragged width within 160..640 px and forgets anything else | UI-3 | ported | |
| ts | src/features/settings/settings.test.ts | default to the system shell and keep only text of at most 1024 characters | TERM-16 | ported | |
| ts | src/features/settings/settings.test.ts | is on by default and keeps only a saved boolean | UI-6 | ported | |
| ts | src/features/settings/settings.test.ts | defaults to the system language and keeps only supported values | UI-9 | ported | |
| ts | src/features/settings/settings.test.ts | lists the known AI CLIs by default and keeps an edited list, even an empty one | DEV-1 | ported | |
| ts | src/features/settings/settings.test.ts | is the default width until dragged, kept within 140..480 px | DEV-2 | ported | |
| ts | src/features/settings/settings.test.ts | give the file viewer its own text size, 13 px by default, between 10 and 24 | VIEW-7 | ported | |
| ts | src/features/settings/settings.test.ts | default to 13 px and stay within their ranges | SET-3a, FONT-1 | ported | |
| ts | src/features/settings/settings.test.ts | steps the chat text with Ctrl+= / Ctrl+- / Ctrl+0 within 10..24 | AI-8 | ported | |
| ts | src/features/settings/settings.test.ts | is off by default there, as WebGL in WebKitGTK delays every key by several frames | SET-2 | dropped | WebKitGTK-specific; SET-2 makes `gpuAcceleration` true everywhere |
| ts | src/features/settings/settings.test.ts | is turned off once in settings saved before this change, and kept when turned on again | SET-2 | dropped | pre-v2 migration kept "minus forcing GPU off" (SET-2) |
| ts | src/features/settings/settings.test.ts | is 400 by default and only one of the offered weights | TERM-18 | ported | |
| ts | src/features/settings/settings.test.ts | is 1 by default and only one of the offered heights | TERM-18 | ported | default changed 1.15 → 1 (W1) |
| ts | src/features/settings/settings.test.ts | are off the relay until set, ask for View only joins and reset sudo by default | SHARE-1 | ported | |
| ts | src/features/settings/settings.test.ts | keep only relay and viewer URLs of their schemes | SHARE-1 | ported | |
| ts | src/features/settings/shortcutCapture.test.ts | names a key combination | KEY-5, SET-3a | ported | |
| ts | src/features/settings/shortcutCapture.test.ts | waits while only modifiers are held | KEY-5, SET-3a | ported | |
| ts | src/features/settings/shortcutCapture.test.ts | needs Ctrl, Alt or Meta and one key | KEY-1 | ported | |
| ts | src/features/settings/shortcutCapture.test.ts | refuses combinations PlexusTerm already uses | KEY-5 | ported | |
| ts | src/features/settings/uiTheme.test.ts | builds the interface colours from the scheme | THEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | layers panels and borders between the background and the text | THEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | marks light schemes so native controls are drawn light | THEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | picks dark or light text for buttons by the accent's brightness | THEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | keeps the original look for the PlexusTerm scheme | THEME-1, SCHEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | blends two colours by a weight and ignores alpha | THEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | colours git statuses in the file lists with VS Code's defaults for dark and light schemes | THEME-1 | ported | |
| ts | src/features/settings/uiTheme.test.ts | colours code in the file viewer from the scheme's ANSI palette | THEME-1, VIEW-1 | ported | |
| ts | src/features/settings/useSettings.test.ts | loads the saved settings | SET-1 | ported | settings model |
| ts | src/features/settings/useSettings.test.ts | keeps the defaults when loading fails | SET-1 | ported | |
| ts | src/features/settings/useSettings.test.ts | saves every change, clamped, and keeps it over a late load | SET-1, DATA-2a | ported | |
| ts | src/features/sidebar/App.sidebar.test.tsx | Ctrl+Shift+B folds the sidebar to its tab icons and opens it again, remembering it | UI-3, KEY-1 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | folds from the toolbar, and a tab icon opens the sidebar on that tab | UI-3, UI-1 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | starts folded when it was folded last time | UI-3 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | hides completely when folded if Settings say so, and comes back with Ctrl+Shift+B | UI-3 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | saves the 'hide completely' choice from Settings > Appearance | UI-3, SET-3a | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | drags its edge to a new width, within 160..640 px, and remembers it when the drag ends | UI-3 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | starts with the remembered width; a double-click on the edge goes back to the default width | UI-3, SHAPE-1 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | keeps following the pointer when it leaves the handle, and blocks text selection while dragging | UI-3, COMP-1 | ui | |
| ts | src/features/sidebar/App.sidebar.test.tsx | has no resize handle while folded | UI-3 | ui | |
| ts | src/features/sidebar/HelpTip.test.tsx | names its explanation and shows it on hover or focus until the pointer leaves | UI-8, COMP-1 | ui | |
| ts | src/features/sidebar/Sidebar.test.tsx | shows the file browser that fits the active tab instead of the one that does not | UI-3 | ui | |
| ts | src/features/sidebar/Sidebar.test.tsx | lists the Explorer first, then Sessions, SFTP, Tunnels and Macros | UI-3 | ui | order (W4) |
| ts | src/features/tabs/TabBar.test.tsx | offers Disconnect instead of Share on a tab that watches another PlexusTerm's share | SHARE-14 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | marks the tabs whose output is logged to a file | TAB-1, TERM-10 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | marks the tabs that need attention | TAB-1, TERM-9 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | in DEV mode's column, names the tabs after one the tabs below it | DEV-2, TAB-1 | ui | "Close tabs below" |
| ts | src/features/tabs/TabBar.test.tsx | offers duplicate, close, close others, close to the right and duplicate on right click | TAB-1 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | copies the host address of SSH, Telnet and Mosh tabs, and offers nothing to copy for others | TAB-1 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | marks the tabs in DEV mode (S10) | TAB-1, DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | stays a row of tabs on top unless asked for the column (S13) | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | shows a tab that hides terminals as one row per terminal, the one on screen as the tab | DEV-2a | ui | |
| ts | src/features/tabs/TabBar.test.tsx | lists no hidden terminals outside the column | DEV-2a | ui | |
| ts | src/features/tabs/TabBar.test.tsx | lists one row per tab with its CLI's icon, title and project folder | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | drops the CLI's own mark from the title when its icon is shown | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | marks the active row and activates a row on click | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | closes a tab with the button at the end of its row and with a middle click | DEV-2, TAB-1 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | keeps the attention and logging marks and the right-click menu | DEV-2, TAB-1 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | shows only each row's icon, keeps the title as its tooltip, and still activates and opens tabs | DEV-2b | ui | folded column |
| ts | src/features/tabs/TabBar.test.tsx | drags its left edge to a new width, within 140..480 px, saved when the drag ends | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | starts at the remembered width and goes back to the default on a double-click | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | has no handle on the top tab bar | DEV-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | moves a tab dragged along the bar to where it is dropped, marking the place meanwhile | TAB-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | moves a tab up the DEV column | TAB-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | leaves a click a click, and a drop back in place moves nothing | TAB-2 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | picks a tab's colour from its menu, and clears it | TAB-1, UI-4 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | shows the colour on the tab on top and on the DEV row | TAB-1, SHAPE-1 | ui | |
| ts | src/features/tabs/TabBar.test.tsx | offers no colour item without a handler | TAB-1 | ui | |
| ts | src/features/tabs/tabs.test.ts | opens a tab and makes it active | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | titles SSH tabs user@host, adding the port only when it is not 22 | TAB-1, SSH-4 | ported | title rule not in PRD text |
| ts | src/features/tabs/tabs.test.ts | gives every tab a unique key | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | activates a tab by key | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing the active tab activates its right neighbour, else the left one | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing an inactive tab keeps the active one | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing the last tab leaves no active tab | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | keeps the same state when a tab is renamed to the title it already has (no re-render) | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | renames a tab | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | opens a tab with one focused pane; pane ids are unique across tabs | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | splitting adds a pane after the focused one and focuses it | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | refuses a fifth pane | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing a pane keeps the tab and focuses a remaining pane | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing an unfocused pane keeps the focus | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing the last pane closes the tab | PANE-1, TERM-19 | ported | |
| ts | src/features/tabs/tabs.test.ts | focuses a pane by id or by direction | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | resizes a split with the ratio clamped | PANE-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closes every tab but one, keeping it active | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | closes the tabs to the right of one, activating it when the active tab closed | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | moves a tab to another place, keeping the active one and every tab's panes | TAB-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | closes every tab | TAB-1 | ported | |
| ts | src/features/tabs/tabs.test.ts | splits the focused terminal down with a chat pane linked to it, focused | AI-4, PANE-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | focuses the existing chat of a terminal instead of opening a second one | AI-4 | ported | |
| ts | src/features/tabs/tabs.test.ts | from a chat pane, goes back to its terminal | AI-4 | ported | |
| ts | src/features/tabs/tabs.test.ts | does nothing when the tab is full | AI-4 | ported | chat |
| ts | src/features/tabs/tabs.test.ts | closing a terminal also closes its chat; closing the chat focuses its terminal | AI-8, PANE-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | closing the only terminal of a tab with its chat closes the tab | AI-8, TERM-19 | ported | |
| ts | src/features/tabs/tabs.test.ts | opens a file at the left of the focused terminal, linked to it and focused | PANE-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | opens a file at the right of the terminal or below it as asked | VIEW-7 | ported | |
| ts | src/features/tabs/tabs.test.ts | shows another file of the same terminal in its viewer instead of opening a second one | PANE-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | does nothing when the tab is full | PANE-2 | ported | viewer |
| ts | src/features/tabs/tabs.test.ts | closing a terminal also closes its viewer; closing the viewer focuses its terminal | PANE-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | opens the AI chat of the viewer's terminal from a focused viewer | AI-4, PANE-2 | ported | |
| ts | src/features/tabs/tabs.test.ts | colours a tab, clears it, and gives a duplicate the same colour | TAB-1, UI-4 | ported | |
| ts | src/features/toolbar/App.toolbarColor.test.tsx | colours the bar from its right-click menu, with readable text, and clears it | UI-4 | ui | |
| ts | src/features/toolbar/App.toolbarColor.test.tsx | takes a custom colour, and keeps it in this window only (session storage) across reloads | UI-4 | ui | per-window state; "reload" has no native meaning |
| ts | src/features/toolbar/App.toolbarColor.test.tsx | does not open the colour menu from the toolbar's buttons | UI-4 | ui | |
| ts | src/features/toolbar/CustomColorDialog.test.tsx | starts from the colour in use and accepts a typed code, the Accept button painted with it | UI-4 | ui | |
| ts | src/features/toolbar/CustomColorDialog.test.tsx | refuses a code that is not a colour | UI-4 | ui | |
| ts | src/features/toolbar/CustomColorDialog.test.tsx | takes the system picker's colour | UI-4 | ui | |
| ts | src/features/toolbar/CustomColorDialog.test.tsx | picks a colour anywhere on the screen, and keeps the colour when picking is cancelled | UI-4 | ui | |
| ts | src/features/toolbar/CustomColorDialog.test.tsx | has no screen picker where the system offers none, and cancels with its button and Escape | UI-4 | ui | |
| ts | src/features/toolbar/CustomColorDialog.test.tsx | says why picking from the screen failed | UI-4 | ui | |
| ts | src/features/toolbar/compact.test.ts | keeps the labels while everything fits | UI-1 | ported | |
| ts | src/features/toolbar/compact.test.ts | drops to icons only when even the shortened labels overflow, remembering that width | UI-1 | ported | |
| ts | src/features/toolbar/compact.test.ts | stays on icons until the toolbar is wider than where the labels stopped fitting | UI-1 | ported | |
| ts | src/features/toolbar/useCompactToolbar.test.tsx | keeps a short label's minimum width the same however often the window is resized | UI-1 | ported | |
| ts | src/i18n/App.i18n.test.tsx | follows the system language by default, and English when it is not supported | UI-9 | ui | |
| ts | src/i18n/App.i18n.test.tsx | names the first tab in the chosen language and renames default tab titles when the language changes | UI-9, TAB-1 | ui | |
| ts | src/i18n/App.i18n.test.tsx | uses the language chosen in Settings, right to left for Arabic | UI-9, COMP-1 | ui | |
| ts | src/i18n/App.i18n.test.tsx | has untranslated rescue buttons that switch to English or the system language at once | UI-9, SET-3a | ui | |
| ts | src/i18n/catalogs.test.ts | names the font picker and the font family field differently in every language | UI-9, SET-3a | ported | |
| ts | src/i18n/catalogs.test.ts | has one catalogue per interface language | UI-9 | ported | |
| ts | src/i18n/catalogs.test.ts | ${code}: has every English key, no extra ones, and keeps each message's placeholders | UI-9 | ported | includes `share.*` keys |
| ts | src/i18n/i18n.test.ts | interpolates every {var} and reads nested keys | UI-9 | ported | |
| ts | src/i18n/i18n.test.ts | picks plural forms with the language's own rules | UI-9 | ported | |
| ts | src/i18n/i18n.test.ts | falls back to English for a key a catalogue lacks, and to the key itself last | UI-9 | ported | |
| ts | src/i18n/i18n.test.ts | leaves unknown placeholders as they are | UI-9 | ported | |
| ts | src/i18n/i18n.test.ts | uses the chosen language, or the first supported system language, or English | UI-9 | ported | |
| ts | src/i18n/i18n.test.ts | lists the ten languages by their own names | UI-9 | ported | |
| ts | src/i18n/i18n.test.ts | starts in English and sets <html lang> and dir, right to left only for Arabic | UI-9, COMP-1 | ported | layout direction flag instead of html attrs |
| ts | src/i18n/i18n.test.ts | translates app keys in the current language | UI-9 | ported | |
| rs | src-tauri/src/screen_color.rs | reads_a_pixel_with_the_screen_visual_masks | UI-4 | ported | X11 eyedropper |
| rs | src-tauri/src/screen_color.rs | turns_the_portal_colour_into_a_hex_code | UI-4 | ported | Wayland portal |
| ts | src/features/share/SharedView.test.tsx | joins through the confirmed relay and keeps the sharer's size | SHARE-13, SHARE-14 | ui | |
| ts | src/features/share/SharedView.test.tsx | types only with Can type, while live and not paused | SHARE-14 | ui | |
| ts | src/features/share/SharedView.test.tsx | never answers terminal queries nor writes the clipboard for the remote program | SHARE-14 | ui | parser filters in term-engine |
| ts | src/features/share/SharedView.test.tsx | clears the pane and says why when the share is over for it | SHARE-13, SHARE-14 | ui | |
| ts | src/features/share/SharedView.test.tsx | closes its connection when the pane closes | SHARE-14 | ui | |
| ts | src/features/share/inputHold.test.ts | releases a viewer's keys whole and in order | SHARE-5 | ported | |
| ts | src/features/share/inputHold.test.ts | keeps another viewer's keys while one types, for 2 s after their last key | SHARE-5 | ported | |
| ts | src/features/share/inputHold.test.ts | holds everything while blocked and lets it through afterwards | SHARE-5 | ported | |
| ts | src/features/share/inputHold.test.ts | drops the keys of a viewer that left and frees the input | SHARE-5 | ported | |
| ts | src/features/share/inputHold.test.ts | blocks for Pause, then a password prompt, then the sharer's own typing | SHARE-5 | ported | |
| ts | src/features/share/inputHold.test.ts | clears every queued key when the share becomes View only | SHARE-5 | ported | |
| ts | src/features/share/useShares.test.ts | starts a share of the pane's tap with its size and the chosen options | SHARE-2 | ported | share model |
| ts | src/features/share/useShares.test.ts | tells the sharer of a join request and sends their answer | SHARE-4 | ported | |
| ts | src/features/share/useShares.test.ts | passes Allow everyone automatically and tells each automatic join | SHARE-4 | ported | |
| ts | src/features/share/useShares.test.ts | types viewer input, and holds it at a password prompt with a notice | SHARE-5 | ported | |
| ts | src/features/share/useShares.test.ts | holds viewer input while paused and types it on resume | SHARE-5 | ported | |
| ts | src/features/share/useShares.test.ts | answers a snapshot request with the pane's serialization | SHARE-6 | ported | |
| ts | src/features/share/useShares.test.ts | types sudo -k once the share is live when asked, only on an empty command line | SHARE-7 | ported | |
| ts | src/features/share/useShares.test.ts | drops the keys a viewer queued once the share is View only or the viewer is disconnected | SHARE-5 | ported | |
| ts | src/features/share/useShares.test.ts | forgets the share when it ends and tells why | SHARE-8 | ported | |
| ts | src/features/share/useShares.test.ts | reports a share a crash ended in an earlier run | SHARE-12 | ported | |
| rs | src-tauri/src/share/audit.rs | appends_entries_with_their_fields_and_reads_them_back | SHARE-11 | ported | |
| rs | src-tauri/src/share/audit.rs | prune_keeps_only_the_last_30_days | SHARE-11 | ported | |
| rs | src-tauri/src/share/audit.rs | a_missing_log_reads_empty_and_prunes_to_nothing | SHARE-11 | ported | |
| rs | src-tauri/src/share/awake.rs | the_lid_switch_is_inhibited_only_when_asked | SHARE-10 | ported | |
| rs | src-tauri/src/share/awake.rs | releasing_a_share_that_never_held_the_system_does_not_underflow | SHARE-10 | ported | |
| rs | src-tauri/src/share/host.rs | changes_wait_for_the_relay_while_attaching_goes_at_once | SHARE-9 | ported | |
| rs | src-tauri/src/share/marker.rs | a_clean_end_leaves_nothing_and_a_crash_is_reported_once | SHARE-12 | ported | |
| rs | src-tauri/src/share/marker.rs | a_share_still_refreshed_by_another_instance_is_not_a_crash | SHARE-12 | ported | |
| rs | src-tauri/src/share/marker.rs | a_missing_folder_has_no_crashes | SHARE-12 | ported | |
| rs | src-tauri/src/share/tests.rs | relay_urls_need_tls_except_on_this_computer | SHARE-1 | ported | |
| rs | src-tauri/src/share/tests.rs | viewer_origins_need_https_except_on_this_computer | SHARE-1 | ported | |
| rs | src-tauri/src/share/tests.rs | every_expiry_ends_within_12_hours_with_a_margin | SHARE-2 | ported | |
| rs | src-tauri/src/share/tests.rs | commands_and_events_have_the_shapes_the_frontend_uses | SHARE-9 | dropped | Tauri IPC JSON shapes; native uses typed messages (§6.1) |
| rs | src-tauri/src/share/tests.rs | qr_codes_are_svg | SHARE-2 | ported | QR rendered natively, no SVG needed |
| rs | src-tauri/src/share/tests.rs | a_tab_cannot_be_shared_twice_nor_after_it_closed | SHARE-2 | ported | |
| rs | src-tauri/src/share/tests.rs | a_shared_tab_reaches_a_viewer_and_takes_its_keys | SHARE-2, SHARE-6, SHARE-11 | ported | real relay in test |
| rs | src-tauri/src/share/tests.rs | the_share_ends_when_the_shared_program_exits | SHARE-8 | ported | |
| rs | src-tauri/src/share/tests.rs | a_wrong_token_ends_the_share_as_refused | SHARE-8 | ported | |
| rs | src-tauri/src/share/tests.rs | view_only_joins_can_skip_approval | SHARE-4 | ported | |
| rs | src-tauri/src/share/tests.rs | quitting_ends_every_share_after_the_user_confirmed | SHARE-8 | ported | |
| rs | src-tauri/src/share/tests.rs | another_plexusterm_watches_and_types_into_a_shared_tab | SHARE-13 | ported | |
| rs | src-tauri/src/share/tests.rs | allowing_everyone_lets_can_type_joins_in_without_asking | SHARE-4 | ported | |
| rs | src-tauri/src/share/viewers.rs | reads_the_browser_and_system_of_common_user_agents | SHARE-4 | ported | |
| rs | src-tauri/src/share/viewers.rs | numbers_viewers_from_one_and_gives_a_returning_viewer_its_number | SHARE-4 | ported | |
| rs | src-tauri/src/share/viewers.rs | view_only_turns_every_viewer_to_view_only | SHARE-3 | ported | |
| rs | src-tauri/src/share/watch.rs | reads_the_relay_a_viewer_page_connects_to | SHARE-13 | ported | |
| rs | src-tauri/src/share/watch.rs | a_link_needs_its_key | SHARE-13 | ported | |
| rs | src-tauri/src/share/watch.rs | names_the_relay_host | SHARE-13 | ported | |
