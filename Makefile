# We need to use a makefile for building because we want to conditionally enable
# using the alsa and pulseaudio backends for both usage in the window manager
# and usage of the cargo dependency.  We could enable the feature from build.rs
# but what won't allow us to conditionally include the crates and if the
# respective libraries are not installed those crates will give us a build error
# even if we don't use them.

FEATURES :=
ifeq ($(shell pkg-config --exists alsa; echo $$?),0)
	FEATURES += my_alsa
endif

ifeq ($(shell pkg-config --exists libpulse; echo $$?),0)
	FEATURES += pulse
endif

# We usually only want to build the window manager so for debug builds we only
# compile the utility programs if requested.
ifeq ($(BUILD_ALL),1)
	CARGO_BIN :=
else
	CARGO_BIN = --bin window_manager
endif

SRC = $(wildcard src/*.rs src/**/*.rs) build.rs Cargo.toml

BIN = target/debug/window_manager
RELBIN = target/release/window_manager

DESKTOP_FILE = window_manager.desktop
SESSION_PATH = /usr/share/xsessions/$(DESKTOP_FILE)
INSTALL_PREFIX ?= /usr/local/bin
COPY ?= cp -v

.PHONY: debug
debug: $(BIN)

.PHONY: release
release: $(RELBIN)

$(BIN): $(SRC)
	cargo build $(CARGO_BIN) --no-default-features --features '$(FEATURES)'

$(RELBIN): $(SRC)
	cargo build --release

$(SESSION_PATH): $(DESKTOP_FILE)
	@$(COPY) $< $@

.PHONY: install
install: $(RELBIN) $(SESSION_PATH)
	@$(COPY) $(RELBIN) $(INSTALL_PREFIX)/window_manager
	@$(COPY) target/release/quit $(INSTALL_PREFIX)/window_manager_quit
	@$(COPY) target/release/message_box $(INSTALL_PREFIX)/window_manager_message_box
