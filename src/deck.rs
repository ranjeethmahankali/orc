use std::collections::BTreeSet;

#[derive(Default)]
pub struct Deck<T> {
    items: Vec<T>,
    depths: Vec<u8>,
    stride_offset: Vec<usize>,
    strides: Vec<usize>,
    pegs: Vec<usize>,
}

/**
Creates a `Deck` from a nested parenthesized literal.

```
let d = orc::deck!(1, 2, 3);              // depth-1: a flat list
let d = orc::deck!((1, 2), (3, 4));       // depth-2: list of lists
let d = orc::deck!(((1,2),(3,4)),((5,6),(7,8))); // depth-3
```
*/
#[macro_export]
macro_rules! deck {
    // Internal: count nesting depth along the first element's path.
    (@depth ($($first:tt)*) $(, $_rest:tt)*) => { 1 + $crate::deck!(@depth $($first)*) };
    (@depth $first:expr $(, $_rest:expr)*) => { 1usize };
    (@depth) => { 0usize };
    // Internal: push items into the deck.
    // Groups: first group inherits the assigned depth; each non-first group
    // gets its own intrinsic depth (the ruler-sequence rule).
    (@push $deck:ident, $depth:expr; ($($g0:tt)*) $(, ($($gn:tt)*))*) => {
        $crate::deck!(@push $deck, $depth; $($g0)*);
        $($crate::deck!(@push $deck, $crate::deck!(@depth $($gn)*); $($gn)*);)*
    };
    // Flat values: first gets the assigned depth, rest get 0.
    (@push $deck:ident, $depth:expr; $v0:expr $(, $vn:expr)*) => {
        $deck.push($v0, $depth as u8);
        $($deck.push($vn, 0u8);)*
    };
    (@push $deck:ident, $depth:expr;) => {};
    // Entry point.
    ($($tt:tt)*) => {{
        let mut _deck = $crate::Deck::default();
        $crate::deck!(@push _deck, $crate::deck!(@depth $($tt)*); $($tt)*);
        _deck
    }};
}

impl<T> Deck<T> {
    pub fn max_depth(&self) -> u8 {
        self.depths.first().copied().unwrap_or(0u8)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn push(&mut self, item: T, depth: u8) {
        self.items.push(item);
        self.push_depth(depth);
    }

    fn push_depth(&mut self, depth: u8) {
        // Ensure first marker is big enough.
        let depth = match self.depths.first().copied() {
            Some(d) => d.min(depth),
            None => depth,
        };
        if depth as usize > self.pegs.len() {
            self.pegs.resize(depth as usize, 0);
        }
        // Update the scan then push the actual marker.
        self.stride_offset.push(
            self.stride_offset.last().copied().unwrap_or(0)
                + self.depths.last().copied().unwrap_or(0) as usize,
        );
        // Update strides.
        self.strides
            .resize(self.strides.len() + depth as usize, usize::MAX);
        for i in 0..depth as usize {
            let peg = self.pegs[i];
            if peg < self.depths.len() {
                let dst = &mut self.strides[self.stride_offset[peg] + i];
                *dst = (*dst).min(self.depths.len() - peg);
            }
            self.pegs[i] = self.depths.len();
        }
        self.depths.push(depth);
    }

    fn stride(&self, pos: usize, depth: u8) -> usize {
        match self.depths.get(pos) {
            Some(_) if depth == 0 => 1usize,
            Some(d) if depth > *d => self.items.len() - pos,
            None => 0usize,
            _ => self.strides[self.stride_offset[pos] + (depth as usize) - 1]
                .min(self.items.len() - pos),
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.depths.clear();
        self.stride_offset.clear();
        self.strides.clear();
        self.pegs.clear();
    }

    pub fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
        self.depths.reserve(additional);
        self.stride_offset.reserve(additional);
    }

    pub fn flatten(&mut self) {
        let n = self.items.len();
        debug_assert_eq!(n, self.depths.len());
        self.depths.fill(0u8);
        if let Some(dfirst) = self.depths.first_mut()
            && n > 1
        {
            *dfirst = 1;
        }
        self.stride_offset.fill(0);
        self.strides.truncate(1);
        self.pegs.truncate(1);
        self.strides.fill(0usize);
        if let Some(first) = self.strides.first_mut() {
            *first = n;
        }
        self.pegs.fill(0usize);
    }

    pub fn graft(&mut self) {
        for d in self.depths.iter_mut() {
            *d += 1; // Will panic on overflow. Intentional.
        }
        calc_strides(
            &self.depths,
            &mut self.pegs,
            &mut self.stride_offset,
            &mut self.strides,
        );
    }

    pub fn simplify(&mut self) {
        let set = BTreeSet::from_iter(self.depths.iter());
        let mut replace = vec![0u8; 1 + (self.max_depth() as usize)];
        for (i, d) in set.into_iter().enumerate() {
            replace[*d as usize] = i as u8;
        }
        for d in self.depths.iter_mut() {
            *d = replace[*d as usize];
        }
        calc_strides(
            &self.depths,
            &mut self.pegs,
            &mut self.stride_offset,
            &mut self.strides,
        );
    }

    pub fn view(&self, depth: u8) -> View<'_, T> {
        View {
            deck: self,
            depth,
            start: 0usize,
            end: self.len(),
        }
    }
}

pub struct View<'a, T> {
    deck: &'a Deck<T>,
    depth: u8,
    start: usize,
    end: usize,
}

impl<'a, T> View<'a, T> {
    pub fn depth(&self) -> u8 {
        self.depth
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn children(&self) -> View<'a, T> {
        View {
            deck: self.deck,
            depth: self.depth - 1,
            start: self.start,
            end: self.end,
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.deck.items[self.start..self.end]
    }

    pub fn as_ref(&self) -> &T {
        &self.deck.items[self.start]
    }
}

impl<'a, T> Iterator for View<'a, T> {
    type Item = View<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let pos = self.start;
            let end = pos + self.deck.stride(pos, self.depth);
            self.start = end;
            Some(View {
                deck: self.deck,
                depth: self.depth,
                start: pos,
                end,
            })
        } else {
            None
        }
    }
}

fn calc_strides(
    depths: &[u8],
    pegs: &mut Vec<usize>,
    stride_offset: &mut Vec<usize>,
    strides: &mut Vec<usize>,
) {
    pegs.clear();
    stride_offset.clear();
    strides.clear();
    // Exclusive prefix sum of depths.
    stride_offset.extend(depths.iter().scan(0usize, |acc, &d| {
        let prev = *acc;
        *acc += d as usize;
        Some(prev)
    }));
    // One stride entry per depth unit per element.
    let total: usize =
        stride_offset.last().copied().unwrap_or(0) + depths.last().copied().unwrap_or(0) as usize;
    strides.resize(total, usize::MAX);
    // Fill strides using pegs (same logic as push_depth, but in batch).
    for (i, &d) in depths.iter().enumerate() {
        let d = d as usize;
        if d > pegs.len() {
            pegs.resize(d, 0);
        }
        for j in 0..d {
            let peg = pegs[j];
            if peg < i {
                let dst = &mut strides[stride_offset[peg] + j];
                *dst = (*dst).min(i - peg);
            }
            pegs[j] = i;
        }
    }
}

#[cfg(test)]
mod test {
    use super::Deck;

    fn binary_deck(depth: u8) -> Deck<usize> {
        let mut deck = Deck::<usize>::default();
        for i in 0usize..(1 << depth) {
            deck.push(i, (i.trailing_zeros() as u8).min(depth));
        }
        deck
    }

    #[test]
    fn t_basic_ops() {
        const DEPTH: u8 = 5;
        let deck = binary_deck(DEPTH);
        assert_eq!(deck.len(), 1 << DEPTH);
        assert_eq!(deck.max_depth(), DEPTH);
        {
            // Iterate from level 5.
            let mut counter = 0;
            let v5 = deck.view(DEPTH);
            assert_eq!(v5.depth(), 5);
            for v4 in v5.children() {
                assert_eq!(v4.depth(), 4);
                for v3 in v4.children() {
                    assert_eq!(v3.depth(), 3);
                    for v2 in v3.children() {
                        assert_eq!(v2.depth(), 2);
                        for v1 in v2.children() {
                            assert_eq!(v1.depth(), 1);
                            // First iterate over v0 references.
                            let prev = counter;
                            for v0 in v1.children() {
                                assert_eq!(v0.depth(), 0);
                                assert_eq!(v0.len(), 1);
                                assert_eq!(v0.as_ref(), &counter);
                                counter += 1;
                            }
                            counter = prev;
                            // Extract slice and iterate over the slice.
                            let v1 = v1.as_slice();
                            assert_eq!(v1.len(), 2);
                            for val in v1 {
                                assert_eq!(*val, counter);
                                counter += 1;
                            }
                        }
                    }
                }
            }
        }
        {
            // Iterate from level 4.
            let mut counter = 0;
            let v4 = deck.view(DEPTH - 1);
            assert_eq!(v4.depth(), 4);
            for v3 in v4.children() {
                assert_eq!(v3.depth(), 3);
                for v2 in v3.children() {
                    assert_eq!(v2.depth(), 2);
                    for v1 in v2.children() {
                        assert_eq!(v1.depth(), 1);
                        // First iterate over v0 references.
                        let prev = counter;
                        for v0 in v1.children() {
                            assert_eq!(v0.depth(), 0);
                            assert_eq!(v0.len(), 1);
                            assert_eq!(v0.as_ref(), &counter);
                            counter += 1;
                        }
                        counter = prev;
                        // Extract slice and iterate over the slice.
                        let v1 = v1.as_slice();
                        assert_eq!(v1.len(), 2);
                        for val in v1 {
                            assert_eq!(*val, counter);
                            counter += 1;
                        }
                    }
                }
            }
        }
        {
            // Iterate from level 3.
            let mut counter = 0;
            let v3 = deck.view(DEPTH - 2);
            assert_eq!(v3.depth(), 3);
            for v2 in v3.children() {
                assert_eq!(v2.depth(), 2);
                for v1 in v2.children() {
                    assert_eq!(v1.depth(), 1);
                    // First iterate over v0 references.
                    let prev = counter;
                    for v0 in v1.children() {
                        assert_eq!(v0.depth(), 0);
                        assert_eq!(v0.len(), 1);
                        assert_eq!(v0.as_ref(), &counter);
                        counter += 1;
                    }
                    counter = prev;
                    // Extract slice and iterate over the slice.
                    let v1 = v1.as_slice();
                    assert_eq!(v1.len(), 2);
                    for val in v1 {
                        assert_eq!(*val, counter);
                        counter += 1;
                    }
                }
            }
        }
    }

    #[test]
    fn t_deck_creation_macro() {
        // depth-1: flat list
        let d1 = deck!(0usize, 1, 2, 3);
        assert_eq!(d1.len(), 4);
        assert_eq!(d1.max_depth(), 1);
        assert_eq!(d1.items, vec![0, 1, 2, 3]);
        assert_eq!(d1.depths, vec![1, 0, 0, 0]);
        // depth-2: list of lists
        let d2 = deck!((0usize, 1), (2, 3));
        assert_eq!(d2.len(), 4);
        assert_eq!(d2.max_depth(), 2);
        assert_eq!(d2.items, vec![0, 1, 2, 3]);
        assert_eq!(d2.depths, vec![2, 0, 1, 0]);
        // depth-3: should match binary_deck(3)
        let d3 = deck!(((0usize, 1), (2, 3)), ((4, 5), (6, 7)));
        let expected = binary_deck(3);
        assert_eq!(d3.items, expected.items);
        assert_eq!(d3.depths, expected.depths);
        // depth-3 with 3-way branching at the top
        let d3b = deck!(((0usize, 1), (2, 3)), ((4, 5), (6, 7)), ((8, 9), (10, 11)));
        assert_eq!(d3b.len(), 12);
        assert_eq!(d3b.max_depth(), 3);
        assert_eq!(d3b.depths, vec![3, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0]);
        // single element
        let d_one = deck!(42usize);
        assert_eq!(d_one.len(), 1);
        assert_eq!(d_one.max_depth(), 1);
        assert_eq!(d_one.depths, vec![1]);
    }

    #[test]
    fn t_deck_edge_cases() {
        // Empty deck.
        let empty = Deck::<usize>::default();
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.max_depth(), 0);
        assert_eq!(empty.view(0).count(), 0);
        // Single element at depth 0 (bare leaf).
        let mut d = Deck::default();
        d.push(42usize, 0);
        assert_eq!(d.len(), 1);
        assert_eq!(d.max_depth(), 0);
        let mut v = d.view(0);
        let item = v.next().unwrap();
        assert_eq!(item.as_ref(), &42);
        assert!(v.next().is_none());
        // Single element at depth 1 (one-element list).
        let mut d = Deck::default();
        d.push(7usize, 1);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(d.view(1).count(), 1);
        assert_eq!(d.view(1).next().unwrap().as_slice(), &[7]);
        // clear() resets everything.
        let mut d = binary_deck(3);
        d.clear();
        assert_eq!(d.len(), 0);
        assert_eq!(d.max_depth(), 0);
        // Re-use after clear.
        d.push(1usize, 1);
        d.push(2usize, 0);
        assert_eq!(d.len(), 2);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(d.view(1).next().unwrap().as_slice(), &[1, 2]);
    }

    #[test]
    fn t_deck_flatten() {
        // Flatten a deep deck: all items survive, structure collapses to depth 1.
        let mut d = binary_deck(4);
        let items_before: Vec<usize> = d.items.clone();
        d.flatten();
        assert_eq!(d.items, items_before);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(d.depths[0], 1);
        assert!(d.depths[1..].iter().all(|&x| x == 0));
        // Iterating at depth 1 yields all elements as one flat group.
        assert_eq!(d.view(1).count(), 1);
        assert_eq!(
            d.view(1).next().unwrap().as_slice(),
            items_before.as_slice()
        );
        // Flatten a single-element deck: depth stays 0.
        let mut d = Deck::default();
        d.push(5usize, 0);
        d.flatten();
        assert_eq!(d.max_depth(), 0);
        assert_eq!(d.depths, vec![0]);
        // Flatten an already-flat deck is idempotent.
        let mut d = binary_deck(3);
        d.flatten();
        let depths_after_first = d.depths.clone();
        d.flatten();
        assert_eq!(d.depths, depths_after_first);
    }

    #[test]
    fn t_deck_graft() {
        // Graft increments every depth by 1.
        let mut d = binary_deck(3);
        let depths_before: Vec<u8> = d.depths.clone();
        d.graft();
        assert_eq!(d.max_depth(), 4);
        for (before, after) in depths_before.iter().zip(d.depths.iter()) {
            assert_eq!(*after, before + 1);
        }
        // Items are unchanged.
        assert_eq!(d.items, binary_deck(3).items);
        // Iteration still produces correct sequential values after graft:
        // The deck is now a depth-4 structure; iterating at depth 4 gives 2 groups.
        let mut counter = 0usize;
        for v3 in d.view(4).children() {
            for v2 in v3.children() {
                for v1 in v2.children() {
                    for val in v1.as_slice() {
                        assert_eq!(*val, counter);
                        counter += 1;
                    }
                }
            }
        }
        assert_eq!(counter, 8);
        // Graft then flatten roundtrip: items survive.
        let mut d = binary_deck(2);
        let items = d.items.clone();
        d.graft();
        d.flatten();
        assert_eq!(d.items, items);
    }

    #[test]
    fn t_deck_simplify() {
        // A deck whose depths already use every level 0..=max is unchanged.
        let mut d = binary_deck(3);
        let depths_before = d.depths.clone();
        d.simplify();
        assert_eq!(d.depths, depths_before);
        // A deck with gaps: only depths 0, 2, 5 present → remapped to 0, 1, 2.
        let mut d = Deck::default();
        d.push(0usize, 5);
        d.push(1usize, 0);
        d.push(2usize, 2);
        d.push(3usize, 0);
        assert_eq!(d.depths, vec![5, 0, 2, 0]);
        d.simplify();
        assert_eq!(d.max_depth(), 2);
        assert_eq!(d.depths, vec![2, 0, 1, 0]);
        // Iteration still works correctly after remapping.
        assert_eq!(d.view(2).count(), 1);
        let mut counter = 0usize;
        for v1 in d.view(2).children() {
            for val in v1.as_slice() {
                assert_eq!(*val, counter);
                counter += 1;
            }
        }
        assert_eq!(counter, 4);
        // Simplify is idempotent.
        d.simplify();
        assert_eq!(d.depths, vec![2, 0, 1, 0]);
    }
}
