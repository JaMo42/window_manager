xrdb -merge $HOME/.Xresources
case $VALGRIND in
  1)
    exec valgrind ./target/debug/window_manager 2>err
    ;;
  2)
    exec valgrind --track-origins=yes --leak-check=full ./target/debug/window_manager 2>err
    ;;
  *)
    exec ./target/debug/window_manager 2>err
    ;;
esac

