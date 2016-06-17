// immutable_arena: a Rust crate for arenas of immutable-once-built objects
// with references to other objects in the arena.
//
// Copyright (c) Chris Fallin <cfallin@c1f.net>. Released under the MIT
// license.

//! `immutable_arena` provides a type `Arena<T>` that are immutable once
//! allocated, and a smart pointer type `Ref<'arena, T>` that may be set exactly
//! once, allowing the user to create cycles among objects in the arena.
//!
//! Example usage:
//!
//! ```
//! use immutable_arena::{Arena, Ref};
//!
//! struct S<'arena> {
//!     id: u32,
//!     next: Ref<'arena, S<'arena>>,
//! }
//!
//! fn alloc_cycle<'arena>(arena: &'arena Arena<S<'arena>>)
//!     -> &'arena S<'arena> {
//!     let s1 = arena.alloc(S { id: 1, next: Ref::empty() });
//!     let s2 = arena.alloc(S { id: 2, next: Ref::empty() });
//!     s1.next.set(s2);
//!     s2.next.set(s1);
//!     s1
//! }
//!
//! fn test_cycle() {
//!     let arena = Arena::new();
//!     let s1 = alloc_cycle(&arena);
//!     assert!(s1.next.next.id == s1.id);
//! }
//! ```

extern crate typed_arena;

use std::fmt;
use std::mem;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicPtr, Ordering};

/// An `Arena<T>` is a container of objects of type `T` that, once allocated,
/// live as long as the containing arena. Within the arena, objects may refer
/// to other objects using the `Ref<'arena, T>` smart-pointer type. These
/// object references are allowed to form cycles. Once created, an object is
/// immutable. However, any `Ref<'arena, T>` instances within the object may be
/// set *exactly once*. The common usage pattern is to create objects and set
/// all their refs before returning them to user code; the objects are
/// subsequently completely immutable.
pub struct Arena<T> {
    arena: typed_arena::Arena<T>,
}

impl<T> Arena<T> {
    /// Create a new immutable-object arena.
    pub fn new() -> Arena<T> {
        Arena { arena: typed_arena::Arena::new() }
    }

    /// Allocate a new immutable object on the arena.
    pub fn alloc<'arena>(&'arena self, t: T) -> &'arena T where T: 'arena {
        self.arena.alloc(t)
    }
}

/// A `Ref<'arena, T>` is a smart pointer type that may be used within an
/// arena-allocated type to hold a reference to another object within that arena.
/// It may be set exactly once, and is immutable thereafter. It dereferences only
/// to a read-only borrow, never a mutable one.
pub struct Ref<'arena, T> {
    ptr: AtomicPtr<T>,
    _lifetime: PhantomData<&'arena ()>,
}

impl<'arena, T> Ref<'arena, T>
    where T: 'arena
{
    /// Create a new empty `Ref`. Dereferencing this reference before it is set
    /// will panic. The reference may be set exactly once.
    pub fn empty() -> Ref<'arena, T> {
        Ref {
            ptr: AtomicPtr::new(0 as *mut T),
            _lifetime: PhantomData,
        }
    }

    /// Set the `Ref`. This may be done only once.
    pub fn set(&'arena self, to: &'arena T) {
        let ptr = to as *const T as *mut T;
        assert!(!ptr.is_null());
        if self.ptr.compare_and_swap(0 as *mut T, ptr, Ordering::Relaxed) != 0 as *mut T {
            panic!("Attempt to re-set a Ref that has already been set.");
        }
    }
}

impl<'arena, T> Deref for Ref<'arena, T>
    where T: 'arena
{
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { mem::transmute(self.ptr.load(Ordering::Relaxed)) }
    }
}

impl<'arena, T> Clone for Ref<'arena, T>
    where T: 'arena
{
    fn clone(&self) -> Ref<'arena, T> {
        Ref {
            ptr: AtomicPtr::new(self.ptr.load(Ordering::Relaxed)),
            _lifetime: PhantomData,
        }
    }
}

impl<'arena, T> fmt::Debug for Ref<'arena, T>
    where T: 'arena + fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.deref().fmt(f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct BasicTest<'arena> {
        id: u32,
        a: Ref<'arena, BasicTest<'arena>>,
        b: Ref<'arena, BasicTest<'arena>>,
    }

    #[test]
    fn basic_test() {
        let arena = Arena::new();

        let x = arena.alloc(BasicTest {
            id: 0,
            a: Ref::empty(),
            b: Ref::empty(),
        });
        let y = arena.alloc(BasicTest {
            id: 1,
            a: Ref::empty(),
            b: Ref::empty(),
        });
        let z = arena.alloc(BasicTest {
            id: 2,
            a: Ref::empty(),
            b: Ref::empty(),
        });
        x.a.set(y);
        x.b.set(z);
        y.a.set(x);
        y.b.set(z);
        z.a.set(x);
        z.b.set(y);

        assert!(x.a.id == 1);
        assert!(x.b.id == 2);
        assert!(y.a.id == 0);
        assert!(y.b.id == 2);
        assert!(z.a.id == 0);
        assert!(z.b.id == 1);
    }
}
