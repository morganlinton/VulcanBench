# calendar-free-slots seed (guest, logged-in user). Today is Monday 9 March
# 2026; "next week" is Monday 16 to Friday 20 March. A daily standup
# (9:30 to 9:45) and a dense set of one-off meetings on Work, and a few
# Personal events, leave exactly one slot for each meeting:
#   Design workshop (90 min, 9 to 12, not Monday)  Wed 18, 9:45 to 11:15
#   Budget review (60 min, 1 to 5, its own day)     Thu 19, 2:00 to 3:00
#   1:1 with Priya (30 min, Friday, not 12 to 1)    Fri 20, 4:30 to 5:00
# (Wednesday 1 to 2 is also free for an hour, but the workshop is that day.)
set -eu
osascript <<'OSA'
on ev(cal, t, d, s, e)
  tell application "Calendar"
    make new event at end of events of calendar cal with properties {summary:t, start date:(date (d & " at " & s)), end date:(date (d & " at " & e))}
  end tell
end ev
tell application "Calendar"
  make new calendar with properties {name:"Work"}
  make new calendar with properties {name:"Personal"}
  make new event at end of events of calendar "Work" with properties {summary:"Team standup", start date:(date "Monday, March 2, 2026 at 9:30:00 AM"), end date:(date "Monday, March 2, 2026 at 9:45:00 AM"), recurrence:"FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,TU,WE,TH,FR"}
end tell
set mo to "Monday, March 16, 2026"
set tu to "Tuesday, March 17, 2026"
set we to "Wednesday, March 18, 2026"
set th to "Thursday, March 19, 2026"
set fr to "Friday, March 20, 2026"
ev("Work", "Quarterly kickoff", mo, "10:00:00 AM", "11:30:00 AM")
ev("Work", "Board prep", mo, "1:00:00 PM", "3:00:00 PM")
ev("Work", "Customer visit: Northwind", mo, "3:30:00 PM", "5:00:00 PM")
ev("Work", "Vendor call", tu, "10:30:00 AM", "11:00:00 AM")
ev("Work", "Offsite planning", tu, "1:30:00 PM", "4:30:00 PM")
ev("Personal", "School pickup", tu, "4:30:00 PM", "5:00:00 PM")
ev("Work", "Interview: backend candidate", we, "11:15:00 AM", "12:00:00 PM")
ev("Work", "Security review", we, "2:00:00 PM", "4:00:00 PM")
ev("Work", "Roadmap review", th, "10:00:00 AM", "11:00:00 AM")
ev("Work", "Design sync", th, "1:00:00 PM", "2:00:00 PM")
ev("Work", "Hiring panel", th, "3:00:00 PM", "5:00:00 PM")
ev("Personal", "Breakfast with Sam", fr, "8:30:00 AM", "9:30:00 AM")
ev("Work", "Quarterly planning", fr, "9:45:00 AM", "12:00:00 PM")
ev("Work", "Customer workshop", fr, "1:00:00 PM", "4:00:00 PM")
ev("Personal", "Pharmacy pickup", fr, "4:00:00 PM", "4:30:00 PM")
ev("Personal", "Dinner with Alex", th, "7:00:00 PM", "9:00:00 PM")
tell application "Calendar" to quit
OSA
echo seeded-calendar
