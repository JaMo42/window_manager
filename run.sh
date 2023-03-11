#!/usr/bin/sh

run_xephyr()
{
  case $1 in
    single)
      Xephyr -screen 2560x1400 -ac :3 &
      ;;
    multi)
      Xephyr +extension RANDR +xinerama -screen 1920x1080+100+100 -screen 1600x900+2020+280 -ac :3 &
      ;;
  esac
  XEPHYR_PID=$!
  sleep .1
  > $HOME/.config/window_manager/log.txt
  DISPLAY=:3 bash xinitrc
  sleep .1
  if ps -p $XEPHYR_PID > /dev/null
  then
    kill -9 $XEPHYR_PID
  fi
}

case "${1:-startx}" in
  startx)
    startx ./xinitrc
    ;;
  xephyr)
    run_xephyr single
    ;;
  multi_mon)
    run_xephyr multi
    ;;
esac
