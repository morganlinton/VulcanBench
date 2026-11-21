# reminders-clear-completed seed (guest, logged-in user). A Work list with
# five open reminders, six completed ones to delete and three completed
# audit reminders to keep, and a Personal list (four completed, two open)
# that must not change. Completed reminders are hidden until Show Completed.
set -eu
osascript <<'OSA'
tell application "Reminders"
  set W to make new list with properties {name:"Work"}
  repeat with t in {"Prepare March board slides", "Review Q2 hiring plan", "Book offsite venue", "Reply to Priya about the budget", "Renew design software licenses"}
    make new reminder at end of W with properties {name:(contents of t)}
  end repeat
  repeat with t in {"Submit February timesheet", "Send onboarding pack to Marcus", "Order new monitor", "Update the team wiki page", "Schedule 1:1 with Elena", "Cancel the old Zoom plan", "Send expense receipts to the auditors", "Export January bank statements for the audit", "Sign the audit engagement letter"}
    make new reminder at end of W with properties {name:(contents of t), completed:true}
  end repeat
  set P to make new list with properties {name:"Personal"}
  repeat with t in {"Pay the water bill", "Return library books", "Buy a birthday card for Mom", "Book the car service"}
    make new reminder at end of P with properties {name:(contents of t), completed:true}
  end repeat
  make new reminder at end of P with properties {name:"Renew passport"}
  make new reminder at end of P with properties {name:"Call the dentist"}
  quit
end tell
OSA
echo seeded-reminders
