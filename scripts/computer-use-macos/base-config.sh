#!/bin/sh
# ComputerUse macOS base image: system configuration, run in the guest as the
# logged-in user through Tart's guest agent by build_base.py:
#
#     base-config.sh NEW_PASSWORD
#
# The stock image (Cirrus Labs macos-golden-gate-vanilla) logs in
# automatically as admin/admin with passwordless sudo, network time, a "Managed
# via Tart" identity and live widgets. This script makes the screen
# deterministic and the identity ordinary, and replaces the public password
# with one the agent is never told (the host keeps it in
# ~/.config/vulcanbench/macos-guest-password). Passwordless sudo stays in the
# base image for seeding and is removed when each task image is finalized
# (build_task_image.py), so a trial's agent cannot become root through
# Terminal.
set -eu
NEWPW="$1"
PERSONA="Jordan Lee"

# Identity: no "Managed via Tart" in the Apple menu, the prompt or sharing.
sudo -n scutil --set ComputerName "Jordan's Mac"
sudo -n scutil --set LocalHostName "Jordans-Mac"
sudo -n scutil --set HostName "Jordans-Mac"
sudo -n dscl . -create /Users/admin RealName "$PERSONA"

# Time: no network time; a fixed zone. The clock itself is set at each boot
# by the vb-clock daemon from /Library/VulcanBench/clock (task images).
sudo -n systemsetup -setusingnetworktime off >/dev/null 2>&1
sudo -n systemsetup -settimezone America/New_York >/dev/null 2>&1
sudo -n mkdir -p /usr/local/libexec /Library/VulcanBench
printf '%s\n' '#!/bin/sh' \
  '# Sets the guest clock at boot from /Library/VulcanBench/clock (date(1) format MMDDhhmmCCYY).' \
  'f=/Library/VulcanBench/clock' \
  '[ -s "$f" ] || exit 0' \
  '/bin/date "$(cat "$f")" >/dev/null 2>&1' \
  | sudo -n tee /usr/local/libexec/vb-clock.sh >/dev/null
sudo -n chmod 755 /usr/local/libexec/vb-clock.sh
printf '%s' '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>Label</key><string>org.vulcanbench.clock</string><key>ProgramArguments</key><array><string>/usr/local/libexec/vb-clock.sh</string></array><key>RunAtLoad</key><true/></dict></plist>' \
  | sudo -n tee /Library/LaunchDaemons/org.vulcanbench.clock.plist >/dev/null
sudo -n chmod 644 /Library/LaunchDaemons/org.vulcanbench.clock.plist

# Updates and background noise.
sudo -n softwareupdate --schedule off >/dev/null 2>&1 || true
sudo -n defaults write /Library/Preferences/com.apple.SoftwareUpdate AutomaticCheckEnabled -bool false
sudo -n defaults write /Library/Preferences/com.apple.SoftwareUpdate AutomaticDownload -bool false
sudo -n defaults write /Library/Preferences/com.apple.commerce AutoUpdate -bool false
defaults write com.apple.assistant.support "Assistant Enabled" -bool false

# A screen that starts the same every time.
defaults write NSGlobalDomain NSQuitAlwaysKeepsWindows -bool false
defaults write com.apple.loginwindow TALLogoutSavesState -bool false
defaults write NSGlobalDomain NSDocumentSaveNewDocumentsToCloud -bool false
# TextEdit keeps its prefs in a protected container; its open panel on launch is stock behaviour.
defaults write com.apple.TextEdit NSShowAppCentricOpenPanelInsteadOfUntitledFile -bool false 2>/dev/null || true
defaults write com.apple.WindowManager StandardHideWidgets -bool true
defaults write com.apple.WindowManager StageManagerHideWidgets -bool true
defaults write com.apple.universalaccess reduceMotion -bool true 2>/dev/null || true
defaults write com.apple.universalaccess reduceTransparency -bool true 2>/dev/null || true
# The template turned on full keyboard access; a stock Mac has it off.
defaults delete NSGlobalDomain AppleKeyboardUIMode 2>/dev/null || true

# A password the agent is never told; auto-login keeps working with it. The
# login keychain is recreated at the next login under the new password.
# The account holds a SecureToken, so the change needs the old password
# (the template's public "admin"); skipped when the new one is already set.
if ! dscl . -authonly admin "$NEWPW" >/dev/null 2>&1; then
  dscl . -passwd /Users/admin "${OLDPW:-admin}" "$NEWPW"
fi
sudo -n sysadminctl -autologin set -userName admin -password "$NEWPW" >/dev/null 2>&1
rm -f "$HOME/Library/Keychains/login.keychain-db"
echo base-config-ok
