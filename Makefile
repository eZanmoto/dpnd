# Note that `target` is used as the output directory for Rust so care should be
# taken that collisions don't occur between Rust output and local output.
tgt_dir:=target
tgt_test_dir:=$(tgt_dir)/tests

.PHONY: check
check: \
	check_intg \
	check_lint

# We run integration tests in sequence because some tests in `tests/cli.rs` run
# a Git daemon during testing and running multiple instances of the Git daemon
# at the same will result in port collisions and will cause failures.
.PHONY: check_intg
check_intg: $(tgt_test_dir)
	TEST_DIR='$(shell pwd)/$(tgt_test_dir)' \
		cargo test \
			-- \
			--show-output \
			--test-threads=1 \
			$(TESTS)

.PHONY: check_lint
check_lint:
	TEST_DIR='$(shell pwd)/$(tgt_test_dir)' \
		cargo clippy \
			--all-targets \
			--all-features \
			-- \
			-D warnings \
			-D clippy::pedantic \
			-D clippy::cargo
	python3 scripts/check_line_length.py \
		'**/*.rs' \
		79

# We tag `$(tgt_test_dir)` as phony so that the test directory is removed and
# recreated at the start of every test run.
.PHONY: $(tgt_test_dir)
$(tgt_test_dir): $(tgt_dir)
	rm -rf '$(tgt_test_dir)'
	mkdir '$@'

$(tgt_dir):
	mkdir '$@'
