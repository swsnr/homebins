# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Do not fail if version check pattern doesn't match; instead assume that the binary is not installed.
    This supports multiple variants of the same binary, e.g. Hugo and Hugo Extended (see [GH-10]).

[GH-10]: https://github.com/lunaryorn/homebins/issues/10

## [0.0.4] – 2020-06-30

### Changed

- Manifest repositories now use the `main` branch instead of `master`.

## [0.0.3] – 2020-06-15

Third prerelease.

### Fixed

- Overwrite existing target files.

## [0.0.2] – 2020-06-15

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
[0.0.2]: https://github.com/lunaryorn/homebins/compare/v0.0.1...v0.0.2
[0.0.3]: https://github.com/lunaryorn/homebins/compare/v0.0.2...v0.0.3
[0.0.4]: https://github.com/lunaryorn/homebins/compare/v0.0.3...v0.0.4
[Unreleased]: https://github.com/lunaryorn/homebins/compare/v0.0.4...HEAD
