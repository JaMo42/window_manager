#!/usr/bin/sh

Xephyr -screen 2560x1400 -ac :3 &
XEPHYR_PID=$!
sleep .1
> $HOME/.config/window_manager/log.txt
DISPLAY=:3 bash xinitrc
sleep .1
kill -9 $XEPHYR_PID
