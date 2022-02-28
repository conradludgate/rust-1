use crate::alloc::{handle_alloc_error, Allocator, Layout, LayoutError};
use crate::boxed::Box;
use crate::collections::TryReserveError;
use crate::collections::TryReserveErrorKind::*;
use core::mem;

pub(crate) unsafe fn from_raw_slice_parts_in<T, A: Allocator>(
    ptr: *mut T,
    len: usize,
    alloc: A,
) -> Box<[T], A> {
    unsafe {
        let raw = core::slice::from_raw_parts_mut(ptr, len);
        Box::from_raw_in(raw, alloc)
    }
}

pub(crate) fn box_into_raw_slice_parts<T, A: Allocator>(
    mut this: Box<[T], A>,
) -> (*mut T, usize, A) {
    let len = this.len();
    let ptr = this.as_mut_ptr();
    let (_, alloc) = Box::into_raw_with_allocator(this);
    (ptr, len, alloc)
}

pub(crate) trait BoxStorage {
    const MIN_NON_ZERO_CAP: usize;

    /// Ensures that the buffer contains at least enough space to hold `len +
    /// additional` elements. If it doesn't already have enough capacity, will
    /// reallocate enough space plus comfortable slack space to get amortized
    /// *O*(1) behavior. Will limit this behavior if it would needlessly cause
    /// itself to panic.
    ///
    /// If `len` exceeds `self.capacity()`, this may fail to actually allocate
    /// the requested space. This is not really unsafe, but the unsafe
    /// code *you* write that relies on the behavior of this function may break.
    ///
    /// This is ideal for implementing a bulk-push operation like `extend`.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    #[cfg(not(no_global_oom_handling))]
    #[inline]
    fn reserve(&mut self, len: usize, additional: usize) {
        // Callers expect this function to be very cheap when there is already sufficient capacity.
        // Therefore, we move all the resizing and error-handling logic from grow_amortized and
        // handle_reserve behind a call, while making sure that this function is likely to be
        // inlined as just a comparison and a call if the comparison fails.
        if self.needs_to_grow(len, additional) {
            self.do_reserve_and_handle(len, additional);
        }
    }

    /// Returns if the buffer needs to grow to fulfill the needed extra capacity.
    /// Mainly used to make inlining reserve-calls possible without inlining `grow`.
    fn needs_to_grow(&self, len: usize, additional: usize) -> bool;

    #[cold]
    fn do_reserve_and_handle(&mut self, len: usize, additional: usize) {
        handle_reserve(self.grow_amortized(len, additional));
    }

    /// A specialized version of `reserve()` used only by the hot and
    /// oft-instantiated `Vec::push()`, which does its own capacity check.
    #[cfg(not(no_global_oom_handling))]
    #[inline(never)]
    fn reserve_for_push(&mut self, len: usize) {
        handle_reserve(self.grow_amortized(len, 1));
    }

    /// Shrinks the buffer down to the specified capacity. If the given amount
    /// is 0, actually completely deallocates.
    ///
    /// # Panics
    ///
    /// Panics if the given amount is *larger* than the current capacity.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    #[cfg(not(no_global_oom_handling))]
    fn shrink_to_fit(&mut self, cap: usize) {
        handle_reserve(self.shrink(cap));
    }

    /// The same as `reserve`, but returns on errors instead of panicking or aborting.
    fn try_reserve(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        self.grow_amortized(len, additional)
    }

    /// Ensures that the buffer contains at least enough space to hold `len +
    /// additional` elements. If it doesn't already, will reallocate the
    /// minimum possible amount of memory necessary. Generally this will be
    /// exactly the amount of memory necessary, but in principle the allocator
    /// is free to give back more than we asked for.
    ///
    /// If `len` exceeds `self.capacity()`, this may fail to actually allocate
    /// the requested space. This is not really unsafe, but the unsafe code
    /// *you* write that relies on the behavior of this function may break.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    #[cfg(not(no_global_oom_handling))]
    fn reserve_exact(&mut self, len: usize, additional: usize) {
        handle_reserve(self.try_reserve_exact(len, additional));
    }

    /// The same as `reserve_exact`, but returns on errors instead of panicking or aborting.
    fn try_reserve_exact(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        self.grow_exact(len, additional)
    }

    fn grow_amortized(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError>;
    fn grow_exact(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError>;
    fn shrink(&mut self, cap: usize) -> Result<(), TryReserveError>;
}

impl<T, A: Allocator> BoxStorage for Box<[mem::MaybeUninit<T>], A> {
    const MIN_NON_ZERO_CAP: usize = if mem::size_of::<T>() == 1 {
        8
    } else if mem::size_of::<T>() <= 1024 {
        4
    } else {
        1
    };

    /// Returns if the buffer needs to grow to fulfill the needed extra capacity.
    /// Mainly used to make inlining reserve-calls possible without inlining `grow`.
    fn needs_to_grow(&self, len: usize, additional: usize) -> bool {
        additional > self.len().wrapping_sub(len)
    }

    // This method is usually instantiated many times. So we want it to be as
    // small as possible, to improve compile times. But we also want as much of
    // its contents to be statically computable as possible, to make the
    // generated code run faster. Therefore, this method is carefully written
    // so that all of the code that depends on `T` is within it, while as much
    // of the code that doesn't depend on `T` as possible is in functions that
    // are non-generic over `T`.
    fn grow_amortized(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        // This is ensured by the calling contexts.
        debug_assert!(additional > 0);

        if mem::size_of::<T>() == 0 {
            // Since we return a capacity of `usize::MAX` when `elem_size` is
            // 0, getting to here necessarily means the `RawVec` is overfull.
            return Err(CapacityOverflow.into());
        }

        // Nothing we can really do about these checks, sadly.
        let required_cap = len.checked_add(additional).ok_or(CapacityOverflow)?;

        // This guarantees exponential growth. The doubling cannot overflow
        // because `cap <= isize::MAX` and the type of `cap` is `usize`.
        let cap = self.len();
        let cap = core::cmp::max(cap * 2, required_cap);
        let cap = core::cmp::max(Self::MIN_NON_ZERO_CAP, cap);

        let new_layout = Layout::array::<T>(cap);

        replace(self, |ptr, len, alloc| {
            Ok((finish_grow(new_layout, slice_layout(ptr, len), alloc)?.as_mut_ptr().cast(), cap))
        })
    }

    // The constraints on this method are much the same as those on
    // `grow_amortized`, but this method is usually instantiated less often so
    // it's less critical.
    fn grow_exact(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        if mem::size_of::<T>() == 0 {
            // Since we return a capacity of `usize::MAX` when the type size is
            // 0, getting to here necessarily means the `RawVec` is overfull.
            return Err(CapacityOverflow.into());
        }

        let cap = len.checked_add(additional).ok_or(CapacityOverflow)?;
        let new_layout = Layout::array::<T>(cap);

        replace(self, |ptr, len, alloc| {
            Ok((finish_grow(new_layout, slice_layout(ptr, len), alloc)?.as_mut_ptr().cast(), cap))
        })
    }

    fn shrink(&mut self, cap: usize) -> Result<(), TryReserveError> {
        assert!(cap <= self.len(), "Tried to shrink to a larger capacity");
        replace(self, |ptr, len, alloc| {
            let (ptr, layout) =
                if let Some(mem) = slice_layout(ptr, len) { mem } else { return Ok((ptr, len)) };

            let ptr = unsafe {
                // `Layout::array` cannot overflow here because it would have
                // overflowed earlier when capacity was larger.
                let new_layout = Layout::array::<T>(cap).unwrap_unchecked();
                alloc
                    .shrink(ptr, layout, new_layout)
                    .map_err(|_| AllocError { layout: new_layout, non_exhaustive: () })?
            };
            Ok((ptr.as_mut_ptr().cast(), cap))
        })
    }
}

fn replace<T, A: Allocator>(
    dst: &mut Box<[mem::MaybeUninit<T>], A>,
    f: impl FnOnce(*mut T, usize, &mut A) -> Result<(*mut T, usize), TryReserveError>,
) -> Result<(), TryReserveError> {
    unsafe {
        let this = core::ptr::read(dst);
        let (ptr, len, mut alloc) = box_into_raw_slice_parts(this);
        match f(ptr.cast(), len, &mut alloc) {
            Ok((ptr, len)) => {
                Ok(core::ptr::write(dst, from_raw_slice_parts_in(ptr.cast(), len, alloc)))
            }
            Err(err) => {
                core::ptr::write(dst, from_raw_slice_parts_in(ptr, len, alloc));
                Err(err)
            }
        }
    }
}

fn slice_layout<T>(ptr: *mut T, len: usize) -> Option<(core::ptr::NonNull<u8>, Layout)> {
    if mem::size_of::<T>() == 0 || len == 0 {
        None
    } else {
        // We have an allocated chunk of memory, so we can bypass runtime
        // checks to get our current layout.
        unsafe {
            let layout = Layout::array::<T>(len).unwrap_unchecked();
            Some((core::ptr::NonNull::new_unchecked(ptr.cast()), layout))
        }
    }
}

// This function is outside `Box` to minimize compile times. See the comment
// above `RawVec::grow_amortized` for details. (The `A` parameter isn't
// significant, because the number of different `A` types seen in practice is
// much smaller than the number of `T` types.)
#[inline(never)]
fn finish_grow<A>(
    new_layout: Result<Layout, LayoutError>,
    current_memory: Option<(core::ptr::NonNull<u8>, Layout)>,
    alloc: &mut A,
) -> Result<core::ptr::NonNull<[u8]>, TryReserveError>
where
    A: Allocator,
{
    // Check for the error here to minimize the size of `RawVec::grow_*`.
    let new_layout = new_layout.map_err(|_| CapacityOverflow)?;

    alloc_guard(new_layout.size())?;

    let memory = if let Some((ptr, old_layout)) = current_memory {
        debug_assert_eq!(old_layout.align(), new_layout.align());
        unsafe {
            // The allocator checks for alignment equality
            core::intrinsics::assume(old_layout.align() == new_layout.align());
            alloc.grow(ptr, old_layout, new_layout)
        }
    } else {
        alloc.allocate(new_layout)
    };

    memory.map_err(|_| AllocError { layout: new_layout, non_exhaustive: () }.into())
}

// Central function for reserve error handling.
#[cfg(not(no_global_oom_handling))]
#[inline]
fn handle_reserve(result: Result<(), TryReserveError>) {
    match result.map_err(|e| e.kind()) {
        Err(CapacityOverflow) => capacity_overflow(),
        Err(AllocError { layout, .. }) => handle_alloc_error(layout),
        Ok(()) => { /* yay */ }
    }
}

// We need to guarantee the following:
// * We don't ever allocate `> isize::MAX` byte-size objects.
// * We don't overflow `usize::MAX` and actually allocate too little.
//
// On 64-bit we just need to check for overflow since trying to allocate
// `> isize::MAX` bytes will surely fail. On 32-bit and 16-bit we need to add
// an extra guard for this in case we're running on a platform which can use
// all 4GB in user-space, e.g., PAE or x32.

#[inline]
pub(crate) fn alloc_guard(alloc_size: usize) -> Result<(), TryReserveError> {
    if usize::BITS < 64 && alloc_size > isize::MAX as usize {
        Err(CapacityOverflow.into())
    } else {
        Ok(())
    }
}

// One central function responsible for reporting capacity overflows. This'll
// ensure that the code generation related to these panics is minimal as there's
// only one location which panics rather than a bunch throughout the module.
#[cfg(not(no_global_oom_handling))]
pub(crate) fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}
