# calendar-trip-timezones seed (guest, logged-in user). Work and Travel
# calendars with a few events that must not change, and a Travel reminders
# list with one existing item.
set -eu
osascript <<'OSA'
tell application "Calendar"
  make new calendar with properties {name:"Work"}
  make new calendar with properties {name:"Travel"}
  make new event at end of events of calendar "Work" with properties {summary:"Quarterly kickoff", start date:(date "Monday, March 16, 2026 at 10:00:00 AM"), end date:(date "Monday, March 16, 2026 at 11:30:00 AM")}
  make new event at end of events of calendar "Work" with properties {summary:"Remote: Lisbon team sync", start date:(date "Wednesday, March 18, 2026 at 9:00:00 AM"), end date:(date "Wednesday, March 18, 2026 at 9:30:00 AM")}
  make new event at end of events of calendar "Travel" with properties {summary:"Passport photo appointment", start date:(date "Thursday, March 12, 2026 at 4:00:00 PM"), end date:(date "Thursday, March 12, 2026 at 4:30:00 PM")}
  quit
end tell
tell application "Reminders"
  set T to make new list with properties {name:"Travel"}
  make new reminder at end of T with properties {name:"Buy a travel adapter"}
  quit
end tell
OSA
echo seeded-calendar-reminders
