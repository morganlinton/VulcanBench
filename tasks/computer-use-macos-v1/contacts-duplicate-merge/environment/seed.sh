# contacts-duplicate-merge seed (guest, logged-in user). Twenty-two cards
# besides the owner's own: four people are on two cards each, their details
# split between the cards (two pairs share an exact name, two do not: Bob and
# Robert Alvarez share an email address, Jen and Jennifer Okafor a mobile
# number); two different people are both named Chris Park, and Sam and
# Samantha Patel are different people too. Ten others are distractors.
set -eu
osascript <<'OSA'
on person(fn, ln, org, ttl, nt)
  tell application "Contacts"
    set p to make new person with properties {first name:fn, last name:ln}
    if org is not "" then set organization of p to org
    if ttl is not "" then set job title of p to ttl
    if nt is not "" then set note of p to nt
    return p
  end tell
end person
on ph(p, lbl, v)
  tell application "Contacts" to make new phone at end of phones of p with properties {label:lbl, value:v}
end ph
on em(p, lbl, v)
  tell application "Contacts" to make new email at end of emails of p with properties {label:lbl, value:v}
end em

-- Duplicated people: one person each, two cards each.
set a to person("Maria", "Gonzalez", "", "", "")
ph(a, "mobile", "(312) 555-0147")
em(a, "home", "maria.gonzalez@example.com")
set b to person("Maria", "Gonzalez", "Fieldstone Realty", "Broker", "")
ph(b, "work", "(312) 555-0190")
em(b, "work", "mgonzalez@fieldstone.example")

set a to person("Tom", "Becker", "", "", "Organizes the spring kayak trip.")
ph(a, "mobile", "646-555-0112")
set b to person("Tom", "Becker", "", "", "")
ph(b, "mobile", "646-555-0112")
em(b, "home", "tom.becker@example.net")

set a to person("Robert", "Alvarez", "Alvarez Plumbing", "Owner", "")
ph(a, "work", "503-555-0175")
em(a, "work", "bob@alvarezplumbing.example")
set b to person("Bob", "Alvarez", "", "", "Fixed the water heater, March 2025.")
ph(b, "mobile", "503-555-0133")
em(b, "work", "bob@alvarezplumbing.example")

set a to person("Jennifer", "Okafor", "Brightline Health", "Product Manager", "")
ph(a, "mobile", "404-555-0128")
em(a, "work", "jokafor@brightline.example")
set b to person("Jen", "Okafor", "", "", "")
ph(b, "mobile", "(404) 555-0128")
em(b, "home", "jen.okafor@example.org")

-- Same name, different people.
set a to person("Chris", "Park", "Northwind Labs", "CTO", "")
ph(a, "mobile", "(415) 555-0134")
em(a, "work", "chris@northwind.example")
set b to person("Chris", "Park", "Harbor Dental", "Dentist", "Cleaning every six months.")
ph(b, "work", "617-555-0199")
em(b, "work", "office@harbordental.example")

set a to person("Sam", "Patel", "Patel Cycles", "", "")
ph(a, "work", "207-555-0110")
em(a, "work", "sam@patelcycles.example")
set b to person("Samantha", "Patel", "", "", "Neighbor at number 14.")
ph(b, "mobile", "617-555-0186")
em(b, "home", "samantha.patel@example.com")

-- Everyone else.
set a to person("Elena", "Novak", "", "", "")
ph(a, "mobile", "(212) 555-0161")
em(a, "home", "elena.novak@example.com")
set a to person("Kwame", "Mensah", "Mensah & Co", "Accountant", "")
ph(a, "work", "617-555-0123")
em(a, "work", "kwame@mensahco.example")
set a to person("Lucy", "Zhang", "", "", "")
ph(a, "mobile", "(857) 555-0109")
set a to person("Omar", "Haddad", "", "", "")
ph(a, "mobile", "(617) 555-0172")
em(a, "home", "omar.haddad@example.net")
set a to person("Grace", "Liu", "Liu Design", "", "")
em(a, "work", "grace@liudesign.example")
set a to person("Henrik", "Larsen", "", "", "")
ph(a, "mobile", "+45 20 55 01 44")
set a to person("Priya", "Shah", "", "", "")
ph(a, "mobile", "(781) 555-0155")
em(a, "home", "priya.shah@example.org")
set a to person("Diego", "Torres", "", "", "")
ph(a, "home", "(617) 555-0141")
set a to person("Nina", "Petrov", "", "", "")
em(a, "home", "nina.petrov@example.com")
set a to person("Alex", "Morgan", "", "", "")
ph(a, "mobile", "(339) 555-0117")

tell application "Contacts"
  save
  quit
end tell
OSA
echo seeded-contacts
