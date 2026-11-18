# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.17.1...lpoco-manifest-v0.18.0) - 2026-11-18

### Added

- *(manifest)* device.toml declared once in Rust, unknown fields refused, schema projected ([#179](https://github.com/linux-phone-oco/lab-tools/pull/179))

## [0.17.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.17.0...lpoco-manifest-v0.17.1) - 2026-11-18

### Added

- device.toml declared once: every table and field, unknown ones refused by devlab, host and bringup alike, its JSON Schema (draft-07) projected and committed
