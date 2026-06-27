# HIP-25 HACD Staking (fullnodedev draft)

**Status:** Port in progress on branch `hip-25-port-draft`  
**Reference implementation:** https://github.com/Moskyera/rust/tree/hip-25-staking @ `v0.1.0-hip25-reference`  
**Spec:** https://github.com/Moskyera/rust/blob/hip-25-staking/docs/HIP25_SPEC.md

## Action kind mapping (important)

Legacy `hacash/rust` fork used kinds **34/35** for stake/unstake.  
**fullnodedev already assigns:**

| Kind | Action |
|------|--------|
| 34 | DiaInscEdit (HIP-22) |
| 35 | DiaInscMove |
| 36 | DiaInscDrop |

HIP-25 on fullnodedev uses **37 = DiaStake**, **38 = DiaUnstake**.

## Diamond status

| Value | Meaning |
|-------|---------|
| 4 | Staked |
| 5 | Staking cooldown |

## Economics (v3)

Redirect **10% DiamondMint `fee_got`** to staking pool when active.  
Inscription fees remain 100% burn. See reference `docs/HIP25_ECONOMICS_V3.md`.

## Config

```ini
[mint]
staking_activation_height = 0
```

`0` = disabled until community agrees a fork height.

## Port phases

| Phase | Scope | Status |
|-------|--------|--------|
| 1 | Types, actions 37/38, stake/unstake core | Done |
| 2 | Block-close rewards + miner-share redirect | Done |
| 3 | RPC `/query/staking/*` | TODO |
| 4 | Tests ported from reference | TODO |

## Maintainer note

Draft PR for review — not activated on mainnet. Coordinate with Istanbul (765432) timing.