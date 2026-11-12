# notes-actions-to-reminders seed (guest, logged-in user): three of today's
# meeting notes in a Meetings folder, an older note, and a Personal reminders
# list that must be left alone. Today is Monday 9 March 2026.
set -eu
osascript <<'OSA'
tell application "Notes"
  set acc to account "On My Mac"
  make new folder at acc with properties {name:"Meetings"}
  make new note at folder "Meetings" of acc with properties {body:"<h1>Ops weekly</h1><div>Mon 9 Mar, 8:30. Jordan, Elena, Sam</div><div><br></div><div>Status</div><ul><li>Staging cert expires 22 Mar</li><li>On-call handover went fine</li></ul><div>Actions</div><ul><li>Jordan: renew the staging SSL certificate by the 20th</li><li>Elena: rotate the on-call schedule for April</li><li>Done: Jordan archived the old incident reports</li></ul>"}
  make new note at folder "Meetings" of acc with properties {body:"<h1>Product sync</h1><div>Mon 9 Mar, 10:00. Jordan, Priya, Marcus, Elena</div><div><br></div><div>Notes</div><ul><li>Onboarding funnel drops at step 3 (41% to 23%)</li><li>Pricing page test runs until the 18th</li></ul><div>Action items</div><ul><li>Jordan: send the onboarding funnel numbers to Priya by Thursday</li><li>Marcus: fix the trial-expiry email copy</li><li>Jordan: book a usability session with two enterprise customers next Monday</li><li>Elena: draft the Q2 pricing memo by Friday</li></ul>"}
  make new note at folder "Meetings" of acc with properties {body:"<h1>Hiring review</h1><div>Mon 9 Mar, 11:30. Jordan, Priya, Marcus</div><div><br></div><ul><li>Jordan to collect interview feedback for the backend role by end of day tomorrow</li><li>Priya to post the product designer role this week</li><li>Interview rubric update: Jordan and Marcus, owner Marcus, by Friday</li><li>Jordan to schedule the debrief with the hiring panel for Wednesday 18 March</li></ul>"}
  make new note at folder "Notes" of acc with properties {body:"<h1>Product sync (last week)</h1><div>Mon 2 Mar</div><ul><li>Jordan: share the churn dashboard by Thursday</li></ul>"}
  quit
end tell
tell application "Reminders"
  set P to make new list with properties {name:"Personal"}
  make new reminder at end of P with properties {name:"Pick up dry cleaning", due date:(date "Tuesday, March 10, 2026 at 6:00:00 PM")}
  make new reminder at end of P with properties {name:"Call the dentist"}
  quit
end tell
OSA
echo seeded-notes-reminders
