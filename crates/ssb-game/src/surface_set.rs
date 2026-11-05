//! What a map query (`crate::map`) scans: the stage's surfaces in map
//! order, and per [`MapSurfaceKind`] the surfaces of that kind.
//!
//! The original walks one kind's line list (`gMPCollisionLineGroups[kind]`)
//! per test. A closure that yields the surfaces in map order
//! (`Fn() -> I`) answers a kind with every surface: its scans test each
//! surface's kind, as before. A [`SurfaceTable`] indexes the kinds once,
//! keeping each surface's map index and the map order inside a kind, so a
//! query visits the same surfaces in the same order and returns the same
//! result.

use crate::weapon::{MapSurface, MapSurfaceKind as Kind};

/// The surfaces a map query scans.
///
/// `I` is what [`Self::all`] returns: a closure's return type, so the
/// queries keep their `<I, F>` parameters and a closure keeps its
/// instantiations.
pub trait SurfaceSet<I> {
    /// [`Self::of_kind`]'s iterator.
    type OfKind<'s>: Iterator<Item = (usize, MapSurface)>
    where
        Self: 's;

    /// The set by reference, for a query that takes its set by value: a
    /// closure's is `&F`, as a closure caller passes.
    type Ref<'s>: SurfaceSet<I>
    where
        Self: 's;

    /// Every surface, in map order.
    fn all(&self) -> I;

    /// [`Self::Ref`].
    fn borrowed(&self) -> Self::Ref<'_>;

    /// Every surface of `kind` with its map index, in map order; a set may
    /// yield others too (a closure yields every surface), so a scan still
    /// tests the kind.
    fn of_kind(&self, kind: Kind) -> Self::OfKind<'_>;
}

impl<I, F> SurfaceSet<I> for F
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    type OfKind<'s>
        = core::iter::Enumerate<I::IntoIter>
    where
        Self: 's;
    type Ref<'s>
        = &'s F
    where
        Self: 's;

    #[inline(always)]
    fn all(&self) -> I {
        self()
    }

    #[inline(always)]
    fn borrowed(&self) -> &F {
        self
    }

    #[inline(always)]
    fn of_kind(&self, _kind: Kind) -> Self::OfKind<'_> {
        self().into_iter().enumerate()
    }
}

/// A stage's surfaces with each kind's map indices, built once per fighter
/// tick (`psp-runtime`'s `FighterScene::tick_fighter_physics_table`).
pub struct SurfaceTable<'a> {
    surfaces: &'a [MapSurface],
    counts: [u8; 4],
    by_kind: [[u8; SurfaceTable::CAPACITY]; 4],
}

fn kind_slot(kind: Kind) -> usize {
    match kind {
        Kind::Floor => 0,
        Kind::Ceiling => 1,
        Kind::RightWall => 2,
        Kind::LeftWall => 3,
    }
}

impl<'a> SurfaceTable<'a> {
    /// The most surfaces a table indexes (the largest stage map has 117).
    pub const CAPACITY: usize = 128;

    /// `surfaces` indexed by kind; `None` past [`Self::CAPACITY`].
    pub fn new(surfaces: &'a [MapSurface]) -> Option<Self> {
        if surfaces.len() > Self::CAPACITY {
            return None;
        }
        let mut table = SurfaceTable {
            surfaces,
            counts: [0; 4],
            by_kind: [[0; Self::CAPACITY]; 4],
        };
        for (i, s) in surfaces.iter().enumerate() {
            let k = kind_slot(s.kind);
            let n = usize::from(table.counts[k]);
            table.by_kind[k][n] = i as u8;
            table.counts[k] += 1;
        }
        Some(table)
    }

    /// Test-only: `kind`'s map indices, for the exactness tests' negative
    /// controls (a reordered or wrong entry is what a broken index gives).
    #[doc(hidden)]
    pub fn kind_indices_mut(&mut self, kind: Kind) -> &mut [u8] {
        let k = kind_slot(kind);
        &mut self.by_kind[k][..usize::from(self.counts[k])]
    }

    /// The number of surfaces of `kind`.
    pub fn count(&self, kind: Kind) -> usize {
        usize::from(self.counts[kind_slot(kind)])
    }
}

/// [`SurfaceTable`]'s surfaces of one kind, in map order.
pub struct TableKind<'s> {
    surfaces: &'s [MapSurface],
    indices: core::slice::Iter<'s, u8>,
}

impl Iterator for TableKind<'_> {
    type Item = (usize, MapSurface);

    #[inline]
    fn next(&mut self) -> Option<(usize, MapSurface)> {
        let i = usize::from(*self.indices.next()?);
        Some((i, self.surfaces[i]))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

/// A table's [`SurfaceSet::all`].
pub type TableAll<'a> = core::iter::Copied<core::slice::Iter<'a, MapSurface>>;

impl<'a> SurfaceSet<TableAll<'a>> for SurfaceTable<'a> {
    type OfKind<'s>
        = TableKind<'s>
    where
        Self: 's;
    type Ref<'s>
        = &'s SurfaceTable<'a>
    where
        Self: 's;

    #[inline]
    fn all(&self) -> TableAll<'a> {
        self.surfaces.iter().copied()
    }

    #[inline]
    fn borrowed(&self) -> &SurfaceTable<'a> {
        self
    }

    #[inline]
    fn of_kind(&self, kind: Kind) -> TableKind<'_> {
        let k = kind_slot(kind);
        TableKind {
            surfaces: self.surfaces,
            indices: self.by_kind[k][..usize::from(self.counts[k])].iter(),
        }
    }
}

/// A query handed the table by reference (`surfaces: F` taking `&table`).
impl<'t, 'a> SurfaceSet<TableAll<'a>> for &'t SurfaceTable<'a> {
    type OfKind<'s>
        = TableKind<'s>
    where
        Self: 's;
    type Ref<'s>
        = &'t SurfaceTable<'a>
    where
        Self: 's;

    #[inline]
    fn all(&self) -> TableAll<'a> {
        (**self).all()
    }

    #[inline]
    fn borrowed(&self) -> &'t SurfaceTable<'a> {
        self
    }

    #[inline]
    fn of_kind(&self, kind: Kind) -> TableKind<'_> {
        (**self).of_kind(kind)
    }
}
