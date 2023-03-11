#!/usr/bin/env bash

: "${INSTALL_PREFIX:=/usr/local/bin}"
: "${CONFIG_PREFIX:=$XDG_CONFIG_HOME}"
: "${CONFIG_PREFIX:=$HOME/.config}"

CONFIG_DIR=$CONFIG_PREFIX/window_manager
RESOURCE_DIR=$CONFIG_DIR/res

echo Installing binaries to: $INSTALL_PREFIX
echo Installing configuration to: $CONFIG_DIR
echo Installing resources to: $RESOURCE_DIR

test -f "target/release/window_manager" || cargo build --release

echo Installing program
sudo cp -v target/release/window_manager $INSTALL_PREFIX/window_manager
sudo cp -v target/release/quit $INSTALL_PREFIX/window_manager_quit
sudo cp -v target/release/message_box $INSTALL_PREFIX/window_manager_message_box

read -p "Install config? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
  test -f $CONFIG_DIR || mkdir -p $CONFIG_DIR
  cp -rv config/* $CONFIG_DIR
fi

read -p "Install session manager session? [y/N]" -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
  sudo cp -v window_manager.desktop /usr/share/xsessions/window_manager.desktop
fi
