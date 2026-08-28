#!/usr/bin/env python3
"""Frama-C Eva -> FDML facts adapter.

Runs Eva's value analysis on a bounded set of C files and emits the unified facts
schema on stdout (see docs/facts-provider.md). Nothing here is imported by the FDML
binary: the contract is the JSON document, so replacing this with a Joern, gopls or
clang-query adapter needs no change to FDML itself.

    adapters/frama_c_eva.py src/vehicle/physics.c > facts.json
    fdml facts --import facts.json --provider frama-c-eva

Eva returns an over-approximation: a value set means "the analyser could not rule
these out", not "the program produced these". Every fact is emitted with
confidence=static-overapproximation and must stay labelled that way downstream.
"""
import json
import re
import shutil
import subprocess
import sys

# "  x ∈ {0; 1}"  /  "  p ∈ {{ &buf + [0..12] }}"  /  "  n ∈ [--..--]"
VALUE_LINE = re.compile(r"^\s{2,}([A-Za-z_][A-Za-z0-9_.\[\]]*)\s+∈\s+(.+?)\s*$")
FUNCTION_HEADER = re.compile(r"^\[eva[^\]]*\]\s+Values at end of function ([A-Za-z_][A-Za-z0-9_]*):")
# "file.c:12:[eva] warning: ..." — an alarm is a claim about a location, not a value
ALARM_LINE = re.compile(r"^([^:]+):(\d+):\[eva\]\s+warning:\s*(.+?)\s*$")


import os

# The macOS .app ships its share/ under Contents/Resources but looks for it under
# Contents/, and ignores FRAMAC_SHARE — point FRAMA_C at a shadow bundle in that case.
FRAMA_C = os.environ.get("FRAMA_C", "frama-c")


def run_eva(paths, extra_args, timeout):
    if not shutil.which(FRAMA_C) and not os.path.isfile(FRAMA_C):
        sys.exit(f"{FRAMA_C} not found. Install Frama-C, or set FRAMA_C to its binary.")
    # Only the end-of-function value tables are consumed; per-statement output is noise.
    cmd = [FRAMA_C, "-eva", *extra_args, *paths]
    try:
        done = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        sys.exit(f"Eva exceeded {timeout}s on {' '.join(paths)}. Narrow the input or lower -eva-slevel.")
    # Eva reports analysis problems on stderr but still produces usable values, so we
    # only fail hard when there is no output at all to parse.
    if not done.stdout.strip():
        sys.exit(f"Eva produced no output.\n{done.stderr[-2000:]}")
    return done.stdout


def parse(output):
    values, alarms, current = {}, [], None
    for line in output.splitlines():
        header = FUNCTION_HEADER.match(line)
        if header:
            current = header.group(1)
            continue
        alarm = ALARM_LINE.match(line)
        if alarm:
            alarms.append({"function": current or "<global>", "file": alarm.group(1),
                           "line": int(alarm.group(2)), "message": alarm.group(3)})
            continue
        if current:
            value = VALUE_LINE.match(line)
            if value:
                name, raw = value.group(1), value.group(2)
                target = f"{current}.{name}" if "." not in name else name
                entry = values.setdefault(target, {"function": current, "variable": name})
                entry["possible_values"] = raw
                if raw.startswith("{{"):
                    entry["pointer_targets"] = raw
        if line.startswith("[eva] done") or line.startswith("[eva:summary]"):
            current = None
    return values, alarms


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    timeout = 300
    if "--timeout" in args:
        i = args.index("--timeout")
        timeout = int(args[i + 1])
        del args[i:i + 2]
    # Inputs are the arguments that name existing files; everything else — including
    # flag values like `-eva-precision 1` — is passed through to Frama-C untouched.
    paths = [a for a in args if os.path.isfile(a)]
    extra = [a for a in args if a not in paths]
    if not paths:
        sys.exit("give at least one existing C file")

    values, alarms = parse(run_eva(paths, extra, timeout))
    json.dump({
        "provider": "frama-c-eva",
        "confidence": "static-overapproximation",
        "values": values,
        "alarms": alarms,
    }, sys.stdout, indent=2, ensure_ascii=False)
    print()


if __name__ == "__main__":
    main()
