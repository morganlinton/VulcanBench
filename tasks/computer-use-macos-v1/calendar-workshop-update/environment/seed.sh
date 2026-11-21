# calendar-workshop-update seed (guest, logged-in user). Work and Personal
# calendars; the all-hands on Tuesday 24 March is why session 3 moves. Last,
# the launch agent that drops Priya's correction (see seed_fs.py), written
# after Calendar has quit so the seed's own Calendar run cannot set it off.
set -eu
osascript <<'OSA'
tell application "Calendar"
  make new calendar with properties {name:"Work"}
  make new calendar with properties {name:"Personal"}
  make new event at end of events of calendar "Work" with properties {summary:"All-hands", start date:(date "Tuesday, March 24, 2026 at 9:00:00 AM"), end date:(date "Tuesday, March 24, 2026 at 12:00:00 PM")}
  make new event at end of events of calendar "Work" with properties {summary:"Hiring sync", start date:(date "Wednesday, March 18, 2026 at 2:00:00 PM"), end date:(date "Wednesday, March 18, 2026 at 2:30:00 PM")}
  make new event at end of events of calendar "Personal" with properties {summary:"Dentist", start date:(date "Thursday, March 19, 2026 at 4:00:00 PM"), end date:(date "Thursday, March 19, 2026 at 5:00:00 PM")}
  quit
end tell
OSA
mkdir -p "/Users/admin/Library/LaunchAgents"
cat > "/Users/admin/Library/LaunchAgents/com.vulcanbench.workshop-correction.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.vulcanbench.workshop-correction</string>
  <key>ProgramArguments</key><array><string>/bin/bash</string><string>/Users/Shared/.workshop/fire.sh</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
PLIST
plutil -lint "/Users/admin/Library/LaunchAgents/com.vulcanbench.workshop-correction.plist"
echo seeded-calendar-and-launch-agent
