#!/usr/bin/sh

Xephyr +extension RANDR +xinerama -screen 1600x900+0+0 -screen 1280x720+1600+180 -ac :3 &
XEPHYR_PID=$!
sleep .1
> $HOME/.config/window_manager/log.txt
DISPLAY=:3 bash xinitrc
sleep .1
kill -9 $XEPHYR_PID
