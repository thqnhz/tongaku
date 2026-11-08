.PHONY: all lint dev rel clean cleanall

all: dev

lint:
	cargo fmt
	cargo clippy

dev: lint
	cargo run

# Should only do this in CI
rel:
	cargo build --release --workspace --timings

clean:
	rm -rf target

cleanall: clean
	cargo clean
