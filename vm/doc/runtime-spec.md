# VM Runtime Design Specification (Deployment / Loading / Execution)

Version: Baseline behavior of current 
Document Type: Design specification + runtime manual (for test design, code review, and regression acceptance)

## 1. Document Goals

This specification answers three core questions:

1. How this VM system is expected to run across the transaction lifecycle.
2. What the boundary responsibilities are at each stage, and which behaviors are mandatory system constraints.
3. Which invariants tests and reviews should be built around.

This document emphasizes logical consistency and system contracts, without expanding into line-by-line implementation details.

## 2. Scope and Out-of-Scope

### 2.1 In Scope

1. VM registration and activation in node startup/runtime wiring.
2. End-to-end rules for contract deployment, update, loading, and execution.
3. P2SH trigger paths and transfer-hook-triggered VM paths.
4. Snapshot, restore, exception propagation, and consistency constraints.

### 2.2 Out of Scope

1. Per-opcode bytecode interpretation semantics.
2. Gas parameter values and fee policy details.
3. Fitsh compilation mechanics.

## 3. System Design Principles

### 3.1 Validate First, Execute Fast

The system uses a "strict pre-execution validation + contract-based runtime" model. Executable code must pass legality checks before entering execution paths; runtime does not repeat full structural validation.

### 3.2 Permission Model is Mode-Driven

Call modes (`Main/P2sh/Abst/Outer/Inner/View/Pure`) are the core of permission control. Cross-contract calls, state read/write permissions, and extension-call permissions are all mode-governed.

### 3.3 Context Address and Code Owner are Decoupled

The system distinguishes:

1. Execution context address (state scope)
2. Current code owner address (`this/self/super` resolution)

This keeps semantics stable under inheritance, library calls, and delegated execution (e.g., CALLCODE).

### 3.4 Re-entrant but Single-Instance Consistent

VM calls may re-enter through extension actions. Re-entry must reuse the same VM runtime instance so call-stack semantics, resource accounting, and settlement semantics remain consistent.

### 3.5 Rollback Does Not Refund Consumption

Branch recovery (e.g., failed AST branches) rolls back state and logs, but does not roll back already consumed resources, ensuring monotonic resource consumption within a transaction.

## 4. Runtime Architecture Overview

The system can be viewed in four layers:

1. Protocol wiring layer: registers VM actions, hooks, and assigners at node startup.
2. Context control layer: decides VM creation at transaction entry and manages context levels.
3. VM runtime layer: manages resource pools, call frames, function resolution, and execution dispatch.
4. Contract state layer: manages contract metadata, function sets, storage I/O, and version evolution.

## 5. Lifecycle Specification

## 5.1 Startup Phase

When VM is enabled, node startup must complete three integrations:

1. Register VM-related actions.
2. Register action hooks (for VM triggers after transfer and similar actions).
3. Register VM assigner (for on-demand VM instance creation).

If any integration is missing, runtime capability gaps or unreachable paths may occur.

## 5.2 Transaction Entry Phase

Before each transaction executes, the system decides whether VM should be initialized based on transaction conditions. Core goals:

1. Zero overhead skip for non-VM transactions.
2. Guaranteed VM availability for VM transactions at execution entry.

Any explicit VM invocation entry must also support fallback initialization, preventing missing-init failures in tests/tools or non-standard paths.

## 5.3 Contract Deployment Phase

Deployment flow must preserve the following ordering semantics:

1. Validate contract object legality first (structure, capacity, signature uniqueness, executable body legality).
2. Validate deployment context constraints next (address conflicts, self-inheritance, self-linking, etc.).
3. Perform pre-commit graph/link checks:
4. `library` targets must exist.
5. `inherit` targets must exist.
6. Inheritance graph must be acyclic before commit.
7. Static call targets in contract code should be resolvable under current `library` + `inherit` topology.
8. Perform fee check and deduction.
9. Persist contract on success.
10. Execute constructor abstract call if defined.

On failure, consistency must ensure "not persisted" or "recoverable to non-persisted" state.

## 5.4 Contract Update Phase

Update uses an "in-memory edit + validation + abstract callback + commit" model:

1. Apply edits in memory and bump revision.
2. Validate full post-edit constraints.
3. Re-run deployment-equivalent link/graph checks on the candidate version (library/inherit existence, acyclic inheritance, static resolvability).
4. Trigger abstract callback by semantics (`Change` or `Append`).
5. Commit new version only after callback success.

This ordering ensures callbacks observe a validated candidate version, while state commit happens only after callback success.
Deployment and update actions are expected to execute as independent transactions; one transaction does not include both operations on the same contract instance.

## 5.5 Execution Phase

The unified execution entry is responsible for:

1. Validating invocation entry legality.
2. Setting/restoring context level.
3. Entering VM dispatcher.
4. Returning standardized outcomes (consistent success/failure semantics).

"nil or 0 means success" is the shared return contract for main and abstract calls.

## 6. Code and Function Loading Specification

## 6.1 Layered Caching Strategy

Contract loading uses a three-tier strategy: transaction-local cache first, global cache second, state read as fallback. Cache keys must include contract revision to prevent stale-object contamination.

## 6.2 Load Cap Constraint

Each VM runtime instance has a maximum number of loadable contracts; exceeding it must fail. This bounds uncontrolled expansion from complex call graphs.

## 6.3 Function Resolution Semantics

Function target resolution follows:

1. `this`: resolve along context contract inheritance chain.
2. `self`: resolve along current code owner inheritance chain.
3. `super`: resolve starting from current code owner's parent layer.
4. `libidx`: locate library first, then resolve function from the target contract's local user-function table only (no inheritance search on library target).

Resolution must enforce cycle detection and bounds safety.

## 7. Call Model Specification

## 7.1 Top-Level Entry Modes

Top-level VM invocation modes are limited to:

1. `Main`
2. `P2sh`
3. `Abst`

Other modes are internal and should not be used as external entry points.

## 7.2 Frame Model

Execution uses multi-frame dispatch:

1. Root frame establishes call context.
2. Child calls create new frames as needed.
3. Delegated execution (CALLCODE) may switch code within current frame.
4. Returns are type-checked against contracts and propagated upward.

## 7.3 CALLCODE Design Constraints

CALLCODE is an implementation-level delegation mechanism, not a normal function call:

1. Target function parameter count must be 0.
2. Return contract is checked against caller signature.
3. Further call initiation is forbidden while in callcode execution state.

## 7.4 Visibility Constraint

External `CALL` (Outer) must enforce public visibility. The `public` marker is an outer-call visibility flag, not a universal cross-mode permission model by itself.

## 8. Permission and Extension Capability Specification

## 8.1 Mode Permissions

Mode defines capability boundaries:

1. `View`: read-only calls allowed, state writes forbidden.
2. `Pure`: state reads and writes forbidden.
3. `Main/P2sh/Abst`: as entry modes, each is limited by its call whitelist.

## 8.2 Extension Call Allowlist

Extension actions must satisfy two constraints:

1. ID must be in allowlist.
2. Mode must satisfy action-level permission requirements.

State-mutating extension actions are restricted to main-call semantics.

## 9. P2SH and Hook Specification

## 9.1 P2SH Proof

P2SH proof centers on binding script identity with execution payload:

1. Proof data must be recomputable into a unique script address.
2. Lockbox code must pass executable legality checks.
3. Witness and library list must satisfy capacity/type constraints.

Within one transaction, proof for the same script address should be unique to avoid semantic ambiguity.

## 9.2 Post-Action Hook

Transfer-type actions may trigger VM hooks after the action body executes, enabling script authorization and contract abstract callbacks. This bridges "asset actions" and "business logic actions" through a unified entry.

## 10. Consistency, Rollback, and Error Propagation

## 10.1 Snapshot Scope

Branch-execution snapshots should include:

1. State branch
2. VM volatile runtime state
3. Log cursor
4. Context volatile data

## 10.2 merge / recover Semantics

1. merge: commit branch state and keep current runtime trajectory.
2. recover: roll back to snapshot point.

Resource consumption (e.g., gas) is not rolled back by recover.

## 10.3 Error Semantics

Errors should satisfy unified behavior:

1. Failures are observable by upper layers and terminate current path.
2. Branch-style execution lets framework choose merge or recover.
3. Top-level call failures must not leave inconsistent intermediate state.

## 11. Runtime Invariants (Testing and Review Baseline)

The following invariants are recommended as long-term regression baselines:

1. Executable code has passed legality checks before entering execution.
2. Context level is correctly restored before/after VM entry call.
3. Context address and code owner address semantics are not mixed.
4. Inheritance resolution is acyclic, library indices are verifiable, and `libidx` lookup uses local target table semantics.
5. Extension capability is strictly constrained by allowlist and mode.
6. Branch recover does not roll back consumed resources.
7. Contract revision is monotonically increasing and update flow is auditable.
8. Cache hits change performance only, not semantics.

## 12. Testing Manual Recommendations (Scenario-Based)

Recommended test dimensions:

1. Wiring and initialization: with/without VM feature, with/without assigner, entry fallback initialization.
2. Deployment and update: happy paths, invalid structures, revision conflicts, self-reference conflicts.
3. Loading and resolution: inheritance chains, library indices, combinations of `this/self/super/libidx`.
4. Call dispatch: normal calls, CALLCODE, tail returns, exception propagation.
5. Mode permissions: call and extension boundaries across Main/P2sh/Abst/View/Pure.
6. P2SH and hooks: proof uniqueness, transfer-to-callback linkage.
7. Rollback semantics: successful-branch merge, failed-branch recover, monotonic consumption.

## 13. Change Management Recommendations

When behavior changes, produce three synchronized outputs:

1. Spec change note: what boundary behavior changed.
2. Test change note: what scenarios were added/modified.
3. Compatibility note: whether historical transactions or legacy contract expectations are affected.

---

This file is a runtime-layer behavioral contract document.  
It does not replace implementation code, but should be referenced before implementation during testing and review.
