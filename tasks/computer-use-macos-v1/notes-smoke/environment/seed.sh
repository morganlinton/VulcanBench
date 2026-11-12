# notes-smoke seed: runs in the guest (build_task_image.py) as the logged-in
# user. Two folders besides the default one, four existing notes.
set -eu
osascript <<'OSA'
tell application "Notes"
  set acc to account "On My Mac"
  make new folder at acc with properties {name:"Trips"}
  make new folder at acc with properties {name:"Work"}
  make new note at folder "Trips" of acc with properties {body:"<h1>Lisbon itinerary</h1><div>Fri: Alfama walk, dinner at 8</div><div>Sat: Belem, tram 15</div>"}
  make new note at folder "Trips" of acc with properties {body:"<h1>Hotel confirmations</h1><div>Casa do Rio, booking 48213</div>"}
  make new note at folder "Work" of acc with properties {body:"<h1>Q2 planning</h1><div>Draft the hiring plan by Friday</div>"}
  make new note at folder "Notes" of acc with properties {body:"<h1>Groceries</h1><div>oat milk, lemons, coffee</div>"}
  quit
end tell
OSA
echo seeded-notes
