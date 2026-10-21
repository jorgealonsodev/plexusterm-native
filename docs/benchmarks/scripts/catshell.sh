#!/bin/sh
# Used as $SHELL for the in-app `cat` measurement (docs/benchmarks/m0.md).
# Needs CORPUS (the 100 MiB file) and MARKS (a folder for the start/end times) in the environment.
sleep 2
date +%s%N > "$MARKS/cat-start"
cat "$CORPUS"
date +%s%N > "$MARKS/cat-end"
sleep 1
