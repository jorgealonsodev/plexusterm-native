#!/usr/bin/env bash
# usage: typekeys.sh <app binary> <log folder> <keys> [load-procs]
# Types <keys> keystrokes into the PlexusTerm window of the app it starts, checking before every
# key that the focused window is that window (owned by the app's pid, named exactly PlexusTerm).
set -u
APP=$1 DIR=$2 KEYS=$3 LOAD=${4:-0}
rm -rf "$DIR"; mkdir -p "$DIR"
LOADPIDS=()
for _ in $(seq 1 "$LOAD"); do yes > /dev/null & LOADPIDS+=($!); done
PLEXUS_LATENCY_LOG="$DIR" "$APP" 2>/dev/null & PID=$!
cleanup() { kill "$PID" 2>/dev/null; for p in "${LOADPIDS[@]}"; do kill "$p" 2>/dev/null; done; }
trap cleanup EXIT
sleep 3
WID=""
for w in $(xdotool search --pid "$PID" 2>/dev/null); do
  if [ "$(xdotool getwindowpid "$w" 2>/dev/null)" = "$PID" ] && [ "$(xdotool getwindowname "$w")" = "PlexusTerm" ]; then WID=$w; fi
done
[ -n "$WID" ] || { echo "no PlexusTerm window owned by pid $PID; not typing"; exit 1; }
xdotool windowactivate --sync "$WID" || { echo "activate failed; not typing"; exit 1; }
sleep 0.5
echo "active=$(xdotool getactivewindow) focus=$(xdotool getwindowfocus) wid=$WID"
grep VmRSS /proc/$PID/status
letters=abcdefghijklmnopqrstuvwxyz
for i in $(seq 0 $((KEYS - 1))); do
  if [ "$(xdotool getactivewindow 2>/dev/null)" != "$WID" ] || [ "$(xdotool getwindowfocus 2>/dev/null)" != "$WID" ] || [ "$(xdotool getwindowpid "$WID" 2>/dev/null)" != "$PID" ]; then
    echo "focus left the PlexusTerm window after $i keys; stopped typing"; exit 2
  fi
  if [ $((i % 60)) -eq 59 ]; then k=ctrl+u; else k=${letters:$((i % 26)):1}; fi
  xdotool key --delay 0 "$k"
  sleep 0.08
done
sleep 1
echo "typed $KEYS keys into window $WID (pid $PID)"
