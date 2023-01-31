
# https://rust-fuzz.github.io/book/cargo-fuzz/coverage.html
gen-fuzz-cov:
	cargo fuzz coverage mqtt_v5_arbitrary
	~/.rustup/toolchains/nightly-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-cov \
		show \
		fuzz/target/aarch64-apple-darwin/release/mqtt_v5_arbitrary \
		--format=html \
		-instr-profile=fuzz/coverage/mqtt_v5_arbitrary/coverage.profdata \
		> fuzz/coverage/index.html
