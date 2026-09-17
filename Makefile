.PHONY: daemon test clean

daemon:
	cargo build --release
	cp -f target/release/omarchy-contactsd ./omarchy-contactsd
	chmod 755 ./omarchy-contactsd

test:
	cargo test

clean:
	cargo clean
	rm -f ./omarchy-contactsd
