`immutable_arena`: an arena for immutable-once-built objects with cyclic refs
=============================================================================

[![Build Status](https://travis-ci.org/cfallin/rust-immutable-arena.svg?branch=master)](https://travis-ci.org/cfallin/rust-immutable-arena)

[crates.io](https://crates.io/crates/immutable_arena/)

[Documentation](https://cfallin.github.io/rust-immutable-arena/immutable_arena/)

This crate implements an arena for objects that are immutable once they are
built, aside from references to other objects in the arena. The user creates
objects once at allocation time, then after objects exist, may set special
smart-pointers (`Ref` instances) to other objects on the arena *exactly once*.
Subsequently, these objects are completely immutable.

Example usage:

```
use immutable_arena::{Arena, Ref};

struct S<'arena> { id: u32, next: Ref<'arena, S<'arena>> }
fn alloc_cycle<'arena>(arena: &'arena Arena<S<'arena>>)
    -> &'arena S<'arena> {
    let s1 = arena.alloc(S { id: 1, next: Ref::empty() });
    let s2 = arena.alloc(S { id: 2, next: Ref::empty() });
    s1.next.set(s2);
    s2.next.set(s1);
    s1
}

fn test_cycle() {
    let arena = Arena::new();
    let s1 = alloc_cycle(&arena);
    assert!(s1.next.next.id == s1.id);
}
```
