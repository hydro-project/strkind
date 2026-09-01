//! Semantic string newtypes, generic over their storage.
//!
//! `strkind` separates what a string *means* (its kind: a thread ID, a commit
//! hash, a user name) from how it is *stored* (`String`, `&str`, `Arc<str>`,
//! `Box<str>`, `SmolStr`, ...). The [`strkind!`] macro generates one newtype
//! per kind, generic over any string-like storage:
//!
//! ```
//! strkind::strkind! {
//!     /// Identifies a conversation thread.
//!     pub ThreadId;
//!
//!     /// A git commit hash.
//!     pub CommitId;
//! }
//!
//! // The default storage is `String`; the borrowed form is `&ThreadId<str>`,
//! // mirroring the `String`/`str` pair with a single type name.
//! let owned: ThreadId = ThreadId::from("thread-7");
//! let borrowed: &ThreadId<str> = &owned; // deref coercion
//! assert_eq!(borrowed, ThreadId::from_ref("thread-7"));
//!
//! // Other storages plug in without a new type name.
//! use std::sync::Arc;
//! let shared: ThreadId<Arc<str>> = owned.clone().convert();
//! assert_eq!(shared, owned); // storages compare directly
//! ```
//!
//! # Generated API
//!
//! For each kind `Name`, the macro generates:
//!
//! - `struct Name<T: ?Sized + AsRef<str> = String>(T)` with a private field,
//!   keeping the representation encapsulated.
//! - `Name::new(storage)` to wrap any storage, `into_inner` to unwrap.
//! - `Name::from_ref(&str) -> &Name<str>` - a `const`, zero-copy cast for
//!   borrowed values, the same way `Path::new` casts `&str` to `&Path`.
//! - `as_str`, and `convert::<U>()` for changing storage (e.g. `String` →
//!   `Arc<str>`), preserving the kind.
//! - `Display`, `Debug`, `Clone`, `AsRef<str>`, `From<&str>`, and an
//!   infallible `FromStr`.
//! - Cross-storage `PartialEq`/`PartialOrd` (any two storages of the *same
//!   kind* compare; different kinds never do), plus `Eq`, `Ord`, and a `Hash`
//!   that is uniform across storages.
//! - `Deref<Target = Name<str>>` for owned storages, and
//!   `Borrow<Name<str>>`/`ToOwned`, so `HashMap<Name, V>` and
//!   `BTreeMap<Name, V>` support lookup by `&Name<str>` - mirroring how
//!   `HashMap<String, V>` supports lookup by `&str`.
//! - `Serialize`/`Deserialize` delegating to the storage (a kind serializes
//!   exactly like the plain string, including as a JSON map key). Requires
//!   the `serde` feature, which is on by default.
//!
//! # Comparison with similar crates
//!
//! - [`aliri_braid`](https://docs.rs/aliri_braid) generates an owned/borrowed
//!   *pair* of types per kind (the `PathBuf`/`Path` pattern). `strkind`
//!   generates a single type whose storage is a parameter, so third-party
//!   storages (`Arc<str>`, `SmolStr`, ...) work without new type names.
//! - [`nutype`](https://docs.rs/nutype) focuses on *validated* newtypes over
//!   many inner types. `strkind` deliberately does no validation: it is
//!   purely about naming kinds of strings and being generic over their
//!   storage.

// Re-exports for macro-generated code. Not public API.
#[cfg(feature = "serde")]
#[doc(hidden)]
pub mod __private {
    pub use serde;
}

/// Define one or more string kinds.
///
/// Each item is `[visibility] Name;`. See the [crate docs](crate) for the
/// full generated API.
///
/// ```
/// strkind::strkind! {
///     /// Identifies a conversation thread.
///     pub ThreadId;
///
///     /// A git commit hash.
///     pub CommitId;
/// }
///
/// let id = CommitId::from("abc123");
/// assert_eq!(id.as_str(), "abc123");
/// ```
#[macro_export]
macro_rules! strkind {
    () => {};

    (
        $(#[$meta:meta])*
        $vis:vis $Name:ident ;
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        #[repr(transparent)]
        $vis struct $Name<T: ?Sized + AsRef<str> = ::std::string::String>(T);

        impl<T: AsRef<str>> $Name<T> {
            /// Wrap `storage` as this kind.
            $vis fn new(storage: T) -> Self {
                Self(storage)
            }

            /// Unwrap into the underlying storage.
            $vis fn into_inner(self) -> T {
                self.0
            }

            /// Convert the storage type (e.g. `String` → `Arc<str>`),
            /// preserving the kind.
            $vis fn convert<U: AsRef<str> + ::core::convert::From<T>>(self) -> $Name<U> {
                $Name(U::from(self.0))
            }
        }

        impl<T: ?Sized + AsRef<str>> $Name<T> {
            /// View as a plain `&str`, for passing to APIs that require one.
            $vis fn as_str(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl $Name<str> {
            /// Cast a borrowed `&str` to a borrowed kind, without allocating.
            $vis const fn from_ref(s: &str) -> &Self {
                // SAFETY: `$Name` is `#[repr(transparent)]` over its single
                // field, so `$Name<str>` has the same layout as `str`. This
                // is the same cast `std` uses for `Path::new`.
                unsafe { &*(::core::ptr::from_ref::<str>(s) as *const Self) }
            }
        }

        impl<T: ?Sized + AsRef<str>> ::core::convert::AsRef<str> for $Name<T> {
            fn as_ref(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl<T: ?Sized + AsRef<str>> ::core::fmt::Display for $Name<T> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(self.0.as_ref(), f)
            }
        }

        // ── Comparisons: uniform across storages ──
        //
        // Any two storages of the same kind compare by string content, so a
        // `$Name<Arc<str>>` equals the `$Name<String>` it was converted from.
        // `Hash` matches, which also upholds the `Borrow` contract below.

        impl<A: ?Sized + AsRef<str>, B: ?Sized + AsRef<str>>
            ::core::cmp::PartialEq<$Name<B>> for $Name<A>
        {
            fn eq(&self, other: &$Name<B>) -> bool {
                self.0.as_ref() == other.0.as_ref()
            }
        }

        impl<T: ?Sized + AsRef<str>> ::core::cmp::Eq for $Name<T> {}

        impl<A: ?Sized + AsRef<str>, B: ?Sized + AsRef<str>>
            ::core::cmp::PartialOrd<$Name<B>> for $Name<A>
        {
            fn partial_cmp(&self, other: &$Name<B>) -> ::core::option::Option<::core::cmp::Ordering> {
                self.0.as_ref().partial_cmp(other.0.as_ref())
            }
        }

        impl<T: ?Sized + AsRef<str>> ::core::cmp::Ord for $Name<T> {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.0.as_ref().cmp(other.0.as_ref())
            }
        }

        impl<T: ?Sized + AsRef<str>> ::core::hash::Hash for $Name<T> {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.0.as_ref().hash(state)
            }
        }

        // ── The `String`/`str` pattern ──

        impl<T: ?Sized + AsRef<str> + ::core::ops::Deref<Target = str>> ::core::ops::Deref
            for $Name<T>
        {
            type Target = $Name<str>;
            fn deref(&self) -> &$Name<str> {
                $Name::from_ref(self.0.as_ref())
            }
        }

        impl<T: AsRef<str> + ::core::ops::Deref<Target = str>>
            ::std::borrow::Borrow<$Name<str>> for $Name<T>
        {
            fn borrow(&self) -> &$Name<str> {
                self
            }
        }

        impl ::std::borrow::ToOwned for $Name<str> {
            type Owned = $Name<::std::string::String>;
            fn to_owned(&self) -> Self::Owned {
                $Name(self.0.to_owned())
            }
        }

        impl ::core::convert::From<&str> for $Name<::std::string::String> {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl ::core::str::FromStr for $Name<::std::string::String> {
            type Err = ::core::convert::Infallible;
            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                ::core::result::Result::Ok(Self(s.to_owned()))
            }
        }

        // ── Serde (with the `serde` feature): a kind serializes exactly
        // like its storage ──

        $crate::__strkind_serde! { $Name }

        $crate::strkind! { $($rest)* }
    };
}

/// Serde impls for a generated kind - expands to nothing when strkind is
/// built without the `serde` feature. Not public API.
///
/// The feature is resolved when *strkind* is compiled (the two definitions
/// below are `cfg`-gated), so downstream expansions of [`strkind!`] follow
/// strkind's feature rather than the downstream crate's.
#[cfg(feature = "serde")]
#[doc(hidden)]
#[macro_export]
macro_rules! __strkind_serde {
    ( $Name:ident ) => {
        impl<T: ?Sized + AsRef<str> + $crate::__private::serde::Serialize>
            $crate::__private::serde::Serialize for $Name<T>
        {
            fn serialize<S: $crate::__private::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }

        impl<'de, T: AsRef<str> + $crate::__private::serde::Deserialize<'de>>
            $crate::__private::serde::Deserialize<'de> for $Name<T>
        {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                T::deserialize(deserializer).map(Self)
            }
        }
    };
}

#[cfg(not(feature = "serde"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __strkind_serde {
    ( $Name:ident ) => {};
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;

    use smol_str::SmolStr;

    strkind! {
        /// A kind for tests.
        pub(crate) ThreadId;

        /// Another kind for tests.
        pub(crate) RawTag;
    }

    // A second macro invocation must still produce distinct types.
    strkind! {
        pub(crate) OtherId;
    }

    #[test]
    fn new_wraps_and_unwraps() {
        let id = ThreadId::new("thread-7".to_owned());
        assert_eq!(id.as_str(), "thread-7");
        assert_eq!(id.to_string(), "thread-7");
        assert_eq!(id.into_inner(), "thread-7");
    }

    #[test]
    fn from_and_from_str() {
        let id = ThreadId::from("thread-7");
        assert_eq!(id.as_str(), "thread-7");
        let parsed: ThreadId = "thread-7".parse().expect("infallible");
        assert_eq!(parsed, id);
    }

    #[test]
    fn from_ref_is_const_and_zero_copy() {
        const ID: &ThreadId<str> = ThreadId::from_ref("thread-7");
        assert_eq!(ID.as_str(), "thread-7");
        assert_eq!(ID, &ThreadId::from("thread-7"));
    }

    #[test]
    fn deref_gives_borrowed_form() {
        fn takes_borrowed(id: &ThreadId<str>) -> &str {
            id.as_str()
        }
        let owned = ThreadId::from("thread-7");
        assert_eq!(takes_borrowed(&owned), "thread-7");
    }

    #[test]
    fn to_owned_round_trips() {
        let borrowed: &ThreadId<str> = ThreadId::from_ref("thread-7");
        let owned: ThreadId = borrowed.to_owned();
        assert_eq!(owned, *borrowed);
    }

    #[test]
    fn map_lookup_by_borrowed_form() {
        let mut hash_map: HashMap<ThreadId, u32> = HashMap::new();
        hash_map.insert(ThreadId::from("a"), 1);
        assert_eq!(hash_map.get(ThreadId::from_ref("a")), Some(&1));
        assert_eq!(hash_map.get(ThreadId::from_ref("b")), None);

        let mut btree_map: BTreeMap<ThreadId, u32> = BTreeMap::new();
        btree_map.insert(ThreadId::from("a"), 1);
        assert_eq!(btree_map.get(ThreadId::from_ref("a")), Some(&1));
    }

    #[test]
    fn alternate_storages_interoperate() {
        let owned = ThreadId::from("thread-7");
        let shared: ThreadId<Arc<str>> = owned.clone().convert();
        let small: ThreadId<SmolStr> = ThreadId::new(SmolStr::new("thread-7"));

        // Cross-storage comparisons.
        assert_eq!(shared, owned);
        assert_eq!(small, owned);
        assert_eq!(shared, small);

        // Cross-storage map lookup via the borrowed form (`Borrow` + `Hash`
        // are uniform across storages).
        let mut map: HashMap<ThreadId<Arc<str>>, u32> = HashMap::new();
        map.insert(shared, 1);
        assert_eq!(map.get(ThreadId::from_ref("thread-7")), Some(&1));
    }

    #[test]
    fn kinds_are_distinct_types() {
        // `ThreadId` and `OtherId` must not unify.
        // (Compile-time property; this test documents it.)
        fn takes_thread_id(id: &ThreadId<str>) -> &str {
            id.as_str()
        }
        let other = OtherId::new("x".to_owned());
        // takes_thread_id(&other); // must not compile
        let thread = ThreadId::from("x");
        assert_eq!(takes_thread_id(&thread), other.as_str());
        assert_eq!(other.convert::<Box<str>>().into_inner().as_ref(), "x");

        // Second kind in the same macro invocation works identically.
        let tag = RawTag::new("x".to_owned());
        assert_eq!(tag.as_str(), "x");
        assert_eq!(tag.convert::<Arc<str>>().into_inner().as_ref(), "x");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_is_transparent() {
        let id = ThreadId::from("thread-7");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, r#""thread-7""#, "serializes as a plain string");
        let back: ThreadId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, id);

        // Borrowed form serializes identically.
        let borrowed_json =
            serde_json::to_string(ThreadId::from_ref("thread-7")).expect("serialize");
        assert_eq!(borrowed_json, json);

        // Alternate storage deserializes too.
        let small: ThreadId<SmolStr> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(small, id);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_map_keys() {
        let mut map: HashMap<ThreadId, u32> = HashMap::new();
        map.insert(ThreadId::from("a"), 1);
        let json = serde_json::to_string(&map).expect("serialize");
        assert_eq!(json, r#"{"a":1}"#);
        let back: HashMap<ThreadId, u32> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, map);
    }

    #[test]
    fn debug_names_the_kind() {
        assert_eq!(format!("{:?}", ThreadId::from("a")), r#"ThreadId("a")"#);
    }

    #[test]
    fn ordering_is_by_content() {
        let mut ids = [ThreadId::from("b"), ThreadId::from("a"), ThreadId::from("c")];
        ids.sort();
        let sorted: Vec<&str> = ids.iter().map(|i| i.as_str()).collect();
        assert_eq!(sorted, ["a", "b", "c"]);
        assert!(ThreadId::from("a") < ThreadId::from("b"));
    }
}
