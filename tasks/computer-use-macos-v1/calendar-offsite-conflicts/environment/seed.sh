# calendar-offsite-conflicts seed (guest, logged-in user). Today is Monday
# 9 March 2026. A Work calendar with three weekly 1:1 series, a daily standup
# and a two-day offsite on Tuesday 17 and Wednesday 18 March; a Personal
# calendar with one event the same week.
set -eu
osascript <<'OSA'
tell application "Calendar"
  set W to make new calendar with properties {name:"Work"}
  set P to make new calendar with properties {name:"Personal"}
  make new event at end of events of W with properties {summary:"1:1 with Alex", start date:(date "Tuesday, March 3, 2026 at 10:00:00 AM"), end date:(date "Tuesday, March 3, 2026 at 10:30:00 AM"), recurrence:"FREQ=WEEKLY;INTERVAL=1"}
  make new event at end of events of W with properties {summary:"1:1 with Priya", start date:(date "Wednesday, March 4, 2026 at 2:00:00 PM"), end date:(date "Wednesday, March 4, 2026 at 2:30:00 PM"), recurrence:"FREQ=WEEKLY;INTERVAL=1"}
  make new event at end of events of W with properties {summary:"1:1 with Sam", start date:(date "Thursday, March 5, 2026 at 9:00:00 AM"), end date:(date "Thursday, March 5, 2026 at 9:30:00 AM"), recurrence:"FREQ=WEEKLY;INTERVAL=1"}
  make new event at end of events of W with properties {summary:"Team standup", start date:(date "Monday, March 2, 2026 at 9:30:00 AM"), end date:(date "Monday, March 2, 2026 at 9:45:00 AM"), recurrence:"FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,TU,WE,TH,FR"}
  make new event at end of events of W with properties {summary:"Leadership offsite", start date:(date "Tuesday, March 17, 2026 at 12:00:00 AM"), end date:(date "Thursday, March 19, 2026 at 12:00:00 AM"), allday event:true}
  make new event at end of events of W with properties {summary:"Quarterly planning review", start date:(date "Thursday, March 19, 2026 at 3:00:00 PM"), end date:(date "Thursday, March 19, 2026 at 4:00:00 PM")}
  make new event at end of events of P with properties {summary:"Dentist", start date:(date "Wednesday, March 18, 2026 at 8:00:00 AM"), end date:(date "Wednesday, March 18, 2026 at 8:45:00 AM")}
  quit
end tell
OSA
echo seeded-calendar
