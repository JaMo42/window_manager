xrdb -merge $HOME/.Xresources
if [ $VALGRIND = 1 ]; then
  exec valgrind ./target/debug/window_manager 2>vgerr
else
  exec ./target/debug/window_manager
fi

