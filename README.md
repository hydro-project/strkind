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

For each kind `Name`, the `strkind!` macro generates:

- `struct Name<T: ?Sized + AsRef<str> = String>(T)` with a private field,
  keeping the representation encapsulated.
- `Name::new(storage)` to wrap any storage, `into_inner` to unwrap.
- `Name::from_ref(&str) -> &Name<str>` — a `const`, zero-copy cast for
  borrowed values, the same way `Path::new` casts `&str` to `&Path`.
- `as_str`, and `convert::<U>()` for changing storage (e.g. `String` →
  `Arc<str>`), preserving the kind.
- `Display`, `Debug`, `Clone`, `AsRef<str>`, `From<&str>`, and an infallible
  `FromStr`.
- Cross-storage `PartialEq`/`PartialOrd` (any two storages of the *same kind*
  compare; different kinds never do), plus `Eq`, `Ord`, and a `Hash` that is
  uniform across storages.
- `Deref<Target = Name<str>>` for owned storages, and
  `Borrow<Name<str>>`/`ToOwned` — generalized over any `Deref<Target = str>`
  storage, not just `String` — so `HashMap<Name<AnyStorage>, V>` and
  `BTreeMap<Name<AnyStorage>, V>` support lookup by `&Name<str>`, mirroring
  how `HashMap<String, V>` supports lookup by `&str`. `Hash` going through
  `as_ref()` uniformly is what upholds the `Borrow` contract across storages.
- `Serialize`/`Deserialize` delegating to the storage: a kind serializes
  exactly like the plain string, including as a JSON map key. (Requires the
  `serde` feature, on by default.)

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

## Features

| Feature | Default | Effect                                            |
|---------|---------|---------------------------------------------------|
| `serde` | ✔       | `Serialize`/`Deserialize` for generated kinds.    |

## License

Licensed under the [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).
