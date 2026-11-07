//! Helpers for parallel loops that write disjoint elements of one buffer.

use std::marker::PhantomData;

/// A slice that many threads may write, each to elements no other thread touches. The caller guarantees
/// disjointness; reads of an element written by another thread in the same pass are not allowed.
pub struct Disjoint<'a, T> {
    ptr: *mut T,
    len: usize,
    _life: PhantomData<&'a mut [T]>,
}

unsafe impl<T: Send> Send for Disjoint<'_, T> {}
unsafe impl<T: Send> Sync for Disjoint<'_, T> {}

impl<'a, T> Disjoint<'a, T> {
    pub fn new(s: &'a mut [T]) -> Self {
        Self { ptr: s.as_mut_ptr(), len: s.len(), _life: PhantomData }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// # Safety
    /// No other thread reads or writes element i during this pass.
    #[inline]
    pub unsafe fn write(&self, i: usize, v: T) {
        assert!(i < self.len);
        unsafe { *self.ptr.add(i) = v };
    }
    /// # Safety
    /// No other thread writes element i during this pass.
    #[inline]
    pub unsafe fn read(&self, i: usize) -> &T {
        assert!(i < self.len);
        unsafe { &*self.ptr.add(i) }
    }
}

/// Exclusive prefix sum in place; returns the total.
pub fn exclusive_scan(v: &mut [u32]) -> u32 {
    let mut sum = 0u32;
    for x in v.iter_mut() {
        let n = *x;
        *x = sum;
        sum += n;
    }
    sum
}
