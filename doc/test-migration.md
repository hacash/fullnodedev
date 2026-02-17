# Test Migration Guide

## Scope

This repository now uses a split strategy for tests:

1. `testkit` holds reusable simulation components only.
2. Root `tests/` holds cross-module integration tests.
3. Crate-local tests remain for module-internal semantics and private behaviors.

## Why `testkit` Exists

`testkit` is a workspace leaf crate for test infrastructure:

1. In-memory state models (`ForkableMemState`, `FlatMemState`)
2. In-memory logs (`MemLogs`)
3. Stub transactions (`DummyTx`, `StubTx`, `StubTxBuilder`)
4. Context builders (`make_ctx_with_state`, `make_ctx_with_logs`, `make_ctx_with_default_tx`)
5. VM volatile snapshot mock (`CounterMockVm`, `new_counter_vm`)
6. Cross-module integration helpers (`test_guard`, `vm_main_addr`, `vm_alt_addr`, `make_stub_tx`, `make_ctx_from_tx`, `set_vm_assigner`)

`testkit` is not a container for all test cases. Keep test logic near its ownership boundary.

## Placement Rules

Put tests in root `tests/` when they:

1. Depend on more than one major module (`protocol`, `vm`, `mint`, `chain`, etc.).
2. Validate end-to-end behavior across module boundaries.
3. Require integration setup spanning multiple crates.

Keep tests inside a crate when they:

1. Validate private/internal details.
2. Cover crate-specific behavior without cross-crate orchestration.
3. Are tightly coupled with local test-only internals.

## Current Migration

Moved to root `tests/`:

1. `tests/vm_systems_integration.rs`
2. `tests/vm_action_coverage_integration.rs`
3. `tests/vm_extaction_allowlist_integration.rs`

Refactored to `testkit` in root tests:

1. `tests/diamond_inscription_regression.rs`
2. `tests/protocol_cross_integration.rs`

Removed duplicates from `vm/tests/`:

1. `vm/tests/vm_systems.rs`
2. `vm/tests/action_coverage.rs`
3. `vm/tests/extaction_allowlist.rs`

Converted public test helper module to test-local helper:

1. removed `vm/src/exec_test.rs`
2. removed `pub mod exec_test;` from `vm/src/lib.rs`
3. migrated helper APIs into `vm/tests/common/mod.rs`:
   `build_push_params`, `execute_lang_with_params`
4. switched `vm/tests/choose.rs` to consume `vm/tests/common/mod.rs`
