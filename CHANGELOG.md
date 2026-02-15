# v0.1.3

## Features
- Add support of custom tokenization with Rhai scripts.
- Add similar token popup and related functionality.

## CI/Deps
- Improve CI configuration.
- Bump dependencies in the cargo group.

# v0.1.2

## Breaking changes
- Update JSON project format.

  Changes:
  - `"version": 1` -> `"version": 2`,
  - `"vocabulary"` -> `"vocabulary.original"`,
  - `"formatted_word"` -> `"vocabulary.formatted"`,
  - For `"sentences.*.words"`, only store signed integer in new version, non-negative means index of `"vocabulary.original"` (0-indexed), negative means index of `"vocabulary.formatted"` (1-indexed).

## Features
- Add formatting chain popup and related functionality.
- Add support for formatted word comments.

## Fixes
- Validate project version on load to prevent unsupported versions.
- Prevent closing when there are unsaved changes in WASM builds.
- Fix word edit box color when empty.
- Mark projects loaded from text as dirty.

## CI/Deps
- Fix CI configuration.
- Bump GitHub Actions dependencies.
- Bump bytes dependency in the cargo group.

# v0.1.1

## Changes
- Auto-loading custom fonts is disabled.

## Features
- Add WASM support.
- Add a new icon.

## Fixes
- Hide console in Windows release builds.
- Fix CI configuration.

# v0.1.0
Initial release.
