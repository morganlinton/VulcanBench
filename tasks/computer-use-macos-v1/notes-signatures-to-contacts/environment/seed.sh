# notes-signatures-to-contacts seed (guest, logged-in user). A note with
# three pasted email signatures, each laid out differently (labels M/O,
# Mobile/Office, cell/direct), and a few existing contacts that must stay.
set -eu
osascript <<'OSA'
tell application "Notes"
  set acc to account "On My Mac"
  make new note at folder "Notes" of acc with properties {body:"<h1>Conference contacts</h1><div>People from the HealthTech Summit, 5 March. Add them to Contacts.</div><div><br></div><div>--</div><div>Priyanka Desai | Director of Partnerships | Lumen Health</div><div>M: (415) 555-0132 | O: (415) 555-0100</div><div>priyanka.desai@lumenhealth.example</div><div>500 Mission St, Suite 1200, San Francisco, CA 94105</div><div><br></div><div>--</div><div>Best,</div><div>Tomas Reyes</div><div>Senior Data Engineer, Northgate Analytics</div><div>Office 312-555-0177 / Mobile 312-555-0119</div><div>treyes@northgate.example</div><div><br></div><div>--</div><div>Grace Kim</div><div>VP Operations</div><div>Cedar &amp; Pine Clinics</div><div>direct: 617.555.0164 | cell: 617.555.0108</div><div>grace.kim@cedarpine.example</div><div>88 Harrison Ave, Boston, MA 02111</div>"}
  make new note at folder "Notes" of acc with properties {body:"<h1>Summit takeaways</h1><ul><li>Everyone wants FHIR exports</li><li>Follow up with Lumen about the pilot</li></ul>"}
  quit
end tell
tell application "Contacts"
  set p to make new person with properties {first name:"Dana", last name:"Whitfield", organization:"Harbor Analytics"}
  make new email at end of emails of p with properties {label:"work", value:"dana@harboranalytics.example"}
  set q to make new person with properties {first name:"Kwame", last name:"Mensah"}
  make new phone at end of phones of q with properties {label:"mobile", value:"(617) 555-0123"}
  save
  quit
end tell
OSA
echo seeded-notes-contacts
