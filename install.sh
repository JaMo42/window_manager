#!/usr/bin/env sh

: "${INSTALL_PREFIX:=/usr/local/bin}"
: "${CONFIG_PREFIX:=$XDG_CONFIG_HOME}"
: "${CONFIG_PREFIX:=$HOME/.config}"

CONFIG_DIR=$CONFIG_PREFIX/window_manager

echo Installing program to: $INSTALL_PREFIX
echo Installing configuration to: $CONFIG_DIR

test -f "target/release/window_manager" || cargo build --release

echo Installing program
sudo cp -v target/release/window_manager $INSTALL_PREFIX/window_manager

read -p "Install config? [Y/n] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Nn]$ ]]; then
  test -f $CONFIG_DIR || mkdir -p $CONFIG_DIR
  cp -v config $CONFIG_DIR/config
fi

read -p "Install autostartrc? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
  test -f $CONFIG_DIR || mkdir -p $CONFIG_DIR
  cp -v autostartrc $CONFIG_DIR/autostartrc
fi

read -p "Install session manager session? [y/N]" -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
  sudo cp -v window_manager.desktop /usr/share/xsessions/window_manager.desktop
fi

