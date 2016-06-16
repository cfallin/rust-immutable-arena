// immutable_arena: a Rust crate for arenas of immutable-once-built objects
// with references to other objects in the arena.
//
// Copyright (c) Chris Fallin <cfallin@c1f.net>. Released under the MIT
// license.

extern crate typed_arena;
extern crate smallvec;

use std::fmt;
use std::mem;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};
use smallvec::SmallVec;

/// An `Arena<T>` is a container of objects of type `T` that, once allocated,
/// live as long as the containing arena. Within the arena, objects may refer
/// to other objects using the `Ref<'arena, T>` smart-pointer type. These
/// object references are allowed to form cycles. The only restriction is that
/// once created, an object is immutable. Objects are created within a "builder
/// context", via the `build()` method on the `Arena<T>`. Within the builder
/// context, user code has mutable access to objects, and queue reference
/// assignments to / other objects.
///
/// *Note:* the builder interface allows safe code to obtain a simultaneous
/// mutable and immutable borrow of an instance of `T` by setting a `Ref` and
/// immediately dereferencing it while still holding the `BuilderRef`.
pub struct Arena<T> {
    arena: typed_arena::Arena<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Arena<T> {
        Arena { arena: typed_arena::Arena::new() }
    }

    pub fn build<'arena, F, R>(&'arena self, build: F) -> R
        where F: FnOnce(&mut Builder<'arena, T>) -> R
    {
        let mut builder = Builder {
            arena: self,
            assignments: Mutex::new(SmallVec::new()),
        };
        build(&mut builder)
    }
}

pub struct Builder<'arena, T>
    where T: 'arena
{
    arena: &'arena Arena<T>,
    assignments: Mutex<SmallVec<[RefAssignment<'arena, T>; 4]>>,
}

struct RefAssignment<'arena, T> {
    from: *const Ref<'arena, T>,
    to: *const T,
}

impl<'builder, 'arena, T> Builder<'arena, T>
    where 'arena: 'builder,
          T: 'arena
{
    pub fn new(&'builder mut self, t: T) -> BuilderRef<'builder, 'arena, T> {
        BuilderRef {
            ptr: self.arena.arena.alloc(t),
            builder: self,
        }
    }

    pub fn freeze(&'builder self, t: BuilderRef<'builder, 'arena, T>) -> Ref<'arena, T> {
        Ref {
            ptr: AtomicPtr::new(t.ptr as *mut T),
            _lifetime: PhantomData,
        }
    }
}

impl<'arena, T> Drop for Builder<'arena, T>
    where T: 'arena
{
    fn drop(&mut self) {
        for r in self.assignments.lock().unwrap().iter() {
            let from: &Ref<'arena, T> = unsafe { mem::transmute(r.from) };
            if from.ptr.compare_and_swap(0 as *mut T, r.to as *mut T, Ordering::Relaxed) !=
               0 as *mut T {
                panic!("Attempt to re-set a Ref that has already been set.");
            }
        }
    }
}

pub struct BuilderRef<'builder, 'arena, T>
    where 'arena: 'builder,
          T: 'arena
{
    ptr: &'arena mut T,
    builder: &'builder Builder<'arena, T>,
}

impl<'builder, 'arena, T> Deref for BuilderRef<'builder, 'arena, T>
    where 'arena: 'builder,
          T: 'arena
{
    type Target = T;
    fn deref(&self) -> &T {
        self.ptr
    }
}

impl<'builder, 'arena, T> DerefMut for BuilderRef<'builder, 'arena, T>
    where 'arena: 'builder,
          T: 'arena
{
    fn deref_mut(&mut self) -> &mut T {
        self.ptr
    }
}

impl<'builder, 'arena, T> fmt::Debug for BuilderRef<'builder, 'arena, T>
    where 'arena: 'builder,
          T: 'arena + fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.deref().fmt(f)
    }
}

pub struct Ref<'arena, T> {
    ptr: AtomicPtr<T>,
    _lifetime: PhantomData<&'arena ()>,
}

impl<'arena, T> Ref<'arena, T>
    where T: 'arena
{
    pub fn empty() -> Ref<'arena, T> {
        Ref {
            ptr: AtomicPtr::new(0 as *mut T),
            _lifetime: PhantomData,
        }
    }

    pub fn set<'builder>(&'arena self, to: &BuilderRef<'builder, 'arena, T>)
        where 'arena: 'builder
    {
        to.builder.assignments.lock().unwrap().push(RefAssignment {
            from: self as *const Ref<'arena, T>,
            to: self.ptr.load(Ordering::Relaxed) as *mut T,
        });
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
        let (x, y, z) = arena.build(|ctx| {
            let x = ctx.new(BasicTest {
                id: 0,
                a: Ref::empty(),
                b: Ref::empty(),
            });
            let y = ctx.new(BasicTest {
                id: 1,
                a: Ref::empty(),
                b: Ref::empty(),
            });
            let z = ctx.new(BasicTest {
                id: 2,
                a: Ref::empty(),
                b: Ref::empty(),
            });
            x.a.set(&y);
            x.b.set(&z);
            y.a.set(&x);
            y.b.set(&z);
            z.a.set(&x);
            z.b.set(&y);
            (ctx.freeze(x), ctx.freeze(y), ctx.freeze(z))
        });
        assert!(x.a.id == 1);
        assert!(x.b.id == 2);
        assert!(y.a.id == 0);
        assert!(y.b.id == 2);
        assert!(z.a.id == 0);
        assert!(z.b.id == 1);
    }
}
