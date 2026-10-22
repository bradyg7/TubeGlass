#!/usr/bin/env bash
# Publishes the important part of a log as an error annotation on the run, so a failure can be
# read from the run's summary page and the public check-run API without downloading logs.
# (GitHub keeps about 4000 characters of an annotation, so only the failures are kept.)
#   annotate.sh <title> <logfile>
set -uo pipefail
[[ -f "${2:-}" ]] || exit 0
clean=$(sed 's/\r//g' "$2")
{
  # compiler errors and failed tests first, then the very end of the log
  echo "$clean" | grep -nE '^error|^---- |panicked at|FAILED|failures:|assertion|^\s+(left|right):|could not compile|linking with' | head -40
  echo "..."
  echo "$clean" | tail -n 12
} | cut -c1-220 | sed 's/%/%25/g' | awk '{printf "%s%%0A", $0}' > /tmp/annotation.txt
msg=$(cat /tmp/annotation.txt)
echo "::error title=$1::${msg:0:3900}"
