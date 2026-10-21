#!/usr/bin/env bash
# NFR-3 startup phases (docs/benchmarks/m0.md, T6b). usage: startup-phases.sh <bench binary or floor_window> <runs> [shell]
# Runs the binary <runs> times with PLEXUS_EXIT_AFTER_FIRST_PROMPT=1 and prints the median of every phase
# (ms since main), exec -> main, exec -> first prompt frame presented, and wall time including teardown.
export LC_ALL=C
BIN=$1 RUNS=$2 SH=${3:-$SHELL}
declare -A acc
for i in $(seq 1 "$RUNS"); do
  t0=$(date +%s%N)
  out=$(SHELL=$SH PLEXUS_EXIT_AFTER_FIRST_PROMPT=1 "$BIN" 2>&1 >/dev/null)
  t1=$(date +%s%N)
  m=$(echo "$out" | grep -o 'main at [0-9]*' | grep -o '[0-9]*$')
  pr=$(echo "$out" | grep -m1 'first prompt frame' | grep -o '[0-9]* µs' | grep -o '^[0-9]*')
  e2m=$(( m - t0/1000 ))
  acc[exec_to_main]+="$e2m "
  acc[exec_to_prompt]+="$(( e2m + pr )) "
  acc[wall_incl_exit]+="$(( (t1-t0)/1000 )) "
  while read -r name us; do acc[$name]+="$us "; done < <(echo "$out" | sed -n 's/^plexusterm: phase \([a-z_]*\) \([0-9]*\) µs$/\1 \2/p' | awk '!seen[$1]++')
  sleep 0.4
done
for k in exec_to_main pty_spawned app_created run_callback view_new font_measured first_render first_paint first_paint_done resize_refresh window_opened first_output_in_view first_wake first_frame_submitted exec_to_prompt wall_incl_exit; do
  v=${acc[$k]:-}; [ -z "$v" ] && continue
  printf '%-22s median %7.1f ms  (%s)\n' "$k" "$(printf '%s\n' $v | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]/1000}')" "$(printf '%s\n' $v | sort -n | awk 'NR==1{mn=$1} {mx=$1} END{printf "%.0f-%.0f", mn/1000, mx/1000}')"
done
