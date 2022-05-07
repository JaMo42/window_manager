xrdb -merge $HOME/.Xresources
case $VALGRIND in
  1)
    exec valgrind ./target/debug/window_manager 2>vgerr
    ;;
  2)
    exec valgrind --track-origins=yes --leck-check=full ./target/debug/window_manager 2>vgerr
    ;;
  *)
    exec ./target/debug/window_manager
    ;;
esac

