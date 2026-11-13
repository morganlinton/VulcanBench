"""calendar-workshop-update seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

/Users/Shared/Workshop/Workshop plan.txt holds Priya's draft (four sessions,
9:30 to 11:00 on 17, 19, 24 and 26 March). A launch agent
(~/Library/LaunchAgents/com.vulcanbench.workshop-correction.plist) writes
"Workshop plan - correction from Priya.txt" next to it 120 s after TextEdit
or Calendar first runs: every session runs 10:00 to 11:30, and session 3
moves to Wednesday 25 March. The clock starts at the agent's own first step
into the task, never at boot (docs/DECISIONS.md, 2026-11-08: mid-task events
are clocked from the agent's actions), so every agent meets the correction
at the same point in its work; the image build never opens either app, so
the correction first appears in a trial, and a run that never opens them
(nop) never gets it. /Users/Shared is outside the folders macOS privacy
protects, so neither the agent nor the launch agent meets a prompt there.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

STAMP = "2026-03-06 16:40"
DELAY_S = 120
PLAN = """Onboarding workshop plan (draft, Priya)

Four sessions for the March new hires. Put them in the Work calendar,
titled "Onboarding: session 1" to "Onboarding: session 4".

Session 1  Tuesday 17 March, 9:30 to 11:00    Welcome and tools
Session 2  Thursday 19 March, 9:30 to 11:00   Product deep dive
Session 3  Tuesday 24 March, 9:30 to 11:00    Customers and support
Session 4  Thursday 26 March, 9:30 to 11:00   Q&A with the team leads

Room: Harbor, 3rd floor.
"""
CORRECTION = """Correction to the onboarding plan (Priya)

Two changes, everything else stays as planned:

1. Every session now runs 10:00 to 11:30 instead of 9:30 to 11:00.
   The team leads have their standup until 10.
2. Session 3 moves to Wednesday 25 March. The 24th is the all-hands.
"""
FIRE = f"""#!/bin/bash
# Drops Priya's correction into the workshop folder {DELAY_S} s after TextEdit or
# Calendar first runs (the agent's first step into the task), once.
until /usr/bin/pgrep -x TextEdit >/dev/null || /usr/bin/pgrep -x Calendar >/dev/null; do sleep 2; done
sleep {DELAY_S}
f="/Users/Shared/Workshop/Workshop plan - correction from Priya.txt"
[ -e "$f" ] || /usr/bin/base64 -D -o "$f" <<'B64'
{{b64}}
B64
"""


def main(root: Path) -> None:
    import base64  # noqa: PLC0415

    shared = root / "Users" / "Shared"
    S.put(shared / "Workshop" / "Workshop plan.txt", STAMP, PLAN)
    S.stamp_dir(shared / "Workshop", STAMP)
    b64 = base64.b64encode(CORRECTION.encode()).decode()
    fire = S.put(shared / ".workshop" / "fire.sh", STAMP, FIRE.replace("{b64}", b64))
    fire.chmod(0o755)
    print("seeded plan and the correction script")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
