# reminders-renovation-plan seed (guest, logged-in user). A Kitchen
# renovation list of nine reminders and a Personal list that must not
# change. (Local, non-iCloud Reminders lists cannot hold subtasks, so the
# task moves reminders to a new list instead.)
set -eu
osascript <<'OSA'
tell application "Reminders"
  set K to make new list with properties {name:"Kitchen renovation"}
  repeat with t in {"Get three contractor quotes", "Choose a contractor", "Order cabinets", "Order countertop", "Order sink and faucet", "Book electrician for new outlets", "Pick paint color", "Schedule final inspection", "Clear out the pantry"}
    make new reminder at end of K with properties {name:(contents of t)}
  end repeat
  set P to make new list with properties {name:"Personal"}
  make new reminder at end of P with properties {name:"Renew passport", flagged:true}
  make new reminder at end of P with properties {name:"Call the dentist", priority:5}
  quit
end tell
OSA
echo seeded-reminders
