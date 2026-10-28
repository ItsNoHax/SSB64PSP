//! [`InlineOption`]: an `Option<T>` whose payload is built where it lives.

use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;

/// An `Option<T>` whose `Some` payload can be written in place
/// ([`Self::write_with`]).
///
/// `Option<T>`'s layout is the compiler's (a niche may stand in for the
/// tag), so its payload cannot be initialised through a pointer before
/// the value is `Some`; built by value, a large `T` lands on the stack
/// and is copied, because `panic = unwind` keeps LLVM from constructing
/// it in its heap or arena slot (RE-469). Here the tag is a plain flag
/// beside the payload.
pub struct InlineOption<T> {
    some: bool,
    value: MaybeUninit<T>,
}

impl<T> InlineOption<T> {
    /// `None`.
    pub const fn none() -> Self {
        InlineOption {
            some: false,
            value: MaybeUninit::uninit(),
        }
    }

    pub fn is_some(&self) -> bool {
        self.some
    }

    pub fn as_ref(&self) -> Option<&T> {
        // SAFETY: `some` is set only once `value` is initialised.
        self.some.then(|| unsafe { self.value.assume_init_ref() })
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        if self.some {
            // SAFETY: as in `as_ref`.
            Some(unsafe { self.value.assume_init_mut() })
        } else {
            None
        }
    }

    /// `*self = None`: the payload, if any, is dropped.
    pub fn clear(&mut self) {
        if core::mem::take(&mut self.some) {
            // SAFETY: the payload was initialised, and `some` is already
            // false, so a panicking drop cannot drop it twice.
            unsafe { self.value.assume_init_drop() }
        }
    }

    /// Writes `None` into `dst`, then lets `init` build the payload in
    /// place; the value is `Some` if `init` returns `true`. Returns that
    /// flag.
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned; its old contents are
    /// overwritten without being dropped. `init` gets the payload's
    /// uninitialised slot: returning `true`, it must have initialised it
    /// whole; returning `false` (or panicking), it must leave nothing in
    /// it that needs dropping.
    pub unsafe fn write_with(dst: *mut Self, init: impl FnOnce(*mut T) -> bool) -> bool {
        // SAFETY: `dst` is valid and aligned (the caller's contract); the
        // flag is false until `init` reports a whole payload.
        unsafe {
            addr_of_mut!((*dst).some).write(false);
            let ok = init(addr_of_mut!((*dst).value).cast::<T>());
            addr_of_mut!((*dst).some).write(ok);
            ok
        }
    }
}

impl<T> Default for InlineOption<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T> Drop for InlineOption<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::InlineOption;
    use alloc::rc::Rc;
    use core::mem::MaybeUninit;

    #[test]
    fn write_with_true_is_some_false_is_none() {
        let mut slot = MaybeUninit::<InlineOption<[u32; 4]>>::uninit();
        // SAFETY: a local slot; `init` writes the whole array.
        let ok = unsafe {
            InlineOption::write_with(slot.as_mut_ptr(), |p| {
                p.write([1, 2, 3, 4]);
                true
            })
        };
        // SAFETY: `write_with` initialised the slot.
        let mut o = unsafe { slot.assume_init() };
        assert!(ok && o.is_some());
        assert_eq!(o.as_ref(), Some(&[1, 2, 3, 4]));
        o.as_mut().unwrap()[0] = 9;
        assert_eq!(o.as_ref().unwrap()[0], 9);
        o.clear();
        assert!(o.as_ref().is_none() && o.as_mut().is_none());

        let mut slot = MaybeUninit::<InlineOption<[u32; 4]>>::uninit();
        // SAFETY: `init` writes nothing and says so.
        let ok = unsafe { InlineOption::write_with(slot.as_mut_ptr(), |_| false) };
        // SAFETY: as above.
        let o = unsafe { slot.assume_init() };
        assert!(!ok && !o.is_some());
    }

    #[test]
    fn payload_dropped_once_on_clear_and_on_drop() {
        let rc = Rc::new(());
        let mut o = InlineOption::<Rc<()>>::none();
        // SAFETY: `o` is a valid `InlineOption` holding nothing to drop.
        unsafe {
            InlineOption::write_with(&mut o, |p| {
                p.write(rc.clone());
                true
            });
        }
        assert_eq!(Rc::strong_count(&rc), 2);
        o.clear();
        o.clear();
        assert_eq!(Rc::strong_count(&rc), 1);
        let mut o = InlineOption::<Rc<()>>::none();
        // SAFETY: as above.
        unsafe {
            InlineOption::write_with(&mut o, |p| {
                p.write(rc.clone());
                true
            });
        }
        drop(o);
        assert_eq!(Rc::strong_count(&rc), 1);
        assert!(InlineOption::<Rc<()>>::default().as_ref().is_none());
    }
}
