xrdb -merge $HOME/.Xresources 2>/dev/null
export RUST_BACKTRACE=1
case $VALGRIND in
  1)
    exec valgrind ./target/debug/window_manager 2>err
    ;;
  2)
    exec valgrind --track-origins=yes --leak-check=full ./target/debug/window_manager 2>err
    ;;
  CALLGRIND)
    exec valgrind --tool=callgrind -v ./target/debug/window_manager 2>err
    ;;
  *)
    exec ./target/debug/window_manager 2>err
    ;;
esac

