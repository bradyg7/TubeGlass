#!/usr/bin/env bash
# Publishes the tail of a log file as an error annotation on the run, so a failure can be read
# from the run's summary page and the public check-run API without downloading logs.
#   annotate.sh <title> <logfile>
set -uo pipefail
[[ -f "${2:-}" ]] || exit 0
msg=$(grep -vE '^\s+(Compiling|Downloaded|Downloading|Fresh|Running)' "$2" | tail -n 70 | sed 's/%/%25/g; s/\r//g' | awk '{printf "%s%%0A", $0}')
echo "::error title=$1::${msg:0:60000}"
