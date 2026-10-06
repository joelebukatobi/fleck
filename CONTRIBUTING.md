# Contributing to Fleck

Patches, bug reports and translations are all welcome. Fleck is GPL-3.0-or-later.

## Before you start

[BUILD.md](BUILD.md) covers the dependencies, the build, and the packages.
[docs/ux.md](docs/ux.md) describes how Fleck is meant to behave, in plain
language — it is the source of truth for the product, so a change in behaviour
or appearance belongs there in the same commit as the code.

For anything larger than a fix, open an issue first. Fleck tries to stay a
small, quiet app, and the most useful thing a discussion settles is whether a
feature belongs in it at all.

## House rules

- **`just check` must pass** before you commit. It is exactly what CI runs.
- **Clippy is pedantic.** Opt-outs live in the root `Cargo.toml` with a reason;
  a local exception is fine where it is justified in place.
- **Every user-visible string goes through `fl!()`** into
  `fleck/i18n/en/fleck.ftl`. Message ids are checked against English at compile
  time, and a test asserts every other catalogue carries the same ids.
- **Use COSMIC's design system** rather than widget defaults: spacing in
  multiples of 8 where a platform number does not force otherwise, colours from
  the theme, and `fleck/src/app/style.rs` for anything derived from it.
- **Tests go beside the code** they cover. `core/` has no GUI dependencies, so
  logic that can live there and be tested there, should.

## Translating

Catalogues are [Fluent](https://projectfluent.org) files at
`fleck/i18n/<locale>/fleck.ftl`. To add a language, copy the English one into a
new locale directory and translate the values — the message ids must stay as
they are:

```bash
cp -r fleck/i18n/en fleck/i18n/sv
```

Then `cargo test -p fleck i18n`, which fails if a catalogue is missing a
message, invents one, or does not parse.

Two things to watch. Plural forms use your language's own categories, so
`[one]`/`[other]` is right for English and Spanish but not for every language;
write the categories CLDR defines for yours, and keep `*[other]` as the
default. And the counts arrive as numbers, so `{ $count }` selects correctly
rather than matching a string.

The catalogues that shipped with Fleck beyond English — German, Spanish,
French, Italian, Dutch, Brazilian Portuguese and Simplified Chinese — have not
been reviewed by native speakers. Corrections are especially welcome, and a
correction needs no issue.

## Commits

One change per commit, with a message that says what changed and why in prose.
The body is for the reasoning a reader would otherwise have to reconstruct from
the diff.
