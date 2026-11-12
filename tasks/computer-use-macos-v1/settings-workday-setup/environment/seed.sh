# settings-workday-setup seed (guest, logged-in user): a known starting state
# for the settings the task touches and the neighbours it must leave alone.
set -eu
defaults write com.apple.dock orientation -string bottom
defaults write com.apple.dock autohide -bool false
defaults write com.apple.dock show-recents -bool true
defaults write com.apple.dock tilesize -int 48
defaults write com.apple.dock magnification -bool false
defaults write com.apple.dock minimize-to-application -bool false
defaults write NSGlobalDomain AppleICUForce24HourTime -bool false
defaults write com.apple.WindowManager EnableStandardClickToShowDesktop -bool true
defaults write com.apple.WindowManager GloballyEnabled -bool false
killall Dock 2>/dev/null || true
sleep 2
echo seeded-settings
