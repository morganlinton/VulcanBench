#!/bin/sh
# usage: cmp.sh sessionfile
echo "=== session ==="; cat "$1"
echo "--- legacy ---"
./legacy/run < "$1"
echo "--- python ---"
python3 lodgecore.py < "$1"
