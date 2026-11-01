# temper: TideOS Eval Kit (Tool 7)

<div align="center">

**pass@k vs pass^k multi-trial reliability evaluation engine at real sampling temperatures.**

[![CI](https://github.com/TechTideOhio/temper/actions/workflows/ci.yml/badge.svg)](https://github.com/TechTideOhio/temper/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TechTideOhio/temper?color=blue&label=release)](https://github.com/TechTideOhio/temper/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-emerald.svg)](LICENSE)
[![Rust: 2021 / 1.82+](https://img.shields.io/badge/Rust-2021%20%2F%201.82%2B-orange.svg)](Cargo.toml)
[![Zero-FFI](https://img.shields.io/badge/Dependencies-Zero--FFI%20Pure%20Rust-purple.svg)](Cargo.toml)

[Quickstart](#60-second-quickstart) | [The 80% Cliff](#the-80-per-run-reliability-cliff) | [Core Capabilities](#core-capabilities) | [The Complete TideOS Eval Kit](#the-complete-tideos-eval-kit) | [Contributing](CONTRIBUTING.md)

</div>

---

## Why temper?

Most AI agent evaluations suffer from the **Single-Run Demo Trap**: an evaluation case is executed once at greedy temperature ($T = 0.0$), passes, and is declared ready for production.

In production, agents run at non-zero temperature ($T = 0.7$) over multi-turn workflows. Evaluating single-run success or reporting capability retry metrics (`pass@k`) masks catastrophic stability failures:

> **pass@k = succeeded at least once in k runs.** (Capability demo metric)  
> **pass^k = succeeded in all k runs.** (Production stability metric)  
> **At 80% per-run success, pass^5 drops to roughly 33%. Production reliability lives in pass^k, so run each case multiple times at your real temperature.**

When an agent succeeds 80% of the time per run:
- **pass@5 is 99.97%**: If allowed 5 retries, it almost always finds the answer once. A cherry-picked video or marketing demo looks flawless.
- **pass^5 is 32.77% (roughly 33%)**: If executing a 5-step sequential task or running 5 times in a production workflow, it fails two out of three times.

`temper` is a pure-Rust multi-trial reliability engine that runs and scores cases across real temperatures, calculates unbiased combinatorial estimators, enforces cryptographic trial provenance, and catches flaky demo traps before they reach production.

---

## The 80% Per-Run Reliability Cliff

| k (Runs) | pass@k (At least 1 success) | pass^k (All k succeed) | Flake Wedge (Gap) | Production Meaning |
|:---:|:---:|:---:|:---:|---|
| **1** | 80.00% | 80.00% | 0.00% | Baseline single-run pass rate |
| **2** | 96.00% | 64.00% | 32.00% | 1 out of 3 2-step workflows fail |
| **3** | 99.20% | 51.20% | 48.00% | Coin flip stability across 3 runs |
| **4** | 99.84% | 40.96% | 58.88% | 60% failure rate in production |
| **5** | **99.97%** | **32.77% (~33%)** | **67.20%** | **The Reliability Cliff: 2 in 3 workflows fail** |
| **8** | 100.00% | 16.78% | 83.22% | Severe non-deterministic instability |
| **10** | 100.00% | 10.74% | 89.26% | 9 out of 10 10-step workflows fail |

---

## Core Capabilities

1. **Unbiased Combinatorial Estimators**:
   Implements Chen et al. (2021) HumanEval formulation for $\text{pass}@k$ and Yao et al. (2024) $\tau$-bench formulation for $\text{pass}\text{\textasciicircum}k$ when $n \ge k$ trials are sampled, with Beta-Binomial Bayesian smoothing and Wilson 95% confidence intervals.
2. **Eliminating Jensen's Inequality Pooling Distortion**:
   Early evaluation tools pooled all traces into a single aggregate pass rate $p = C/N$ and calculated $p^k$. Because $f(p) = p^k$ is strictly convex, pooled metrics distort true case reliability. `temper` audits and reports macro-average vs naive-pooled divergence (`jensen_pooling_distortion`).
3. **Real Sampling Temperature Verification**:
   Enforces that recorded trials were executed at the required deployment temperature (e.g. $T = 0.7 \pm 0.05$). Rejects greedy $T = 0.0$ runs submitted to production release gates (`TemperatureSpoofViolation`).
4. **Reliability Regime Classification**:
   Classifies each evaluation case into explicit domain states:
   - `DeterministicPass`: $\text{pass}\text{\textasciicircum}k = 1.0$ (rock solid)
   - `ProductionReliable`: $\text{pass}\text{\textasciicircum}k \ge \text{target}$
   - `DemoTrapFlaky`: $\text{pass}@k \ge 0.80$ while $\text{pass}\text{\textasciicircum}k < \text{target}$
   - `CapabilityDeficit`: $\text{pass}@k < \text{min\_pass\_at\_k}$
5. **Sequential Merkle Provenance Chaining**:
   Trial records are chained via SHA-256 hashes ($H(\text{prev\_hash} \parallel \text{trial})$). Removing a failed trial or cherry-picking runs invalidates the cryptographic chain.
6. **Multi-Trial Economic Cost Modeling**:
   Calculates $p_{50}, p_{95}$, mean latency, token consumption, and effective economic cost per reliable production delivery:
   $$\text{Expected Cost} = \frac{\text{Mean Trial Cost} \times k}{\text{pass}\text{\textasciicircum}k}$$
7. **sha256-framed-v1 Cryptographic Lockfiles**:
   Pins test suites, trial evidence digests, and evaluation targets with domain-tagged length-prefixed SHA-256 digests (`temper lock` / `temper verify`).
8. **Export Integrations**:
   Exports Mermaid `xychart-beta` decay curves, OpenTelemetry/OpenInference W3C spans, and `reval` baseline ratchet configurations.
9. **Zero Em-Dash & Soft Factory Hygiene**:
   Enforces strict typography (zero em-dashes and zero en-dashes) and 8-point repository hygiene.

---

## 60-Second Quickstart

### 1. Inspect the Reliability Cliff
```bash
temper table --p 0.80 --k 1,2,3,4,5,8,10
```

### 2. Score a Trial Evidence File
```bash
temper score --trials packs/fixtures/webhook_idempotency.jsonl --target-k 5 --temperature 0.7
```

### 3. Evaluate and Gate a Suite
```bash
temper check --suite packs/softfactory.toml --root .
```

### 4. Audit Repository Hygiene
```bash
temper repo .
```

---

## The Complete TideOS Eval Kit

`temper` is Tool 7 in the Complete TideOS Eval Kit:

| # | Tool | Crate | Release Binary | Core Responsibility |
|---|---|---|---|---|
| **1** | **`reval`** | `reval` | `2.07 MB` | **Trace Error Analysis, Gating & MCP Server** |
| **2** | **`gild`** | `gild` | `2.46 MB` | **Golden Dataset Governance & Stratification** |
| **3** | **`tessel`** | `tessel-eval` | `1.97 MB` | **Combinatorial Synthetic Scenario Generator** |
| **4** | **`assay`** | `assay-eval` | `2.09 MB` | **Deterministic Assertions First & In-Memory RLS** |
| **5** | **`tide-rill`** | `tide-rill` | `1.54 MB` | **Offline Retrieval Quality & Answer Faithfulness** |
| **6** | **`keel`** | `keel-eval` | `1.91 MB` | **Agent Trajectory Alignment & Step Efficiency (14 vs 3 calls)** |
| **7** | **`temper`** | `temper-eval` | `< 2.0 MB` | **Multi-Trial Reliability (pass@k vs pass^k at Real Temperature)** |

---

## License

MIT License. Copyright (c) 2026 TechTide Ohio.
