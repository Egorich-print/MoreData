# Known CI failures — what is currently red and why

> **This file is a marker for agents and for the owner.** Every item below was
> reproduced on a real CI run (2026-11-02) and points at exact `file:line`.
> If you fix one — remove the item here and say so in the commit message.
>
> Context: the repository is built by Jenkins (multibranch project `MoreData`,
> branch `mission/moredata`) and by GitHub Actions (same branch). **Both are red
> for the same reason**, so this is not an environment problem of the agent.

---

## 1. `_parallel_pool_stress` — wall-clock bound is too tight — **open**

**Symptom**

```
test _parallel_pool_stress ... FAILED
thread '_parallel_pool_stress' panicked at crates/moredata-scheduler/tests/contract.rs:197:5:
parallel block spike 1876813ns
test result: FAILED. 10 passed; 1 failed; ...
```

**Where**

`crates/moredata-scheduler/tests/contract.rs:196-197`:

```rust
// OS-thread pool adds scheduling overhead; keep the bound.
assert!(max_ns < 1_000_000, "parallel block spike {max_ns}ns");
```

The bound is 1 ms; the measured peak is well above it.

**Measured peak vs the 1 ms bound** (same test, same commit, different machines):

| Machine | Peak | Result |
|---|---|---|
| GitHub Actions (ubuntu-latest) | 1 876 813 ns (×1.9) | ❌ |
| Mac (Apple Silicon, idle) | 8 891 833 ns (×8.9) | ❌ |
| Jenkins agent `pve-xeon` | 5 500 298 ns (×5.5) | ❌ |
| Jenkins agent `pve-n150` | — | ❌ |

The test is red **everywhere**, including on GitHub Actions. It is not a property
of one machine: the assertion races a wall-clock limit against OS-thread pool
scheduling jitter, and the pool has 4 workers on a machine that may be busy.

**How to close it**

Pick one and make it deliberate:

1. **Deterministic signal instead of a clock** — wait for the pool to report the
   block as done (channel/condvar) and assert on ordering, not on elapsed time.
2. **Scale the bound to the machine** — derive the budget from a calibration
   measurement taken in the same process, so the test compares the parallel path
   against the serial path measured right next to it.
3. **Relax the constant** and document why the number is what it is.

Option 1 is the only one that makes the result independent of the machine.

**Verification**

```bash
cargo test -p moredata-scheduler --test contract
```

---

## 2. `scheduler_stress_bounded_block_time` — same shape, smaller bound — **open**

`crates/moredata-scheduler/tests/contract.rs:151`:

```rust
// 4-node graph, 4 workers: budget is the same as serial.
assert!(max_ns < 500_000, "scheduler block spike {max_ns}ns");
```

Bound 0.5 ms. This one passes on fast, idle machines and fails on a loaded
agent (measured 3 017 248 ns on the Jenkins container) — the same race as item 1,
just with less headroom. Fixing item 1 properly should be reviewed for this
assertion as well; leaving it as a bare wall-clock number keeps the test flaky.

---

## 3. `stress_control_churn_vs_rt` — the same race in another crate — **open**

Jenkins build `#9` (branch `mission/moredata`, 2026-11-02) failed here — a
different test and a different crate than items 1 and 2, so it was not covered
by them:

**Symptom**

```
---- stress_control_churn_vs_rt stdout ----
thread 'stress_control_churn_vs_rt' panicked at crates/moredata-runtime/tests/contract.rs:277:5:
max block 821885ns exceeds contract
test result: FAILED. 8 passed; 1 failed; ...
```

**Where** — `crates/moredata-runtime/tests/contract.rs:277`:

```rust
assert!(max_ns < 500_000, "max block {max_ns}ns exceeds contract");
```

Bound 0.5 ms, measured peak 821 885 ns (×1.6) on the Jenkins agent. Same shape
as item 1 and item 2: a wall-clock bound raced against scheduling jitter, in a
*runtime* crate this time — so fixing items 1 and 2 must be reviewed for this
assertion too, otherwise the crate just moves the red to a different file.

**Verification**

```bash
cargo test -p moredata-runtime --test contract
```

---

## 4. Scope of the CI gate

The repository's own workflow (`.github/workflows/ci.yml`) runs
`cargo test --workspace --verbose` in the **debug** profile. The timing bounds
above were therefore calibrated — if at all — against release performance, while
CI measures debug. Running the scheduler contract tests in release (`--release`)
is a cheap way to buy headroom, but it does not remove the race.
