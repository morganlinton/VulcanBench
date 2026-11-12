# notes-folder-cleanup seed (guest, logged-in user). Twenty notes in the
# default Notes folder and three empty folders (Recipes, Travel, Work):
# four recipes, four travel notes, four work notes, three empty notes and
# five others that stay. Two of the others look like they belong in a folder
# (a dinner party guest list, a garden planting schedule) and one travel note
# is a list of restaurants, not a recipe.
set -eu
osascript <<'OSA'
on nn(t, b)
  tell application "Notes"
    make new note at folder "Notes" of account "On My Mac" with properties {body:"<h1>" & t & "</h1>" & b}
  end tell
end nn
tell application "Notes"
  set acc to account "On My Mac"
  make new folder at acc with properties {name:"Recipes"}
  make new folder at acc with properties {name:"Travel"}
  make new folder at acc with properties {name:"Work"}
end tell
nn("Lemon ricotta pancakes", "<div>2 eggs, 1 cup ricotta, zest of a lemon, 3/4 cup flour. Cook on medium heat.</div>")
nn("Tokyo restaurant list", "<ul><li>Sushi Saito (book a month ahead)</li><li>Afuri ramen, Ebisu</li><li>Tsuta, Yoyogi-Uehara</li></ul>")
nn("1:1 notes with Priya", "<div>Career goals: lead the onboarding project. Follow up on the training budget.</div>")
nn("Gift ideas for Mom", "<ul><li>Pottery class</li><li>Garden tools</li></ul>")
tell application "Notes" to make new note at folder "Notes" of account "On My Mac" with properties {body:"<div><br></div>"}
nn("Weeknight chili", "<div>1 lb ground turkey, 2 cans beans, 1 can tomatoes, chili powder, cumin. Simmer 30 minutes.</div>")
nn("Packing list: Japan", "<ul><li>Passport</li><li>Rail pass voucher</li><li>Power adapter (Type A)</li></ul>")
nn("Q2 hiring plan", "<div>Two backend engineers, one designer. Budget approved by finance on 3 March.</div>")
nn("Dinner party guest list", "<ul><li>Sofia and Alex</li><li>Kwame</li><li>Lucy (vegetarian)</li></ul>")
nn("Grandma's banana bread", "<div>3 ripe bananas, 1/3 cup melted butter, 3/4 cup sugar, 1 egg, 1 1/2 cups flour. 350F for 60 minutes.</div>")
nn("Lisbon trip ideas", "<ul><li>Tram 28</li><li>Day trip to Sintra</li><li>Pasteis de Belem</li></ul>")
tell application "Notes" to make new note at folder "Notes" of account "On My Mac" with properties {body:"<div><br></div>"}
nn("Offsite agenda draft", "<div>Day 1: strategy. Day 2: team workshops. Dinner at 7.</div>")
nn("Books to read", "<div>The Overstory; Piranesi; Project Hail Mary</div>")
nn("Miso salmon", "<div>Marinate salmon in white miso, mirin and soy for 1 hour. Broil 8 minutes.</div>")
nn("Rental car confirmation", "<div>Hertz, Denver airport, 14 to 18 April. Confirmation K8812093.</div>")
nn("Vendor contract questions", "<div>Termination notice period? Data deletion on exit? Price cap on renewal?</div>")
nn("Garden planting schedule", "<div>Tomatoes after the last frost (early May). Basil in June.</div>")
tell application "Notes" to make new note at folder "Notes" of account "On My Mac" with properties {body:"<div><br></div>"}
nn("Wedding RSVP list", "<div>Respond by 1 April. Plus one: yes.</div>")
tell application "Notes" to quit
OSA
echo seeded-notes
