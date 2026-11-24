#!/usr/bin/env bash
# Usage: xvfb-run -a -s "-screen 0 1700x1050x24" bash tools/ui_checks/marking_menu.sh build/Aztek/AppRun
set -euo pipefail

work=$(mktemp -d)
export QT_QPA_PLATFORM=xcb AZTEK_ECHO=1 AZTEK_PROFILE=1
export XDG_CONFIG_HOME="$work/config" AZTEK_PREFS="$work/prefs" AZTEK_APP_DIR="$work/app"
"$1" --script /dev/null >"$work/app.log" 2>&1 &
app=$!
trap 'kill "$app" 2>/dev/null || true; wait "$app" 2>/dev/null || true; rm -rf -- "$work"' EXIT

win=$(timeout 15 xdotool search --sync --onlyvisible --name '^Aztek - untitled' | head -1)
for attempt in {1..100}; do
    if rg -q '^render persp ' "$work/app.log"; then break; fi
    sleep .1
done
rg -q '^render persp ' "$work/app.log"
xdotool windowfocus "$win"

highlighted() {
    import -window root -crop 10x4+520+514 "$work/hover.png"
    magick "$work/hover.png" -format %c histogram:info:- | rg -qi '#5285A6'
}

# A tap opens the menu. An unpressed pointer highlights the Select All row.
xdotool mousemove 600 420 click 3 mousemove 620 515
sleep .2
highlighted
xdotool key Escape

# A fast right-button stroke crosses that row between motion samples.
xdotool mousemove 600 420 mousedown 3
sleep .2
xdotool mousemove 460 515
sleep .1
xdotool mousemove 820 515
sleep .2
highlighted
xdotool mouseup 3
for attempt in {1..30}; do
    if rg -q '^SelectAll;' "$work/app.log"; then break; fi
    sleep .1
done
rg -q '^SelectAll;' "$work/app.log"

# With an empty selection, Cube is south and its option box has a separate hit target.
sleep .5
xdotool mousemove 650 420 keydown Shift_L mousedown 3
sleep .2
xdotool mousemove 682 474 mouseup 3 keyup Shift_L
options=$(timeout 8 xdotool search --sync --onlyvisible --name '^Cube Options$' | head -1)
xdotool windowfocus "$options" key Escape
sleep .5
xdotool windowfocus "$win" mousemove 650 420 keydown Shift_L mousedown 3
sleep .2
xdotool mousemove 650 474 mouseup 3 keyup Shift_L
for attempt in {1..30}; do
    if rg -q '^polyCube(;| )' "$work/app.log"; then exit 0; fi
    sleep .1
done
tail -30 "$work/app.log" >&2
exit 1
