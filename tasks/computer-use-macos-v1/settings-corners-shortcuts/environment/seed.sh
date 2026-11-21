# settings-corners-shortcuts seed (guest, logged-in user). The top-right hot
# corner already opens Notification Center and must stay; the bottom-right
# one is macOS's default Quick Note; nothing is on the other two. "omw" is
# macOS's default text replacement and must stay.
set -eu
defaults write com.apple.dock wvous-tr-corner -int 12
defaults write com.apple.dock wvous-tr-modifier -int 0
defaults write com.apple.dock wvous-br-corner -int 14
defaults write com.apple.dock wvous-br-modifier -int 0
defaults delete com.apple.dock wvous-tl-corner 2>/dev/null || true
defaults delete com.apple.dock wvous-tl-modifier 2>/dev/null || true
defaults delete com.apple.dock wvous-bl-corner 2>/dev/null || true
defaults delete com.apple.dock wvous-bl-modifier 2>/dev/null || true
defaults write com.apple.dock mineffect -string genie
killall Dock || true
sleep 3
defaults read com.apple.dock | grep -E 'wvous|mineffect'
echo seeded-settings
