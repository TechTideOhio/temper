# Changelog

All notable changes to `temper` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-01

### Added
- Initial release of Tool 7 in The Complete TideOS Eval Kit.
- Pure-Rust multi-trial reliability evaluation engine: pass@k vs pass^k.
- Unbiased combinatorial estimators (Chen et al., 2021 and Yao et al., 2024).
- Real sampling temperature verification and temperature spoof firewalls.
- Flake wedge and reliability regime classification: DeterministicPass, ProductionReliable, DemoTrapFlaky, TemperatureBrittle, and CapabilityDeficit.
- Sequential SHA-256 Merkle trial provenance chaining.
- Cryptographic lockfile generation and drift verification (sha256-framed-v1).
- Multi-trial execution runner with early fail-fast for pass^k gates.
- Ingestion of agent-harness and reval execution traces.
- Multi-trial latency, token, and economic cost modeling.
- OpenTelemetry and OpenInference span export.
- Soft Factory 8-point repository hygiene auditor.
- Terminal audio alert (\x07) on pass^k cliff and temperature spoof anomalies.
