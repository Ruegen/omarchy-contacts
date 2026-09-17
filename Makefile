.PHONY: daemon test clean install

daemon:
	cargo build --release
	cp -f target/release/omarchy-contactsd ./omarchy-contactsd
	chmod 755 ./omarchy-contactsd

test:
	cargo test

install: daemon
	mkdir -p "$(HOME)/.local/bin" "$(HOME)/.local/share/applications"
	install -m755 omarchy-contacts "$(HOME)/.local/bin/omarchy-contacts"
	install -m644 omarchy-contacts.desktop "$(HOME)/.local/share/applications/omarchy-contacts.desktop"
	ln -sfn /usr/share/omarchy/shell/Commons Commons
	ln -sfn /usr/share/omarchy/shell/Ui Ui

clean:
	cargo clean
	rm -f ./omarchy-contactsd
