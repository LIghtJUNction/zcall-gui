.PHONY: all check check-ui install
PREFIX ?= /usr/local
DESTDIR ?=
TARGET_DIR = $(shell cargo metadata --no-deps --format-version 1 --locked --offline | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')

all:
	cargo build --release --locked --offline

check:
	python3 scripts/check.py

check-ui:
	python3 scripts/check.py --ui

install:
	install -Dm755 "$(TARGET_DIR)/release/zcall-gui" "$(DESTDIR)$(PREFIX)/bin/zcall-gui"
	install -Dm644 packaging/zcall-gui.desktop "$(DESTDIR)$(PREFIX)/share/applications/zcall-gui.desktop"
	install -Dm644 packaging/zcall-gui.svg "$(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/zcall-gui.svg"
	install -Dm644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/zcall-gui/LICENSE"
	install -Dm644 vendor/zgpui/LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/zcall-gui/zgpui-LICENSE"

.PHONY: icons
icons:
	python3 scripts/generate_icons.py
