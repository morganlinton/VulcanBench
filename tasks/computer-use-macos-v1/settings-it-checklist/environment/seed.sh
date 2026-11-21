# settings-it-checklist seed (guest, logged-in user): a known starting state.
# Already compliant with the checklist: Dock auto-hide on (item 3), Close
# windows when quitting on (item 6), new Finder windows show Documents
# (item 9), Spaces not rearranged (item 12). Recent apps show in the Dock,
# which the checklist's revision keeps. Neighbours that must not change:
# Dock at the bottom, size 48, no magnification, no accent color set.
set -eu
defaults write com.apple.dock orientation -string bottom
defaults write com.apple.dock autohide -bool true
defaults write com.apple.dock show-recents -bool true
defaults write com.apple.dock tilesize -int 48
defaults write com.apple.dock magnification -bool false
defaults write com.apple.dock mineffect -string genie
defaults write com.apple.dock mru-spaces -bool false
defaults write NSGlobalDomain NSQuitAlwaysKeepsWindows -bool false
defaults write NSGlobalDomain AppleShowScrollBars -string Automatic
defaults write NSGlobalDomain AppleICUForce24HourTime -bool false
defaults write NSGlobalDomain AppleShowAllExtensions -bool false
defaults delete NSGlobalDomain AppleInterfaceStyle 2>/dev/null || true
defaults delete NSGlobalDomain AppleFirstWeekday 2>/dev/null || true
defaults delete NSGlobalDomain AppleAccentColor 2>/dev/null || true
defaults write com.apple.finder NewWindowTarget -string PfDo
defaults write com.apple.finder NewWindowTargetPath -string "file:///Users/admin/Documents/"
killall Dock 2>/dev/null || true
killall Finder 2>/dev/null || true
sleep 2
echo seeded-settings
