# notes-calls-to-calendar seed (guest, logged-in user). Today is Monday
# 9 March 2026 in New York, the day after the US moved to daylight time;
# the UK, Portugal and Germany move on 29 March, so this week London and
# Lisbon are four hours ahead of New York, not five. Maya's note lists six
# calls in other time zones, one tentative, and a later update moving one.
# The Work calendar already holds a daily standup and two meetings; a
# Personal calendar holds a lunch.
set -eu
osascript <<'OSA'
tell application "Notes"
  set acc to account "On My Mac"
  make new note at folder "Notes" of acc with properties {body:"<h1>Calls to set up</h1><div>From Maya, Friday 6 March</div><div><br></div><div>I have lined these up for the next two weeks. Times are in their time zones, so please convert them for your calendar.</div><ul><li>Intro call with Oliver Grant (London): Wednesday 11 March, 2:00 PM London time, 30 minutes</li><li>Budget sync with Ana Ribeiro (Lisbon): Tuesday 10 March, 4:00 PM Lisbon time, 45 minutes</li><li>Design review with Kenji Watanabe (Tokyo): Friday 13 March, 9:00 AM Tokyo time, 1 hour</li><li>Vendor demo with Priya Nair (Bengaluru): Wednesday 11 March, 8:30 PM India time, 1 hour</li><li>Quarterly check-in with Lena Vogel (Berlin): Thursday 19 March, 11:00 AM Berlin time, 30 minutes. Tentative: don't put this one on the calendar until she confirms.</li><li>Catch-up with Marcus Reed (San Francisco): Monday 16 March, 9:00 AM Pacific time, 30 minutes</li></ul><div><br></div><div>Update, Saturday 7 March: Ana can't make Tuesday anymore. Same time on Thursday instead.</div>"}
  make new note at folder "Notes" of acc with properties {body:"<h1>Groceries</h1><ul><li>Oat milk</li><li>Coffee beans</li><li>Lemons</li></ul>"}
  make new note at folder "Notes" of acc with properties {body:"<h1>Books to read</h1><div>The Overstory; Project Hail Mary; Piranesi</div>"}
  quit
end tell
tell application "Calendar"
  set W to make new calendar with properties {name:"Work"}
  set P to make new calendar with properties {name:"Personal"}
  make new event at end of events of W with properties {summary:"Team standup", start date:(date "Monday, March 2, 2026 at 9:30:00 AM"), end date:(date "Monday, March 2, 2026 at 9:45:00 AM"), recurrence:"FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,TU,WE,TH,FR"}
  make new event at end of events of W with properties {summary:"Roadmap review", start date:(date "Tuesday, March 10, 2026 at 2:00:00 PM"), end date:(date "Tuesday, March 10, 2026 at 3:00:00 PM")}
  make new event at end of events of W with properties {summary:"Quarterly planning review", start date:(date "Thursday, March 19, 2026 at 3:00:00 PM"), end date:(date "Thursday, March 19, 2026 at 4:00:00 PM")}
  make new event at end of events of P with properties {summary:"Lunch with Sofia", start date:(date "Friday, March 13, 2026 at 12:30:00 PM"), end date:(date "Friday, March 13, 2026 at 1:30:00 PM")}
  quit
end tell
OSA
echo seeded-notes-calendar
