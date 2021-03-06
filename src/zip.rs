// This file is part of faster, the SIMD library for humans.
// Copyright 2017 Adam Niederer <adam.niederer@gmail.com>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::iters::{SIMDIterator, SIMDIterable, SIMDObject, UnsafeIterator, SIMDSized};
use crate::vecs::{Packed, Packable};

/// A macro which takes a number n and an expression, and returns a tuple
/// containing n copies of the expression. Only works for numbers less than or
/// equal to 12.
///
/// ```
/// #[macro_use] extern crate faster;
/// use faster::*;
///
/// # fn main() {
/// assert_eq!(tuplify!(2, 1), (1, 1));
/// assert_eq!(tuplify!(5, "hi"), ("hi", "hi", "hi", "hi", "hi"));
/// assert_eq!(tuplify!(3, i8s::splat(0)), (i8s::splat(0), i8s::splat(0), i8s::splat(0)));
/// # }
/// ```
#[macro_export] macro_rules! tuplify {
    (1, $i:expr) => { ($i) };
    (2, $i:expr) => { ($i, $i) };
    (3, $i:expr) => { ($i, $i, $i) };
    (4, $i:expr) => { ($i, $i, $i, $i) };
    (5, $i:expr) => { ($i, $i, $i, $i, $i) };
    (6, $i:expr) => { ($i, $i, $i, $i, $i, $i) };
    (7, $i:expr) => { ($i, $i, $i, $i, $i, $i, $i) };
    (8, $i:expr) => { ($i, $i, $i, $i, $i, $i, $i, $i) };
    (9, $i:expr) => { ($i, $i, $i, $i, $i, $i, $i, $i, $i) };
    (10, $i:expr) => { ($i, $i, $i, $i, $i, $i, $i, $i, $i, $i) };
    (11, $i:expr) => { ($i, $i, $i, $i, $i, $i, $i, $i, $i, $i, $i) };
    (12, $i:expr) => { ($i, $i, $i, $i, $i, $i, $i, $i, $i, $i, $i, $i) };
}

/// A lazy iterator which returns tuples of the elements of its contained
/// iterators.
pub struct Zip<T> {
    iters: T
}

/// A lazy mapping iterator which applies its function to a stream of tuples of
/// vectors.
pub struct SIMDZipMap<I, F> where I : SIMDZippedIterator {
    iter: I,
    func: F,
}

/// A trait which can transform a collection of iterators into a `Zip`
pub trait IntoSIMDZip : Sized {
    /// Return an iterator which may iterate over `self` in lockstep.
    fn zip(self) -> Zip<Self>;
}

pub trait SIMDZippedObject : Sized {
    type Scalars;
    type Vectors;

    /// Return the vector length of this object.
    fn width(&self) -> usize;

    /// Return the scalar length of this object.
    fn size(&self) -> usize;
}

/// An iterator which automatically packs the values it iterates over into SIMD
/// vectors.
pub trait SIMDZippedIterable : SIMDZippedObject + ExactSizeIterator<Item = <Self as SIMDZippedObject>::Vectors> {
    /// Return the current position of this iterator, measured in scalars
    fn scalar_pos(&self) -> usize;

    /// Return the current position of this iterator, measured in vectors.
    #[inline(always)]
    fn vector_pos(&self) -> usize {
        self.scalar_pos() / self.width()
    }

    /// Return the length of this iterator, measured in scalars.
    #[inline(always)]
    fn scalar_len(&self) -> usize {
        <Self as ExactSizeIterator>::len(self)
    }

    /// Return the length of this iterator, measured in vectors.
    #[inline(always)]
    fn vector_len(&self) -> usize {
        self.scalar_len() / self.width()
    }

    /// Advance the iterable by `amount` scalars.
    fn advance(&mut self, amount: usize);

    /// Advance the iterable such that it procudes no more items.
    #[inline(always)]
    fn finalize(&mut self) {
        let end = self.scalar_len() - self.scalar_pos();
        self.advance(end);
    }

    /// Return the default vector for this iterable.
    fn default(&self) -> Self::Vectors;

    // #[inline(always)]
    // /// Create a an iterator over the remaining scalar elements in this iterator
    // fn unpack(self) -> Unpacked<Self> {
    //     Unpacked {
    //         iter: self,
    //     }
    // }

    // #[inline(always)]
    // /// Create an iterator which returns `amt` vectors at a time.
    // fn unroll<'a>(&'a mut self, amt: usize) -> Unrolled<'a, Self> {
    //     assert!(amt <= 8);
    //     Unrolled {
    //         iter: self,
    //         amt: amt,
    //         scratch: [<Self as SIMDZippedObject>::Vectors::default(); 8]
    //     }
    // }
}

/// An iterator which automatically packs the values it iterates over into SIMD
/// vectors, and can handle collections which do not fit into the system's
/// vectors natively.
pub trait SIMDZippedIterator : SIMDZippedIterable {
    /// Pack and return a partially full vector containing up to the next
    /// `self.width()` of the iterator, or None if no elements are left.
    /// Elements which are not filled are instead initialized to default.
    fn end(&mut self) -> Option<(Self::Vectors, usize)>;

    /// Return an iterator which calls `func` on vectors of elements.
    #[inline(always)]
    fn simd_map<A, B, F>(self, func: F) -> SIMDZipMap<Self, F>
        where F : FnMut(Self::Vectors) -> A, A : Packed<Scalar = B>, B : Packable {
        SIMDZipMap {
            iter: self,
            func: func,
        }
    }

    /// Pack and run `func` over the iterator, returning no value and not
    /// modifying the iterator.
    #[inline(always)]
    fn simd_do_each<F>(&mut self, mut func: F)
        where F : FnMut(Self::Vectors) -> () {
        while let Some(v) = self.next() {
            func(v);
        }
        if let Some((v, _)) = self.end() {
            func(v);
        }
    }

    /// Return a vector generated by reducing `func` over accumulator `start`
    /// and the values of this iterator, initializing all vectors to `default`
    /// before populating them with elements of the iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate faster;
    /// use faster::*;
    ///
    /// # fn main() {
    /// let reduced = (&[2.0f32; 100][..]).simd_iter(f32s(0.0))
    ///     .simd_reduce(f32s(0.0), |acc, v| acc + v);
    /// # }
    /// ```
    ///
    /// In this example, on a machine with 4-element vectors, the argument to
    /// the last call of the closure is
    ///
    /// ```rust,ignore
    /// [ 2.0 | 2.0 | 2.0 | 2.0 ]
    /// ```
    ///
    /// and the result of the reduction is
    ///
    /// ```rust,ignore
    /// [ 50.0 | 50.0 | 50.0 | 50.0 ]
    /// ```
    ///
    /// whereas on a machine with 8-element vectors, the last call is passed
    ///
    /// ```rust,ignore
    /// [ 2.0 | 2.0 | 2.0 | 2.0 | 0.0 | 0.0 | 0.0 | 0.0 ]
    /// ```
    ///
    /// and the result of the reduction is
    ///
    /// ```rust,ignore
    /// [ 26.0 | 26.0 | 26.0 | 26.0 | 24.0 | 24.0 | 24.0 | 24.0 ]
    /// ```
    ///
    /// # Footgun Warning
    ///
    /// The results of `simd_reduce` are not portable, and it is your
    /// responsibility to interpret the result in such a way that the it is
    /// consistent across different architectures. See [`Packed::sum`] and
    /// [`Packed::product`] for built-in functions which may be helpful.
    ///
    /// [`Packed::sum`]: vecs/trait.Packed.html#tymethod.sum
    /// [`Packed::product`]: vecs/trait.Packed.html#tymethod.product
    #[inline(always)]
    fn simd_reduce<A, F>(&mut self, mut start: A, mut func: F) -> A
        where F : FnMut(A, Self::Vectors) -> A {

        while let Some(v) = self.next() {
            start = func(start, v);
        }
        if let Some((v, _)) = self.end() {
            start = func(start, v);
        }
        start
    }
}

macro_rules! impl_iter_zip {
    (($($a:tt),*), ($($b:tt),*), ($($n:tt),*)) => (
        impl<$($a),*> IntoSIMDZip for ($($a),*) where $($a : SIMDIterator + UnsafeIterator),* {
            #[inline(always)]
            fn zip(self) -> Zip<Self> {
                if $(self.0.len() != self.$n.len())||* {
                    panic!("You can only zip iterators of the same length.");
                }
                Zip { iters: self }
            }
        }

        impl<$($a),*> ExactSizeIterator for Zip<($($a),*)>
            where $($a : SIMDIterator + UnsafeIterator),* {
            #[inline(always)]
            fn len(&self) -> usize {
                self.iters.0.len()
            }
        }

        impl<$($a),*> Iterator for Zip<($($a),*)>
            where $($a : SIMDIterator + UnsafeIterator),* {
            type Item = ($(<$a as Iterator>::Item),*);

            #[inline(always)]
            fn next(&mut self) -> Option<<Self as SIMDZippedObject>::Vectors> {
                let pos = self.iters.0.scalar_pos();
                self.iters.0.next().map(|v| unsafe {
                    (v, $(self.iters.$n.next_unchecked(pos)),*)
                })
            }
        }

        impl<$($a),*> SIMDZippedObject for Zip<($($a),*)>
            where $($a : SIMDIterator + UnsafeIterator),* {
            type Vectors = ($($a::Vector),*);
            type Scalars = ($($a::Scalar),*);

            #[inline(always)]
            fn width(&self) -> usize {
                self.iters.0.width()
            }

            #[inline(always)]
            fn size(&self) -> usize {
                self.iters.0.size()
            }
        }

        impl<$($a),*> SIMDZippedIterator for Zip<($($a),*)>
            where $($a : SIMDIterator + UnsafeIterator),* {

            #[inline(always)]
            fn end(&mut self) -> Option<(Self::Vectors, usize)> {
                let pos = self.iters.0.scalar_pos();
                self.iters.0.end().map(|(v, n)| unsafe {
                    ((v, $(self.iters.$n.end_unchecked(pos, n)),*), n)
                })
            }
        }

        impl<$($a),*> SIMDZippedIterable for Zip<($($a),*)>
            where $($a : SIMDIterator + UnsafeIterator),* {

            #[inline(always)]
            fn scalar_pos(&self) -> usize {
                self.iters.0.scalar_pos()
            }

            #[inline(always)]
            fn advance(&mut self, amount: usize) {
                self.iters.0.advance(amount);
            }

            #[inline(always)]
            fn default(&self) -> Self::Vectors {
                (self.iters.0.default(), $(self.iters.$n.default()),*)
            }
        }
    );
}

impl<I, F, A> Iterator for SIMDZipMap<I, F>
    where I : SIMDZippedIterator, F : FnMut(I::Vectors) -> A, A : Packed {
    type Item = A;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(&mut self.func)
    }
}

impl<I, F, A> ExactSizeIterator for SIMDZipMap<I, F>
    where I : SIMDZippedIterator, F : FnMut(I::Vectors) -> A, A : Packed {
    #[inline(always)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<I, F, A> SIMDObject for SIMDZipMap<I, F>
    where I : SIMDZippedIterator, F : FnMut(I::Vectors) -> A, A : Packed {
    type Vector = A;
    type Scalar = A::Scalar;
}

impl<I, F, A> SIMDSized for SIMDZipMap<I, F>
    where I : SIMDZippedIterator, F : FnMut(I::Vectors) -> A, A : Packed {
    /// Return the length of this iterator, measured in scalars.
    #[inline(always)]
    fn scalar_len(&self) -> usize {
        self.iter.scalar_len()
    }

    /// Return the length of this iterator, measured in vectors.
    #[inline(always)]
    fn vector_len(&self) -> usize {
        self.iter.vector_len()
    }
}

impl<I, F, A> SIMDIterable for SIMDZipMap<I, F>
    where I : SIMDZippedIterator, F : FnMut(I::Vectors) -> A, A : Packed {
    #[inline(always)]
    fn scalar_pos(&self) -> usize {
        self.iter.scalar_pos()
    }

    #[inline(always)]
    fn advance(&mut self, amount: usize) {
        self.iter.advance(amount)
    }

    #[inline(always)]
    fn default(&self) -> Self::Vector {
        // TODO: Is there a more sane return value (without invoking the closure)?
        <Self::Vector as Packed>::default()
    }
}

impl<I, F, A> SIMDIterator for SIMDZipMap<I, F>
where I : SIMDZippedIterator, F : FnMut(I::Vectors) -> A, A : Packed {
    #[inline(always)]
    fn end(&mut self) -> Option<(Self::Vector, usize)> {
        let (v, n) = self.iter.end()?;
        let nr = n * self.iter.size() / self.size();
        Some(((self.func)(v), nr))
    }
}

impl_iter_zip!((A, B),
               (AA, BB),
               (1));
impl_iter_zip!((A, B, C),
               (AA, BB, CC),
               (1, 2));
impl_iter_zip!((A, B, C, D),
               (AA, BB, CC, DD),
               (1, 2, 3));
impl_iter_zip!((A, B, C, D, E),
               (AA, BB, CC, DD, EE),
               (1, 2, 3, 4));
impl_iter_zip!((A, B, C, D, E, F),
               (AA, BB, CC, DD, EE, FF),
               (1, 2, 3, 4, 5));
impl_iter_zip!((A, B, C, D, E, F, G),
               (AA, BB, CC, DD, EE, FF, GG),
               (1, 2, 3, 4, 5, 6));
impl_iter_zip!((A, B, C, D, E, F, G, H),
               (AA, BB, CC, DD, EE, FF, GG, HH),
               (1, 2, 3, 4, 5, 6, 7));
impl_iter_zip!((A, B, C, D, E, F, G, H, I),
               (AA, BB, CC, DD, EE, FF, GG, HH, II),
               (1, 2, 3, 4, 5, 6, 7, 8));
impl_iter_zip!((A, B, C, D, E, F, G, H, I, J),
               (AA, BB, CC, DD, EE, FF, GG, HH, II, JJ),
               (1, 2, 3, 4, 5, 6, 7, 8, 9));
impl_iter_zip!((A, B, C, D, E, F, G, H, I, J, K),
               (AA, BB, CC, DD, EE, FF, GG, HH, II, JJ, KK),
               (1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
impl_iter_zip!((A, B, C, D, E, F, G, H, I, J, K, L),
               (AA, BB, CC, DD, EE, FF, GG, HH, II, JJ, KK, LL),
               (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11));
impl_iter_zip!((A, B, C, D, E, F, G, H, I, J, K, L, M),
               (AA, BB, CC, DD, EE, FF, GG, HH, II, JJ, KK, LL, MM),
               (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12));
