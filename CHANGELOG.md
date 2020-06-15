# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Second prerelease.

### Fixed

- Copy target files atomically, and properly update running executables to support self-update.

## [0.0.1] – 2020-06-15

Initial prerelease.

### Added

- Clone default manifest repo from <https://github.com/lunaryorn/homebin-manifests>.
- Add commands for manifests from the manifest repo: `list`, `outdated`, `installed`, `files`, `install`, `remove` and `update`.
- Add corresponding `manifest-` commands to work on manifest files.
- Check `$HOME` and `manpath` and warn if these variables to not include `~/.local/`.

[0.0.1]: https://github.com/lunaryorn/homebins/releases/tag/v0.0.1
[Unreleased]: https://github.com/lunaryorn/homebins/compare/v0.0.1...HEAD