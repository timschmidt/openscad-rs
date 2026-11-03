# Agent guidance for openscad-rs

openscad-rs is a syntactic OpenSCAD parser. Numeric tokens parse directly
into `hyperreal::Real`, so literals keep their exact rational meaning. See
`README.md` for the API, coverage and development commands.

## Compatibility corpus: ratchet, never lower

`tests/openscad_compat.rs` parses the vendored upstream OpenSCAD test corpus.

The corpus is pinned as the `vendor/openscad` submodule. The ratchet
(D03 of csgrs's `docs/research/simplification-and-api-program.md`) works like
this:

- `tests/openscad_compat_passing.txt` lists exactly the files that parse.
- The test fails when a listed file stops parsing, and also when an unlisted
  file starts parsing, so every fix is locked in.
- After an intentional change, regenerate the list with
  `OPENSCAD_COMPAT_BLESS=1 cargo test --test openscad_compat`. Commit it in the
  same change, and say in the commit why the set changed.
- CI sets `OPENSCAD_COMPAT_REQUIRED=1`, so a missing corpus fails instead of
  skipping. Before this, `/vendor/` was gitignored and the submodule had no
  gitlink, so the job always skipped.
- Updating the submodule pin changes the corpus. Regenerate the list in the same
  change and review every added or removed file.

Never remove a file from the list to make a change pass, and never special-case
corpus files.

### Files that do not parse (pin `8e8df3dd`)

| File | Classification |
| --- | --- |
| `issues/issue1890-{comment,include,string,use}.scad` | Deliberately invalid: in upstream's `FAILING_FILES` |
| `misc/unicode-identifiers-fail.scad` | Deliberately invalid: upstream expects a parser error |
| `issues/issue6942.scad` | Gap: backslash-newline continuation inside a string |
| `misc/no-break-space-test.scad`, `misc/nbsp-utf8-test.scad`, `misc/nbsp-latin1-test.scad` | Gap: no-break space as whitespace |
| `misc/unicode-identifiers.scad`, `customizer/unicode-identifiers-parameters.scad` | Gap: Unicode identifiers normalized to NFC |

If a deliberately invalid file ever starts parsing, the ratchet reports it as
newly passing. That is a regression, not a fix.

### Downstream

Evaluate the same corpus in SynapsCAD and diff the results against OpenSCAD
with its Manifold backend. This yields a SCAD corpus and a float-kernel
comparison. Every difference is triaged and published, not silently absorbed.

### Building

`Cargo.toml` takes `hyperreal` from `../hyperreal` at version 0.13.1. The csgrs
monorepo keeps hyperreal at `crates/hyperreal`, at version 0.14.0, and
crates.io has 0.13.0. So neither CI nor a plain checkout beside csgrs can
resolve the dependency, and CI has failed on that since 2026-10-05. Choosing how
this crate takes hyperreal (published version, git dependency or a sibling
path) is a maintainer decision.
