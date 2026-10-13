//! A vector with inline, fixed capacity, for per-frame lists whose bound
//! the game fixes (four fighters, sixteen items): no heap allocation in
//! per-frame paths (`docs/memory.md`, RE-484).

use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};

/// Up to `N` values of `T`, stored inline.
pub struct ArrayVec<T, const N: usize> {
    buf: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> ArrayVec<T, N> {
    pub const fn new() -> Self {
        Self {
            buf: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    /// Appends `v`, or hands it back when the vector is full.
    pub fn try_push(&mut self, v: T) -> Result<(), T> {
        if self.len == N {
            return Err(v);
        }
        self.buf[self.len].write(v);
        self.len += 1;
        Ok(())
    }

    /// Appends `v`.
    ///
    /// # Panics
    ///
    /// When the vector is full: its bound is the game's, so an overflow is
    /// a bug.
    pub fn push(&mut self, v: T) {
        if self.try_push(v).is_err() {
            panic!("ArrayVec capacity {N} exceeded");
        }
    }

    pub fn clear(&mut self) {
        let len = self.len;
        self.len = 0;
        // SAFETY: the first `len` slots were initialised and are dropped
        // once; `len` is already 0 if a drop panics.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.buf.as_mut_ptr().cast::<T>(),
                len,
            ));
        }
    }
}

impl<T, const N: usize> Default for ArrayVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for ArrayVec<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> Deref for ArrayVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        // SAFETY: the first `len` slots are initialised.
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr().cast::<T>(), self.len) }
    }
}

impl<T, const N: usize> DerefMut for ArrayVec<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        // SAFETY: the first `len` slots are initialised.
        unsafe { core::slice::from_raw_parts_mut(self.buf.as_mut_ptr().cast::<T>(), self.len) }
    }
}

impl<T, const N: usize> FromIterator<T> for ArrayVec<T, N> {
    /// # Panics
    ///
    /// When the iterator yields more than `N` values.
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut v = Self::new();
        v.extend(iter);
        v
    }
}

impl<T, const N: usize> Extend<T> for ArrayVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for x in iter {
            self.push(x);
        }
    }
}

impl<T: core::fmt::Debug, const N: usize> core::fmt::Debug for ArrayVec<T, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;

    #[test]
    fn pushes_to_capacity_and_drops_each_value_once() {
        let rc = Rc::new(());
        {
            let mut v: ArrayVec<Rc<()>, 3> = ArrayVec::new();
            for _ in 0..3 {
                v.push(rc.clone());
            }
            assert!(v.try_push(rc.clone()).is_err());
            assert_eq!(v.len(), 3);
            assert_eq!(Rc::strong_count(&rc), 4);
            v.clear();
            assert_eq!(Rc::strong_count(&rc), 1);
            v.push(rc.clone());
        }
        assert_eq!(Rc::strong_count(&rc), 1);
    }

    #[test]
    fn collects_and_derefs_to_a_slice() {
        let mut v: ArrayVec<u32, 4> = (1..=4).collect();
        v[0] = 9;
        assert_eq!(&*v, &[9, 2, 3, 4]);
    }

    #[test]
    #[should_panic(expected = "capacity 2 exceeded")]
    fn overflow_panics() {
        let _: ArrayVec<u8, 2> = (0..3).collect();
    }
}
