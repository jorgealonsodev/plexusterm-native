#!/usr/bin/env bash
# NFR-4 memory breakdown (docs/benchmarks/m0.md, T6b). usage: rss-breakdown.sh <binary> [secs] [shell]
# Starts the binary, waits <secs> (10), prints VmRSS, smaps_rollup, Rss per mapping >= 1 MiB and the
# nvidia-smi GPU memory of the process, then kills it.
BIN=$1 SECS=${2:-10} SH=${3:-$SHELL}
SHELL=$SH "$BIN" >/dev/null 2>&1 & PID=$!
sleep "$SECS"
grep -E 'VmRSS|RssAnon|RssFile|RssShmem|VmHWM' /proc/$PID/status
grep -E '^(Rss|Pss|Anonymous|Private_Dirty|Shared_Clean|Private_Clean)' /proc/$PID/smaps_rollup
awk '/^[0-9a-f]+-[0-9a-f]+ /{name=$6; if(name=="")name="[anon]"} /^Rss:/{r[name]+=$2} END{for(n in r) if(r[n]>=1024) printf "%8d kB %s\n", r[n], n}' /proc/$PID/smaps | sort -rn | head -${TOP:-25}
nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader 2>/dev/null | grep "^$PID" || nvidia-smi 2>/dev/null | grep " $PID " 
kill $PID; wait $PID 2>/dev/null
