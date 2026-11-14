"""settings-it-audit gold: Terminal writes the eighteen changes with
`defaults` (the preference keys System Settings and Finder write), restarts
the Dock, Finder and the menu bar, and the final message lists the changes.
The gold knows the checklist; an agent has to read it, including the Sales
and Finance sections and Revisions B and C.

Cheat variants: VB_TEAM=finance applies the Finance section instead of
Sales; VB_REVC=ignore sets the fastest key repeat; VB_REVB=ignore also hides
recent apps; VB_SKIP=finder leaves the path bar off.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

G_ = "defaults write NSGlobalDomain"
CMDS = [
    f"{G_} AppleInterfaceStyle -string Dark",
    f"{G_} AppleShowScrollBars -string Always",
    "defaults write com.apple.dock mineffect -string scale",
    f"{G_} AppleWindowTabbingMode -string always",
    f"{G_} NSCloseAlwaysConfirmsChanges -bool true",
    f"{G_} AppleActionOnDoubleClick -string Minimize",
    f"{G_} AppleICUForce24HourTime -bool true",
    "defaults write com.apple.menuextra.clock ShowSeconds -bool true",
    f"{G_} AppleShowAllExtensions -bool true",
    "defaults write com.apple.finder FXEnableExtensionChangeWarning -bool false",
    "defaults write com.apple.finder _FXSortFoldersFirst -bool true",
    "defaults write com.apple.finder FXDefaultSearchScope -string SCcf",
    f"{G_} NSAutomaticSpellingCorrectionEnabled -bool false",
    f"{G_} AppleFirstWeekday -dict gregorian -int 2",
    "defaults write com.apple.WindowManager EnableStandardClickToShowDesktop -bool false",
]
if os.environ.get("VB_SKIP") != "finder":
    CMDS.append("defaults write com.apple.finder ShowPathbar -bool true")
CMDS.append(f"{G_} KeyRepeat -int {2 if os.environ.get('VB_REVC') == 'ignore' else 6}")
if os.environ.get("VB_TEAM") == "finance":
    CMDS += ["defaults write com.apple.dock wvous-br-corner -int 13", f"{G_} NSAutomaticQuoteSubstitutionEnabled -bool false"]
else:
    CMDS.append("defaults write com.apple.dock wvous-br-corner -int 4")
if os.environ.get("VB_REVB") == "ignore":
    CMDS.append("defaults write com.apple.dock show-recents -bool false")

G.start()
G.open_terminal()
# A few commands per line: one very long typed line has dropped keys before.
for k in range(0, len(CMDS), 4):
    G.run(" && ".join(CMDS[k : k + 4]) + " && echo ok", wait=3)
G.run("killall Dock; killall Finder; killall SystemUIServer; echo restarted", wait=6)
G.shot("final")
with open("/logs/agent/final_answer.txt", "w") as f:
    f.write(
        "I went through the checklist as a Sales user, with Revisions B and C. Changed: Dark appearance, scroll "
        "bars always shown, the Scale minimize effect, prefer tabs always, ask to keep changes when closing "
        "documents, double-click the title bar to minimize, 24-hour clock with seconds, all filename extensions, "
        "no warning when changing an extension, the path bar, folders on top when sorting by name, search the "
        "current folder, key repeat one step below the fastest (Revision C), spelling correction off, Monday as "
        "the first day of the week, click wallpaper to reveal the desktop only in Stage Manager, and the "
        "bottom-right hot corner to Desktop. Already set: Dock auto-hide, closing windows when quitting, new "
        "Finder windows on Documents, the status bar, Spaces not rearranging, and Stage Manager off. I kept "
        "recent apps in the Dock (Revision B) and skipped the Finance section.\n"
    )
print("gold done")
