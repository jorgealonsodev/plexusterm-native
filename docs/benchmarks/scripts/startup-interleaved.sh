#!/usr/bin/env bash
# NFR-3 interleaved startup (docs/benchmarks/m0.md, T6c). usage: startup-interleaved.sh <rounds> "<label>|<VAR=value>|<bench binary>|<shell>" ...
# Runs every spec once per round, in order, with PLEXUS_EXIT_AFTER_FIRST_PROMPT=1, and prints the median
# exec -> first prompt frame presented per label. <VAR=value> is passed to env (use X=1 for none).
export LC_ALL=C
R=$1; shift
declare -A acc
for i in $(seq 1 "$R"); do
  for spec in "$@"; do
    IFS='|' read -r label envs bin sh <<<"$spec"
    t0=$(date +%s%N)
    out=$(env $envs SHELL=$sh PLEXUS_EXIT_AFTER_FIRST_PROMPT=1 "$bin" 2>&1 >/dev/null)
    m=$(echo "$out" | grep -o 'main at [0-9]*' | grep -o '[0-9]*$')
    pr=$(echo "$out" | grep -m1 'first prompt frame' | grep -o '[0-9]* µs' | grep -o '^[0-9]*')
    [ -n "$m" ] && [ -n "$pr" ] && acc[$label]+="$(( m - t0/1000 + pr )) "
    sleep 0.4
  done
done
for spec in "$@"; do IFS='|' read -r label _ <<<"$spec"; v=${acc[$label]}
  printf '%-28s n=%2d median %7.1f ms  min-max %s\n' "$label" "$(wc -w <<<"$v")" "$(printf '%s\n' $v | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]/1000}')" "$(printf '%s\n' $v | sort -n | awk 'NR==1{mn=$1} {mx=$1} END{printf "%.0f-%.0f", mn/1000, mx/1000}')"
done
