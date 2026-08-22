curl https://raw.githubusercontent.com/unicode-org/cldr/main/common/supplemental/windowsZones.xml > win_cldr_data/windowsZones.xml
git submodule update --init
cd tz && git checkout main && git pull && 
cd ..
