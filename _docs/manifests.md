---
title: Manifest support
---

Dalil treats common package, workspace, and build manifests as repository landmarks.

For `Cargo.toml`, `package.json`, and `pyproject.toml`, it also reads a bounded subset
of metadata that can point to runtime code, public library code, and common build, test,
or run commands.

The metadata appears under each project root's `manifest_metadata` array in JSON.
The evidence profile also prints it in the repository-landmarks section. When a
declared target resolves to a safe file in the selected repository scope, the default
reading plan can prefer that file over a conventional filename.

## Parsed metadata

### `Cargo.toml`

Dalil reads Cargo's package and target tables:

- `[lib]` identifies the library target. Its `path` overrides `src/lib.rs`.
- `[[bin]]` identifies named runtime targets. Its `path` overrides Cargo's inferred
  target path.
- `package.autolib` and `package.autobins` control whether Dalil considers Cargo's
  default `src/lib.rs` and `src/main.rs` targets.
- Reports include `cargo build`, `cargo test`, and a `cargo run --bin NAME` command for
  each identified binary.

Cargo can infer targets from the filesystem, so Dalil records an inferred default
only when the corresponding source file exists. It does not expand workspace members,
features, or build-script output.

See the [Cargo targets reference](https://doc.rust-lang.org/cargo/reference/cargo-targets.html).

### `package.json`

Dalil reads these npm package fields:

- `bin` identifies installed command entry points.
- `main`, `module`, and string paths nested under `exports` identify public module entry
  points.
- common script names identify commands: `build`, `build:*`, `test`, `test:*`, `start`,
  `dev`, `serve`, and `run`.
- `packageManager` selects `npm`, `pnpm`, `yarn`, or `bun` for script commands when
  declared. Dalil uses `npm` when the field is absent or unsupported.

Script bodies are not interpreted. Dalil reports the package-manager invocation, such
as `pnpm run test`, without running it.

Conditional and array exports are traversed only far enough to collect bounded string
targets. Commands with names that require shell-specific quoting are omitted
from the portable command list.

See npm's [`package.json` reference](https://docs.npmjs.com/files/package.json/) and
[script documentation](https://docs.npmjs.com/cli/using-npm/scripts/).

### `pyproject.toml`

Dalil reads standardized Python packaging metadata and one common test-tool table:

- `[project.scripts]` and `[project.gui-scripts]` identify installed command entry points.
- `[project].import-names` identifies public import packages when it is present.
- `[build-system]` adds the common `python -m build` command.
- `[tool.pytest]` adds the common `pytest` command.
- `[tool.poetry.scripts]` is accepted as a compatibility source for command entry points.

Python entry points name importable objects rather than files. Dalil checks the corresponding
module and package paths at the project root and under `src/`, then records a resolved path only
when one exists. It does not import Python modules or invoke a build backend. See the
[`pyproject.toml` specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
and the [entry-points
specification](https://packaging.python.org/en/latest/specifications/entry-points/).

## Recognized project manifests

The following files establish project roots or workspace landmarks. Metadata parsing beyond root
classification is currently limited to the three formats above.

| Ecosystem | Recognized files                                                                        |
| --------- | --------------------------------------------------------------------------------------- |
| Rust      | `Cargo.toml`                                                                            |
| Node.js   | `package.json`, `pnpm-workspace.yaml`, `pnpm-workspace.yml`, `lerna.json`, `nx.json`    |
| Go        | `go.mod`, `go.work`                                                                     |
| Python    | `pyproject.toml`, `setup.py`, `setup.cfg`                                               |
| Ruby      | `Gemfile`, `gemspec`, `*.gemspec`                                                       |
| JVM       | `pom.xml`, `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts` |
| .NET      | `*.csproj`, `*.sln`                                                                     |
| PHP       | `composer.json`                                                                         |
| Elixir    | `mix.exs`                                                                               |

The format descriptions come from the maintainers of each ecosystem:

- [Go modules](https://go.dev/doc/modules/gomod-ref)
- [Bundler](https://bundler.io/guides/getting_started.html)
- [Maven](https://maven.apache.org/pom.html)
- [Gradle](https://docs.gradle.org/current/userguide/gradle_basics.html)
- [.NET projects and solutions](https://learn.microsoft.com/en-us/visualstudio/ide/solutions-and-projects-in-visual-studio)
- [Composer](https://getcomposer.org/doc/04-schema.md)
- [Mix](https://hexdocs.pm/mix/Mix.Project.html)

## Limitations

Manifest reads use the report's per-file byte limit. Each parsed manifest retains at
most 16 runtime targets, 16 library targets, and 16 commands; individual names and
declarations are capped at 512 characters. The manifest metadata's `truncated` field
reports when an item cap discarded additional declarations.

Dalil validates declared paths as repository-relative paths and resolves them only
against files already visible in the selected scope. Absolute paths, parent traversal,
missing files, generated values, environment interpolation, plugin behavior, and
executable configuration are left unresolved.

Invalid JSON, TOML, or UTF-8 adds a limitation to the report while preserving the
manifest as a landmark.
