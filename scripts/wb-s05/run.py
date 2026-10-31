# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-05: the same as run.sh, started as `python3 -I run.py …`.

Runs `/bin/bash run.sh …` (bash reads the script as data). Use it when
macOS's syspolicyd is unavailable: a script file created on this Mac carries
`com.apple.provenance`, and executing it directly then waits for a policy
check that never comes (seen 2026-10-31 while syspolicyd crash-looped).
"""
import os
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
script = os.path.join(here, sys.argv[1] if len(sys.argv) > 1 and sys.argv[1].endswith(".sh") else "run.sh")
args = sys.argv[2:] if len(sys.argv) > 1 and sys.argv[1].endswith(".sh") else sys.argv[1:]
sys.exit(subprocess.call(["/bin/bash", script, *args]))
