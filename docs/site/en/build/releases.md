---
title: Releases
description: Cut from a private pipeline that holds the signing key; one feature release a week.
sidebar:
  order: 7
---

Maintainers cut releases from a private pipeline that holds the signing key. It builds exactly the tagged source in the public repository, signs the updater artifacts, verifies every signature and publishes to the repository's Releases; installed apps only accept updates signed by that key.

## The rules

- **One feature release a week at most.** A bug-fix release (a patch bump, same major.minor) is exempt.
- **The tag must match the app version** in `src-tauri/tauri.conf.json` and `Cargo.toml`; the pipeline bumps both when it cuts.
- **The docs must be in step**: the translation gate runs before a build.
- **Asset names carry no version**, so `releases/latest/download/<name>` links keep working forever.

## What a release carries

A `.dmg` and a signed updater tarball for Apple Silicon and for Intel, the `hark` CLI for both, checksums, and `latest.json` for the in-app updater. Pushing to `main` never releases anything: the release is a deliberate, manual dispatch.
