# strkind

Semantic string newtypes, generic over their storage: one type per *kind* of
string, any string-like backing (`String`, `str`, `Arc<str>`, `Box<str>`,
[`SmolStr`](https://docs.rs/smol_str), ...).

```rust
strkind::strkind! {
    /// Identifies a conversation thread.
    pub ThreadId;

    /// A git commit hash.
    pub CommitId;
}

// The default storage is `String`; the borrowed form is `&ThreadId<str>`,
// mirroring the `String`/`str` pair with a single type name.
let owned: ThreadId = ThreadId::from("thread-7");
let borrowed: &ThreadId<str> = &owned; // deref coercion
assert_eq!(borrowed, ThreadId::from_ref("thread-7"));

// Other storages plug in without a new type name.
use std::sync::Arc;
let shared: ThreadId<Arc<str>> = owned.clone().convert();
assert_eq!(shared, owned); // storages compare directly
```

## Motivation

Codebases that pass many kinds of IDs around as bare `String`s let a thread
ID be handed to a function expecting a commit hash without complaint.
`strkind` separates what a string **means** (its kind — the type name) from
how it is **stored** (the type parameter):

```rust
pub struct ThreadId<T: ?Sized + AsRef<str> = String>(T);
```

- **One type name per kind, not an owned/borrowed pair.** `ThreadId` is the
  owned form and `&ThreadId<str>` the borrowed form, mirroring `String`/`str`
  without the `PathBuf`/`Path` two-type pattern (which would mean two names ×
  every ID kind, and no storage flexibility).
- **Storage-agnostic by construction.** `ThreadId<Arc<str>>` or
  `ThreadId<SmolStr>` work where cheap clones matter (`SmolStr`'s heap variant
  is `Arc`-backed, so clones are O(1) even for 36-char UUIDs; short IDs fit
  its ≤23-byte inline representation for free). Cross-storage
  `PartialEq`/`PartialOrd` impls let mixed storages compare directly, and
  changing a codebase-wide default later is a one-line edit at the alias or
  field, not a new type.
- **Transparent serde.** A kind serializes exactly like the plain string,
  including as a JSON map key, so adopting `strkind` changes no wire or save
  formats.

## Generated API

Each kind is a single `#[repr(transparent)]` struct with a private field,
generic over its storage, mirroring the `String`/`str` pattern with one type
name: constructors (`new`, `From<&str>`/`From<String>`/`FromStr`, and
zero-copy `from_ref` —
the same cast as `Path::new`), `convert::<U>()` for changing storage,
cross-storage comparisons and a storage-uniform `Hash`,
`Deref`/`Borrow`/`ToOwned` (so maps keyed by any storage support lookup by
`&Name<str>`), `AsRef<str>` and `AsRef<Name<str>>` (so APIs can take
`impl AsRef<Name<str>>` over any storage), and transparent serde.

Rather than listing every generated item here, see the full rendered API of
an example kind at
[`example::ExampleId`](https://docs.rs/strkind/latest/strkind/example/struct.ExampleId.html).

Downstream traits can stay monomorphic and object-safe by taking
`&Name<str>` in their methods, containing the generic machinery to the types
themselves.

## Comparison with similar crates

- [`aliri_braid`](https://docs.rs/aliri_braid) generates an owned/borrowed
  *pair* of types per kind (the `PathBuf`/`Path` pattern). `strkind` generates
  a single type whose storage is a parameter, so third-party storages
  (`Arc<str>`, `SmolStr`, ...) work without new type names.
- [`nutype`](https://docs.rs/nutype) focuses on *validated* newtypes over many
  inner types. `strkind` deliberately does no validation: it is purely about
  naming kinds of strings and being generic over their storage.
- Global interners ([`lasso`](https://docs.rs/lasso),
  [`ustr`](https://docs.rs/ustr)) offer cheap clones via global mutable state,
  at the cost of custom serde on every wire type and unbounded growth in
  long-lived processes. When distinct live IDs number in the hundreds,
  `Arc`-backed storage already makes clones O(1) without any of that.

## Features and `no_std`

`strkind` is `#![no_std]`-compatible. MSRV is Rust 1.85.

| Feature | Effect                                            |
|---------|---------------------------------------------------|
| `std`   | Enables `alloc`, and `std` support in optional dependencies. |
| `alloc` | The `alloc`-backed API: the `String` **default** storage parameter, `From<&str>`/`From<String>`, and `ToOwned` for the borrowed form. |
| `serde` | `Serialize`/`Deserialize` for generated kinds.    |
| `verus` | A [Verus](https://github.com/verus-lang/verus) verification boundary for generated kinds (see below). Adds a feature-gated `vstd` dependency; ghost code compiles away under plain cargo. |

The default features are `std` and `serde` (and therefore `alloc`, via
`std`).

Without `alloc`, kinds are core-only: explicit storages (`Name<&str>`,
`Name<heapless::String<N>>`, ...) and the borrowed `&Name<str>` form still
work, but `Name` must always be written with an explicit storage parameter.

Which impls a downstream `strkind!` expansion gets is decided by *strkind's*
features, not the downstream crate's: the feature-dependent pieces are
emitted through helper macros that are themselves `cfg`-gated inside strkind.

## Verus

With the `verus` feature, `strkind!` additionally emits a
[Verus](https://github.com/verus-lang/verus) verification boundary for each
kind, so verified code can call `as_str` and reason about kinds by their
string content. The feature adds [`vstd`](https://crates.io/crates/vstd) as a
dependency of strkind (vstd is Verus-version-coupled: strkind declares a
minimum, so pin the release matching your Verus toolchain in your lockfile).
The emission sits inside `#[cfg(verus_keep_ghost)]` blocks — set only by the
Verus driver — so under plain cargo the feature compiles to nothing beyond
building vstd as an ordinary (ghost-erased) dependency. strkind itself
carries the Verus declarations the boundary needs (such as the external-trait
specification for `AsRef`, which vstd does not yet provide), exported to
consumers through the driver like vstd's own.

The one piece of setup: when strkind's `verus` feature is enabled anywhere
in the build (cargo unifies features, so this includes crates that did not
enable it themselves), every crate that invokes `strkind!` must declare the
cfg, or plain-cargo builds warn (`unexpected_cfgs`) about the emitted
`#[cfg(verus_keep_ghost)]` blocks:

```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ["cfg(verus_keep_ghost)"] }
```

(A `cfg` emitted by a macro is checked against the invoking crate's declared
cfgs, and the boundary's impls must sit at module scope, where strkind
cannot attach an `allow`. The cfg also cannot be resolved inside strkind
instead: a non-verified crate in a verified build expands `strkind!` as plain
Rust, so it must see the cfg as unset even when strkind is compiled by the
driver.)

Each kind `Name` then carries, for every storage `T`:

- an uninterpreted view: `impl View for Name<T>` with `type V = Seq<char>`
  (vstd's `str`/`String` view type);
- an uninterpreted purity marker `Name::<T>::storage_obeys_str_model()`,
  granted by trusted axiom for the `String` and `str` storages
  (`Name::axiom_string_storage_obeys_str_model`,
  `Name::axiom_str_storage_obeys_str_model`);
- a specification for `as_str`, conditioned on that marker:
  `Name::<T>::storage_obeys_str_model() ==> s@ == id@`.

So verified code compares kinds by content (vstd already specifies `&str`
equality):

```rust,ignore
verus! {
fn same_id(a: &ThreadId, b: &ThreadId) -> (r: bool)
    ensures r == (a@ == b@),
{
    broadcast use ThreadId::axiom_string_storage_obeys_str_model;
    a.as_str() == b.as_str()
}
}
```

The purity axioms are `broadcast` axioms, so bring them into scope with
`broadcast use`. Doing so **inside each function** that needs them, as above,
always works. A module-level `broadcast use` in the same module as the
`strkind!` invocation does not: Verus reports a definition cycle with the
kind's own emitted items.

**What is trusted.** One definitional axiom per kind (the view *is* whatever
`as_str` returns — byte-for-byte the shape of vstd's own `String::as_str`
axiom) plus two ground purity facts (`String`'s and `str`'s `AsRef<str>`
impls are deterministic — true by inspection). The conditioning mirrors
vstd's own pattern for user-implementable traits (`obeys_key_model` in
`vstd::std_specs::hash`): a storage with a wrong or impure `AsRef<str>` impl
simply gets *no* `as_str` postcondition, so it cannot be used to prove
`false`. To use another known-pure storage (`Arc<str>`, `SmolStr`, ...),
grant the marker yourself — the trust is then yours, in your crate:

```rust,ignore
verus! {
broadcast axiom fn axiom_arc_str_obeys_str_model()
    ensures #[trigger] ThreadId::<Arc<str>>::storage_obeys_str_model();
}
```

**What is deliberately absent.** A view-injectivity axiom (`a@ == b@ ==>
a == b`): vstd has none for `String` either (content-equal values need not be
the same spec value). Write specifications over views (`a@`), not values.

**Compatibility caveat.** strkind declares Verus's external-trait
specification for `AsRef`, of which Verus allows only one per crate graph. A
future vstd that ships its own would conflict until strkind drops its
declaration, and two semver-incompatible strkind versions with `verus`
enabled cannot be verified in the same graph.

The boundary is verified in CI against a pinned Verus release by the
`verus-test/` crate, which doubles as a usage example. To run the
verification locally, use `./scripts/verus-verify.sh` (needs Docker; it
downloads the pinned Verus release and z3) or run `cargo verus verify` in
`verus-test/` with a matching Verus toolchain on your `PATH`.

## License

Licensed under the [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).
