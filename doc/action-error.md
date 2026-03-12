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

- `XRet<T> = Result<T, XError>` and `XRerr = Result<(), XError>` are the typed error carriers.
- `XError::Unwind(msg)` means business/runtime failure that can be handled by caller logic.
- `XError::Interrupt(msg)` means hard failure and must stop the current execution path.
- Wire protocol between `Ret<Error>` and `XRet<XError>`:
  - Recoverable: `"[UNWIND] " + msg`
  - Unrecoverable: plain message with no prefix

In action code:

- `err!` / `errf!` => unrecoverable by default.
- `xerr!` / `xerrf!` => unrecoverable by default.
- `xerr_r!` / `xerr_rf!` => explicitly recoverable.

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

- `[UNWIND] ...` => recoverable.
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

- Unrecoverable (`err!`/`errf!`, `xerr!`/`xerrf!`): default for all framework/system/usage/policy errors.
- Recoverable (`xerr_r!`/`xerr_rf!`): only for contract explicit business throw/return semantics.

This keeps action semantics aligned with VM and snapshot rollback strategy.

## 6. One-to-one recoverable checklist

This checklist is the authoritative reference for recoverable errors.

### 6.0 Guard Action checks/judgement failures

- `HeightScope`: invalid range / out-of-range height check failure.
- `ChainAllow`: chain-id allowlist check failure.
- Guard-level judgement failures in Guard actions use explicit recoverable signaling.

### 6.1 HAC/HACD/BTC/ASSET state failures

- HAC: amount non-positive, insufficient balance, balance check insufficient.
	- module: `protocol/src/operate/hacash.rs`
- BTC(Satoshi): amount empty/zero, insufficient satoshi, transfer self-denied.
	- module: `protocol/src/operate/satoshi.rs`
- ASSET: amount empty/zero, insufficient asset, transfer self-denied.
	- module: `protocol/src/operate/asset.rs`
- HACD: insufficient diamond number, transfer self-denied, ownership/status mismatch (`not belong`, mortgaged/not transferable), owned-form missing.
	- module: `protocol/src/operate/diamond.rs`

### 6.2 Channel insufficient balance on open

- `ChannelOpen` fails via HAC balance debit path (`hac_sub`) when either side is insufficient.
	- module: `mint/src/action/channel.rs` + `protocol/src/operate/hacash.rs`

### 6.3 `maincall` / `p2shcall` / `abstcall` return error codes

`check_vm_return_value` (logic inlined, no separate helper):

- **Success:** Return value is falsy (nil, 0, false, empty/all-zero bytes or address).
- **Recoverable:** Non-falsy scalar or object (Args, Compo) — business error code or JSON detail; all reported via `xerr_rf!`/`XError::revert`.
- **Unrecoverable:** Return type is `HeapSlice` — not supported; reported via `xerrf!`/`XError::fault`.

Rule:

- Falsy return => success.
- Non-falsy or Args/Compo => recoverable return error.
- HeapSlice => unrecoverable (return type not supported).

Mapping location:

- `vm/src/machine/setup.rs` (`check_vm_return_value`)
- `vm/src/machine/machine.rs` (`main_call` / `p2sh_call` / `abst_call` call sites)

### 6.4 Contract-thrown errors by AST / ERR / ABT bytecode

- VM frame converts `Abort|Throw` exits into `ThrowAbort`.
- `ThrowAbort` is classified as recoverable.

Mapping location:

- `vm/src/frame/call.rs`
- `vm/src/rt/error.rs`

### 6.5 Ext action recoverability pass-through

- Host `action_call` returns `XRet`; recoverability is carried by `XError` (Unwind vs Interrupt), not by string prefix.
- Interpreter maps `XError::Unwind(msg)` → `ItrErr(ActCallUnwind, msg)`, `XError::Interrupt(msg)` → `ItrErr(ActCallError, msg)`.
- `vm/src/rt/error.rs` maps `ActCallUnwind` → recoverable, `ActCallError` → unrecoverable (by code, no prefix parsing).

Mapping location:

- `vm/src/machine/host.rs` (trait returns `XRet`)
- `vm/src/interpreter/execute.rs` (XError → ItrErr by variant)
- `vm/src/rt/error.rs` (ItrErrCode → XError/Error)

## 7. Additional business-failure candidates (optional)

Potentially recoverable if product policy confirms:

- Explicit business quota/limit exceeded.
- Idempotent duplicate-request rejection.
- Business-state transition denied (without protocol-format violation).
