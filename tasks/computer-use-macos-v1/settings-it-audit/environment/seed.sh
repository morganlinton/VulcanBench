# settings-it-audit seed (guest, logged-in user): a known starting state.
# Already compliant: Dock auto-hide (item 3), close windows when quitting
# (8), new Finder windows show Documents (14), status bar (16), Spaces not
# rearranged (22), Stage Manager off (S2). Recent apps show in the Dock,
# which Revision B keeps. Neighbours that must not change: Dock at the
# bottom, size 48, no magnification, no accent color, initial key repeat
# delay 25, smart quotes on (a Finance-only item), other hot corners unset.
set -eu
defaults write com.apple.dock orientation -string bottom
defaults write com.apple.dock autohide -bool true
defaults write com.apple.dock show-recents -bool true
defaults write com.apple.dock tilesize -int 48
defaults write com.apple.dock magnification -bool false
defaults write com.apple.dock mineffect -string genie
defaults write com.apple.dock mru-spaces -bool false
defaults write com.apple.dock wvous-br-corner -int 1
defaults write com.apple.dock wvous-br-modifier -int 0
defaults write NSGlobalDomain NSQuitAlwaysKeepsWindows -bool false
defaults write NSGlobalDomain AppleShowScrollBars -string Automatic
defaults write NSGlobalDomain AppleICUForce24HourTime -bool false
defaults write NSGlobalDomain AppleShowAllExtensions -bool false
defaults write NSGlobalDomain AppleWindowTabbingMode -string fullscreen
defaults write NSGlobalDomain NSCloseAlwaysConfirmsChanges -bool false
defaults write NSGlobalDomain AppleActionOnDoubleClick -string Fill
defaults write NSGlobalDomain KeyRepeat -int 12
defaults write NSGlobalDomain InitialKeyRepeat -int 25
defaults write NSGlobalDomain NSAutomaticSpellingCorrectionEnabled -bool true
defaults write NSGlobalDomain NSAutomaticQuoteSubstitutionEnabled -bool true
defaults write NSGlobalDomain NSAutomaticDashSubstitutionEnabled -bool true
defaults delete NSGlobalDomain AppleInterfaceStyle 2>/dev/null || true
defaults delete NSGlobalDomain AppleFirstWeekday 2>/dev/null || true
defaults delete NSGlobalDomain AppleAccentColor 2>/dev/null || true
defaults write com.apple.menuextra.clock ShowSeconds -bool false
defaults write com.apple.finder NewWindowTarget -string PfDo
defaults write com.apple.finder NewWindowTargetPath -string "file:///Users/admin/Documents/"
defaults write com.apple.finder FXEnableExtensionChangeWarning -bool true
defaults write com.apple.finder ShowPathbar -bool false
defaults write com.apple.finder ShowStatusBar -bool true
defaults write com.apple.finder _FXSortFoldersFirst -bool false
defaults write com.apple.finder FXDefaultSearchScope -string SCev
defaults write com.apple.WindowManager GloballyEnabled -bool false
defaults write com.apple.WindowManager EnableStandardClickToShowDesktop -bool true
killall Dock 2>/dev/null || true
killall Finder 2>/dev/null || true
killall SystemUIServer 2>/dev/null || true
sleep 2
echo seeded-settings
