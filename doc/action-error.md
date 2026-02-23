# Action Error Classification

## 1. Why classify errors as recoverable vs unrecoverable

The runtime must separate business-level failures from system-level failures to keep state transitions deterministic and safe:

- Recoverable errors represent expected business or execution outcomes under valid protocol context.
- Unrecoverable errors represent system faults, protocol misuse, malformed usage, or invariant violations.

This separation enables:

- Safe fallback/branch behavior (for example in AST conditional execution).
- Clear rollback boundaries for transaction/state snapshots.
- Stable consensus behavior by preventing ambiguous error handling.
- Explicit error contracts between Action, VM, and host layers.

## 2. Error model and signaling

- `BRet<T> = Result<T, BError>` and `BRerr = Result<(), BError>` are the typed error carriers.
- `BError::Recoverable(msg)` means business/runtime failure that can be handled by caller logic.
- `BError::Unrecoverable(msg)` means hard failure and must stop the current execution path.

In action code:

- `err!` / `errf!` => unrecoverable by default.
- `berr!` / `berrf!` => unrecoverable by default.
- `erru!` / `erruf!` => explicitly recoverable.
- `berru!` / `berruf!` => explicitly recoverable.

## 3. Business errors (recoverable)

Business errors are limited to exactly three categories.

### 3.1 State-data business failures

Examples:

- Balance check failures.
- Transfer debit/credit failures caused by state data constraints.

Why recoverable:

- They represent business-state rejection, not runtime/framework corruption.

### 3.2 Guard-action decision failures

Examples:

- Guard condition evaluates to false.
- Optional guarded branch is rejected by business condition.

Why recoverable:

- They are expected logic outcomes under business rules.

### 3.3 VM opcode explicit throw/return failures

Runtime class:

- `ThrowAbort` (contract opcode explicit throw/abort path).

External action bridge:

`ExtActCallError` uses dynamic prefix classification:

- `Recoverable: ...` => recoverable.
- `Unrecoverable: ...` => unrecoverable.
- No explicit prefix => unrecoverable by default.

Why recoverable:

- Downstream module explicitly declares business-level failure semantics.
- Caller receives a stable contract for fallback behavior.

## 4. Unrecoverable error classes

### 4.1 System faults / invariant violations

Any failure indicating corrupted state machine assumptions, broken runtime invariants, or impossible internal state must be unrecoverable.

### 4.2 Protocol or usage errors

Errors caused by malformed protocol framing, invalid entry shape, invalid call usage, or forbidden execution context are unrecoverable.

### 4.3 VM structural limitations and illegal program form

Structural VM constraints and illegal opcodes are unrecoverable by policy (for example invalid opcode / never-touch opcode categories).

### 4.4 VM runtime operation failures are unrecoverable

The following classes are unrecoverable by policy:

- Control-flow/bounds errors.
- Instruction permission/context errors.
- Execution-time resource limit failures.
- Runtime subsystem operation failures.
- Call semantic failures.
- Type/cast/data-shape failures.
- Composite data operation failures.
- Arithmetic/native/data utility failures.
- Storage business constraint failures.

### 4.5 Action execute: system/usage errors

In `Action::execute`, these must be unrecoverable:

- Feature-gate checks.
- Height-not-enabled checks.
- Kind/index/range limit checks.
- Meta parameter format and validation errors.
- Protocol fee and protocol rule validation failures.
- Context misuse and illegal invocation pattern.
- Snapshot/recovery framework failures.
- Negative returned gas or impossible gas accounting state.
- AST or execution-structure constraint violations treated as hard policy errors.
- Any condition showing infrastructure breakage instead of business rejection.

## 5. Action execute classification rule (normative)

Use this strict split in all action implementations:

- Unrecoverable (`err!`/`errf!`, `berr!`/`berrf!`): default for all framework/system/usage/policy errors.
- Recoverable (`erru!`/`erruf!`, `berru!`/`berruf!`): only for contract explicit business throw/return semantics.

This keeps action semantics aligned with VM and snapshot rollback strategy.

## 6. Explicit classification (business vs system)

### 6.1 Business errors (recoverable)

Only the following are business errors and recoverable:

- State-data business failures (balance/transfer state constraint failures).
- All state-data checks and state operation failures.
- Guard-action decision failures.
- VM opcode explicit throw/return failures (`ThrowAbort`).
- `ExtActCallError` with explicit `Recoverable: ...` prefix.

### 6.2 System errors (unrecoverable)

Everything else is system error and unrecoverable, including:

- VM control-flow/bounds failures.
- Instruction permission/context failures.
- Execution-time resource failures.
- Runtime subsystem operation failures.
- Call semantic failures.
- Type/cast/data-shape failures.
- Composite-data operation failures.
- Arithmetic/native/data utility failures.
- Storage constraint failures.
- `Action::execute` framework/policy/usage failures (feature, height gate, kind/range limits, meta validation, protocol fee/rule checks, snapshot/gas accounting anomalies).
- `ExtActCallError` with `Unrecoverable: ...` or without explicit prefix.

## 7. Current recoverable list (audited)

Current code-level recoverable points are:

- State operation check/failure paths in `protocol/src/operate` (hacash/satoshi/asset/diamond modules).
- `ThrowAbort` path from VM return error throw.
- `ExtActCallError` only when downstream error text is explicitly prefixed with `Recoverable: `.

Current code scan result:

- State operation modules now emit explicit recoverable errors for state checks and operation failures.
- Recoverable behavior additionally includes VM throw (`ThrowAbort`) and external-call explicit recoverable prefixing.
