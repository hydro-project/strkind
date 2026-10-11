//! Verifies the trust seam emitted by `strkind!` with the `verus` feature:
//! the acceptance test from hydro-project/strkind#3.
#![deny(unsafe_code)]

use vstd::prelude::*;

strkind::strkind! {
    /// Identifies a conversation thread.
    pub ThreadId;

    /// A second kind: regression test that per-kind emission does not
    /// conflict (duplicate external trait/type declarations, proxy names).
    pub CommitId;
}

verus! {

/// The issue's acceptance sketch: verified string-identity comparison through
/// `as_str`, with the postcondition stated over views.
pub fn same_id(a: &ThreadId, b: &ThreadId) -> (r: bool)
    ensures
        r == (a@ == b@),
{
    broadcast use ThreadId::axiom_string_storage_obeys_str_model;

    a.as_str() == b.as_str()
}

/// The borrowed form (`str` storage) is specified too, and mixes with the
/// owned form: both views land in `Seq<char>`.
pub fn same_id_mixed(a: &ThreadId, b: &ThreadId<str>) -> (r: bool)
    ensures
        r == (a@ == b@),
{
    broadcast use ThreadId::axiom_string_storage_obeys_str_model;
    broadcast use ThreadId::axiom_str_storage_obeys_str_model;

    a.as_str() == b.as_str()
}

/// Each kind gets its own independent boundary.
pub fn same_commit(a: &CommitId, b: &CommitId) -> (r: bool)
    ensures
        r == (a@ == b@),
{
    broadcast use CommitId::axiom_string_storage_obeys_str_model;

    a.as_str() == b.as_str()
}

/// The consumer-extension recipe: grant the purity predicate for another
/// known-pure storage with a one-line trusted axiom, and `as_str`'s
/// specification applies to it.
broadcast axiom fn axiom_box_str_storage_obeys_str_model()
    ensures
        #[trigger] ThreadId::<Box<str>>::storage_obeys_str_model(),
;

pub fn same_id_boxed(a: &ThreadId<Box<str>>, b: &ThreadId<Box<str>>) -> (r: bool)
    ensures
        r == (a@ == b@),
{
    broadcast use axiom_box_str_storage_obeys_str_model;

    a.as_str() == b.as_str()
}

} // verus!
