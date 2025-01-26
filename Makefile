#/usr/bin/make -f

INSTALL_PROGRAM = install -D -m 0755
INSTALL_DATA = install -D -m 0644

PREFIX ?= $(DESTDIR)
BINDIR ?= $(PREFIX)/usr/bin

BIN := elbey

MESON = meson


all: build

distclean: clean

clean:
	-cargo clean

build-arch: build

build-independent: build

binary: build

binary-arch: build

binary-independent: build

build: 
	cargo build --release

install: 
	$(INSTALL_PROGRAM) "./target/release/$(BIN)" "$(BINDIR)/$(BIN)"

uninstall:
	rm -f "$(BINDIR)/$(BIN)"

run-test:
	cargo test -- --test-threads=1
