.PHONY: all check check-ui install
PREFIX ?= /usr/local
DESTDIR ?=

all:
	cargo build --release --locked --offline

check:
	python3 scripts/check.py

check-ui:
	python3 scripts/check.py --ui

install:
	install -Dm755 target/release/zcall-gui "$(DESTDIR)$(PREFIX)/bin/zcall-gui"
	install -Dm644 packaging/zcall-gui.desktop "$(DESTDIR)$(PREFIX)/share/applications/zcall-gui.desktop"
	install -Dm644 packaging/zcall-gui.svg "$(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/zcall-gui.svg"
	install -Dm644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/zcall-gui/LICENSE"
	install -Dm644 vendor/zui/LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/zcall-gui/zui-LICENSE"
