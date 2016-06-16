// 
extern crate typed_arena;

use std::fmt;
use std::mem;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicPtr, Ordering};

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
        let mut builder = Builder { arena: self };
        build(&mut builder)
    }
}

pub struct Builder<'arena, T>
    where T: 'arena
{
    arena: &'arena Arena<T>,
}

impl<'arena, T> Builder<'arena, T> {
    pub fn new(&mut self, t: T) -> Ref<'arena, T> {
        let obj: &'arena mut T = self.arena.arena.alloc(t);
        Ref {
            ptr: AtomicPtr::new(obj as *mut T),
            _lifetime: PhantomData,
        }
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

    pub fn set(&self, pointee: &Ref<'arena, T>) {
        if self.ptr.compare_and_swap(0 as *mut T,
                                     pointee.ptr.load(Ordering::Relaxed),
                                     Ordering::Relaxed) != 0 as *mut T {
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

impl<'arena, T> DerefMut for Ref<'arena, T>
    where T: 'arena
{
    fn deref_mut(&mut self) -> &mut T {
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
            (x, y, z)
        });
        assert!(x.a.id == 1);
        assert!(x.b.id == 2);
        assert!(y.a.id == 0);
        assert!(y.b.id == 2);
        assert!(z.a.id == 0);
        assert!(z.b.id == 1);
    }
}
