#!/usr/bin/sh

Xephyr +extension RANDR +xinerama -screen 1600x900+0+0 -screen 1280x720+1600+180 -ac :3 &
XEPHYR_PID=$!
sleep .1
DISPLAY=:3 ./target/debug/window_manager
sleep .1
kill -9 $XEPHYR_PID
