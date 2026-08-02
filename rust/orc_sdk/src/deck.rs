use crate::{
    Error,
    bindings::{OrcHandle, OrcMark},
    slice_from_ptr,
};
use std::{
    fmt::Display,
    ops::{Index, IndexMut, Range},
};

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Deck<T>
where
    T: Default,
{
    items: Vec<T>,
    marks: Vec<OrcMark>,
    stride_offset: Vec<u64>, // Explicit 64 bit int for FFI
    strides: Vec<u64>,       // Explicit 64 bit int for FFI
    pegs: Vec<usize>,        // This is not used in FFI.
}

/**
This macro creates a `Deck` from a nested parenthesized literal.

``` rust
let d = orc_sdk::deck![1, 2, 3];                                // depth-1: a flat list
let d = orc_sdk::deck![[1, 2], [3, 4]];                         // depth-2: list of lists
let d = orc_sdk::deck![[[1, 2], [3, 4]], [[5, 6], [7, 8]]]; // depth-3
```
*/
#[macro_export]
macro_rules! deck {
    // Internal: count nesting depth along the first element's path.
    (@depth [$($first:tt)*] $(, $_rest:tt)*) => { 1 + $crate::deck!(@depth $($first)*) };
    (@depth $first:expr $(, $_rest:expr)*) => { 1usize };
    (@depth) => { 1usize };
    // Internal: push items into the deck.
    // Groups: first group inherits the assigned depth; each non-first group
    // gets its own intrinsic depth (the ruler-sequence rule).
    (@push $deck:ident, $depth:expr; [$($g0:tt)*] $(, [$($gn:tt)*])*) => {
        $crate::deck!(@push $deck, $depth; $($g0)*);
        $($crate::deck!(@push $deck, $crate::deck!(@depth $($gn)*); $($gn)*);)*
    };
    // Flat values: first gets the assigned depth, rest get 0.
    (@push $deck:ident, $depth:expr; $v0:expr $(, $vn:expr)*) => {
        $deck.push($v0, $depth as u8);
        $($deck.push($vn, 0u8);)*
    };
    (@push $deck:ident, $depth:expr;) => { $deck.start_new_arr($depth as u8); };
    // Entry point (square brackets, like vec![]).
    [$($tt:tt)*] => {{
        let mut _deck = $crate::Deck::default();
        $crate::deck!(@push _deck, $crate::deck!(@depth $($tt)*); $($tt)*);
        _deck
    }};
    // Entry point (parentheses).
    ($($tt:tt)*) => {{
        let mut _deck = $crate::Deck::default();
        $crate::deck!(@push _deck, $crate::deck!(@depth $($tt)*); $($tt)*);
        _deck
    }};
}

impl<T> Deck<T>
where
    T: Default,
{
    /**
    Create a deck from the raw data buffer, and a raw marks buffer that indicates the structure of the data.
     */
    pub fn from_raw_data(items: &[T], marks: &[OrcMark]) -> Self
    where
        T: Clone,
    {
        let mut out = Deck {
            items: items.to_vec(),
            marks: marks.to_vec(),
            stride_offset: Default::default(),
            strides: Default::default(),
            pegs: Default::default(),
        };
        calc_strides(
            &out.marks,
            &mut out.pegs,
            &mut out.stride_offset,
            &mut out.strides,
        );
        out
    }

    /**
    Get the maximum depth of this deck. By definition, this is the depth of the
    first mark.
    */
    pub fn max_depth(&self) -> u8 {
        self.marks.first().map(|m| m.depth + 1).unwrap_or(0u8)
    }

    /**
    Number of items in this deck.
    */
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /**
    Push an item. `depth` uses external numbering: 0 = continuation (no mark),
    1+ = starts a new group at that level.
    */
    pub fn push(&mut self, item: T, depth: u8) {
        self.start_new_arr(depth);
        self.items.push(item);
    }

    /**
    Push an empty group at the given external depth (must be >= 1).  No item is
    added; this creates a mark at the current item position.
    */
    pub fn start_new_arr(&mut self, depth: u8) {
        if depth > 0 {
            let pos = self.items.len();
            self.push_mark(OrcMark {
                depth: depth - 1,
                pos: pos as u64,
            });
        }
    }

    fn push_mark(&mut self, mark: OrcMark) {
        // Ensure no mark exceeds the first mark's depth. Not letting the depths
        // of subsequent marks exceed the first mark allows us to incrementally
        // and efficiently update the offsets and strides, without recomputing
        // the whole thing. This is because we don't have to change the number
        // of offsets we have to remember for the first mark.
        let depth = match self.marks.first() {
            Some(m) => m.depth.min(mark.depth),
            None => mark.depth,
        };
        let mark = OrcMark {
            depth,
            pos: mark.pos,
        };
        let d = depth as usize; // number of stride levels for this mark
        if d > self.pegs.len() {
            self.pegs.resize(d, 0);
        }
        // Update the scan then push the actual marker.
        self.stride_offset
            .push(calc_stride_count(&self.marks, &self.stride_offset) as u64);
        // Update strides.
        self.strides.resize(self.strides.len() + d, u64::MAX);
        for i in 0..d {
            let peg = self.pegs[i];
            if peg < self.marks.len() {
                let dst = &mut self.strides[self.stride_offset[peg] as usize + i];
                *dst = (*dst).min((self.marks.len() - peg) as u64);
            }
            self.pegs[i] = self.marks.len();
        }
        self.marks.push(mark);
    }

    /**
    Clear the contents of this deck.
    */
    pub fn clear(&mut self) {
        self.items.clear();
        self.marks.clear();
        self.stride_offset.clear();
        self.strides.clear();
        self.pegs.clear();
    }

    /**
    Reserve memory for `additional` number of elements.
    */
    pub fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
        self.marks.reserve(additional);
        self.stride_offset.reserve(additional);
    }

    /**
    Flattens all items into one list.

    ```rust
    let mut d = orc_sdk::deck![[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    d.flatten();
    assert_eq!(d, orc_sdk::deck![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    ```
    */
    pub fn flatten(&mut self) {
        self.marks.clear();
        self.stride_offset.clear();
        self.strides.clear();
        self.pegs.clear();
        if self.items.len() > 1 {
            self.push_mark(OrcMark { depth: 0, pos: 0 });
        }
    }

    /**
    Increases the depth of the tree by 1, by wrapping each item in it's own list.

    ```rust
    let mut d = orc_sdk::deck![[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    d.graft();
    assert_eq!(d, orc_sdk::deck![[[1], [2], [3]], [[4], [5], [6]], [[7], [8], [9]]]);
    ```
    */
    pub fn graft(&mut self) {
        let count = self.marks.len();
        let old_marks = std::mem::replace(
            &mut self.marks,
            Vec::with_capacity(count + self.items.len()),
        );
        let mut prev = 0u64;
        for mut m in old_marks.into_iter() {
            m.depth = m.depth.checked_add(1).expect("Depth overflow");
            self.marks
                .extend((prev..m.pos).map(|pos| OrcMark { depth: 0, pos }));
            self.marks.push(m);
            prev = m.pos + 1;
        }
        self.marks
            .extend((prev..(self.items.len() as u64)).map(|pos| OrcMark { depth: 0, pos }));
        calc_strides(
            &self.marks,
            &mut self.pegs,
            &mut self.stride_offset,
            &mut self.strides,
        );
    }

    /**
    Eliminates redundant hierarchy in the tree. For example, if every depth-2
    list in this tree only ever contains a single depth-1 child, then all
    depth-2 lists are unwrapped, as the structure they provide is
    redundant. Keep in mind, this will not combine data from different
    branches/sub-lists. It only eliminates redundant hierarchy.

    ```rust
    let mut d = orc_sdk::deck![[[1, 2, 3]], [[4, 5, 6]], [[7, 8, 9]]];
    d.simplify(); // This eliminates 1 level of redundant hierarchy.
    assert_eq!(d, orc_sdk::deck![[1, 2, 3], [4, 5, 6], [7, 8, 9]]);

    d = orc_sdk::deck![[[[1, 2, 3]]], [[[4, 5, 6]]], [[[7, 8, 9]]]];
    d.simplify(); // This eliminates 2 levels of redundant hierarchy.
    assert_eq!(d, orc_sdk::deck![[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
    ```
     */
    pub fn simplify(&mut self) {
        let dmax = self.marks.first().map(|m| m.depth).unwrap_or(0) as usize;
        let mut remap = [0u8; u8::MAX as usize + 1];
        let remap = &mut remap[..=dmax];
        for m in self.marks.iter() {
            // Mark depths that are seen.
            remap[m.depth as usize] = 1;
        }
        let mut acc = 0u8;
        for r in remap.iter_mut() {
            let prev = acc;
            acc += *r;
            *r = prev;
        }
        for m in self.marks.iter_mut() {
            m.depth = remap[m.depth as usize];
        }
        calc_strides(
            &self.marks,
            &mut self.pegs,
            &mut self.stride_offset,
            &mut self.strides,
        );
    }

    pub fn view(&self, depth: u8) -> DeckView<'_, T> {
        DeckView {
            items: &self.items,
            cursor: ReadCursor {
                n_items: self.items.len(),
                marks: &self.marks,
                strides: &self.strides,
                stride_offset: &self.stride_offset,
                depth,
                start: 0,
                end: if depth == 0 {
                    self.items.len()
                } else {
                    self.marks.len()
                },
            },
        }
    }

    pub fn writer(&mut self, depth: u8) -> DeckWriter<'_, T> {
        let start = self.len();
        DeckWriter {
            deck: self,
            cursor: WriteCursor {
                depth,
                next_depth: Some(depth),
            },
            start,
        }
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn marks(&self) -> &[OrcMark] {
        &self.marks
    }

    pub fn stride_info(&self) -> (&[u64], &[u64]) {
        (&self.stride_offset, &self.strides)
    }
}

pub fn fmt_raw_deck<T: Default + Display>(
    items: &[T],
    marks: &[OrcMark],
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    if items.is_empty() && marks.is_empty() {
        return writeln!(f, "<empty_deck>");
    }
    const TAB_WIDTH: usize = 3;
    let dmax = marks.first().map(|m| m.depth + 1).unwrap_or(0u8);
    let n_items = items.len() as u64;
    let mut tail_start = 0u64;
    for w in marks.windows(2) {
        let (m, next_pos) = (&w[0], w[1].pos);
        write!(
            f,
            "{lp:>width$}",
            lp = "",
            width = ((dmax - m.depth) as usize) * TAB_WIDTH
        )?;
        let d_current = m.depth + 1;
        write!(
            f,
            "{d:>3} {rp:─>rw$}",
            d = d_current,
            rp = "┤",
            rw = (d_current as usize) * TAB_WIDTH
        )?;
        if m.pos < next_pos {
            let end = next_pos.min(n_items);
            let mut iter = m.pos..end;
            if let Some(i) = iter.next() {
                writeln!(f, " {}", items[i as usize])?;
            }
            for i in iter {
                writeln!(
                    f,
                    "{lp:>width$}   ┤ {}",
                    items[i as usize],
                    lp = "",
                    width = (dmax as usize + 1) * TAB_WIDTH
                )?;
            }
        } else {
            writeln!(f)?;
        }
        tail_start = next_pos;
    }
    if let Some(last) = marks.last() {
        let next_pos = n_items;
        write!(
            f,
            "{lp:>width$}",
            lp = "",
            width = ((dmax - last.depth) as usize) * TAB_WIDTH
        )?;
        let d_current = last.depth + 1;
        write!(
            f,
            "{d:>3} {rp:─>rw$}",
            d = d_current,
            rp = "┤",
            rw = (d_current as usize) * TAB_WIDTH
        )?;
        if last.pos < next_pos {
            let end = next_pos.min(n_items);
            let mut iter = last.pos..end;
            if let Some(i) = iter.next() {
                writeln!(f, " {}", items[i as usize])?;
            }
            for i in iter {
                writeln!(
                    f,
                    "{lp:>width$}   ┤ {}",
                    items[i as usize],
                    lp = "",
                    width = (dmax as usize + 1) * TAB_WIDTH
                )?;
            }
        } else {
            writeln!(f)?;
        }
        tail_start = n_items;
    }
    // Items after the last mark (or all items if no marks).
    for item in items.iter().skip(tail_start as usize) {
        writeln!(
            f,
            "{lp:>width$}   ┤ {}",
            item,
            lp = "",
            width = (dmax as usize + 1) * TAB_WIDTH
        )?;
    }
    Ok(())
}

impl<T> Display for Deck<T>
where
    T: Default + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_raw_deck(&self.items, &self.marks, f)
    }
}

fn stride(
    marks: &[OrcMark],
    strides: &[u64],
    stride_offset: &[u64],
    mark_idx: usize,
    depth: u8,
) -> usize {
    match marks.get(mark_idx) {
        Some(_) if depth == 0 => 1,
        Some(m) if depth > m.depth => marks.len() - mark_idx,
        None => 0usize,
        _ => strides[(stride_offset[mark_idx] + (depth - 1) as u64) as usize]
            .min((marks.len() - mark_idx) as u64) as usize,
    }
}

fn mark_pos(marks: &[OrcMark], n_items: usize, idx: usize) -> usize {
    marks.get(idx).map(|m| m.pos as usize).unwrap_or(n_items)
}

#[derive(Clone, Debug)]
pub(crate) struct ReadCursor<'a> {
    n_items: usize,
    marks: &'a [OrcMark],
    strides: &'a [u64],
    stride_offset: &'a [u64],
    depth: u8,
    start: usize,
    end: usize,
}

/// A view into a `Deck`. At depth >= 1, `start`/`end` are mark indices.
/// At depth 0, they are item indices.
pub struct DeckView<'a, T> {
    items: &'a [T],
    cursor: ReadCursor<'a>,
}

impl<'a> ReadCursor<'a> {
    fn depth(&self) -> u8 {
        self.depth
    }

    fn len(&self) -> usize {
        if self.start >= self.end {
            0
        } else if self.depth == 0 {
            1
        } else {
            let start = mark_pos(self.marks, self.n_items, self.start);
            let end = mark_pos(
                self.marks,
                self.n_items,
                self.start
                    + stride(
                        self.marks,
                        self.strides,
                        self.stride_offset,
                        self.start,
                        self.depth - 1,
                    ),
            );
            end - start
        }
    }

    fn range(&self) -> Range<usize> {
        if self.depth == 0 {
            self.start..(self.start + 1)
        } else {
            let start = mark_pos(self.marks, self.n_items, self.start);
            let end = mark_pos(
                self.marks,
                self.n_items,
                self.start
                    + stride(
                        self.marks,
                        self.strides,
                        self.stride_offset,
                        self.start,
                        self.depth - 1,
                    ),
            );
            start..end
        }
    }

    fn child(&self) -> Self {
        if self.depth == 0 {
            ReadCursor {
                n_items: self.n_items,
                marks: self.marks,
                strides: self.strides,
                stride_offset: self.stride_offset,
                depth: self.depth,
                start: self.start,
                end: self.end,
            }
        } else if self.depth < 2 {
            ReadCursor {
                n_items: self.n_items,
                marks: self.marks,
                strides: self.strides,
                stride_offset: self.stride_offset,
                depth: 0,
                start: mark_pos(self.marks, self.n_items, self.start),
                end: mark_pos(
                    self.marks,
                    self.n_items,
                    self.start
                        + stride(
                            self.marks,
                            self.strides,
                            self.stride_offset,
                            self.start,
                            self.depth - 1,
                        ),
                ),
            }
        } else {
            ReadCursor {
                n_items: self.n_items,
                marks: self.marks,
                strides: self.strides,
                stride_offset: self.stride_offset,
                depth: self.depth - 1,
                start: self.start,
                end: self.start
                    + stride(
                        self.marks,
                        self.strides,
                        self.stride_offset,
                        self.start,
                        self.depth - 1,
                    ),
            }
        }
    }

    fn advance(&mut self) -> bool {
        if self.start >= self.end {
            return false;
        }
        let mut new_start = self.start;
        if self.depth == 0 {
            new_start += 1;
        } else {
            new_start += stride(
                self.marks,
                self.strides,
                self.stride_offset,
                self.start,
                self.depth - 1,
            );
        }
        if new_start < self.end {
            self.start = new_start;
            true
        } else {
            false
        }
    }
}

impl<'a, T> AsRef<T> for DeckView<'a, T>
where
    T: Default,
{
    fn as_ref(&self) -> &T {
        if self.cursor.depth == 0 {
            &self.items[self.cursor.start]
        } else {
            &self.items[self.cursor.marks[self.cursor.start].pos as usize]
        }
    }
}

impl<'a, T> DeckView<'a, T>
where
    T: Default,
{
    pub fn depth(&self) -> u8 {
        self.cursor.depth()
    }

    pub fn len(&self) -> usize {
        self.cursor.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items[self.cursor.range()]
    }

    pub fn child(&self) -> DeckView<'a, T> {
        DeckView {
            items: self.items,
            cursor: self.cursor.child(),
        }
    }

    pub fn advance_iter(mut self) -> impl Iterator<Item = DeckView<'a, T>> {
        let mut valid = true;
        std::iter::from_fn(move || {
            if valid && self.cursor.start < self.cursor.end {
                let copy = DeckView {
                    items: self.items,
                    cursor: self.cursor.clone(),
                };
                valid = self.cursor.advance();
                Some(copy)
            } else {
                None
            }
        })
    }

    pub fn from_handle(handle: &'a OrcHandle) -> Self {
        let (marks, stride_offset, strides) = unsafe {
            let marks = slice_from_ptr(handle.marks.cast(), handle.n_marks as usize);
            let stride_offset = slice_from_ptr(handle.stride_offset, handle.n_marks as usize);
            let strides = slice_from_ptr(handle.strides, calc_stride_count(marks, stride_offset));
            (marks, stride_offset, strides)
        };
        let depth = marks.first().map(|m| m.depth + 1).unwrap_or(0u8);
        let end = if depth == 0 {
            handle.n_items
        } else {
            handle.n_marks
        } as usize;
        Self {
            items: unsafe { slice_from_ptr(handle.items.cast(), handle.n_items as usize) },
            cursor: ReadCursor {
                n_items: handle.n_items as usize,
                marks,
                strides,
                stride_offset,
                depth,
                start: 0,
                end,
            },
        }
    }
}

impl<'a, T> Index<usize> for DeckView<'a, T>
where
    T: Default,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let arr = self.as_slice();
        &arr[index]
    }
}

#[derive(Debug)]
pub(crate) struct WriteCursor {
    depth: u8,
    next_depth: Option<u8>,
}

/// A view into a `Deck`. At depth >= 1, `start`/`end` are mark indices.
/// At depth 0, they are item indices.
pub struct DeckWriter<'a, T>
where
    T: Default,
{
    deck: &'a mut Deck<T>,
    cursor: WriteCursor,
    start: usize,
}

impl WriteCursor {
    pub(crate) fn advance(&mut self) {
        self.next_depth = Some(self.depth);
    }

    pub(crate) fn child(&mut self) -> Self {
        let depth = self.depth.saturating_sub(1);
        WriteCursor {
            depth,
            next_depth: self.next_depth.take().or(Some(depth)),
        }
    }

    fn depth(&self) -> u8 {
        self.depth
    }

    pub(crate) fn take(&mut self) -> Self {
        let depth = self.depth;
        WriteCursor {
            depth,
            next_depth: self.next_depth.take().or(Some(depth)),
        }
    }

    pub(crate) fn writer<'a, T: Default>(&mut self, deck: &'a mut Deck<T>) -> DeckWriter<'a, T> {
        let start = deck.len();
        DeckWriter {
            deck,
            cursor: self.take(),
            start,
        }
    }
}

impl<'a, T> DeckWriter<'a, T>
where
    T: Default,
{
    pub fn child(&mut self) -> DeckWriter<'_, T> {
        let start = self.deck.len();
        DeckWriter {
            deck: self.deck,
            cursor: self.cursor.child(),
            start,
        }
    }

    pub fn depth(&self) -> u8 {
        self.cursor.depth()
    }

    pub fn push(&mut self, item: T) {
        self.deck
            .push(item, self.cursor.next_depth.take().unwrap_or(0u8));
    }

    pub fn extend_from_slice(&mut self, items: &[T])
    where
        T: Copy,
    {
        let depth = self.cursor.next_depth.take().unwrap_or(0);
        self.deck.start_new_arr(depth);
        self.deck.items.extend_from_slice(items);
    }

    pub fn len(&self) -> usize {
        self.deck.len() - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_slice(&self) -> &[T] {
        &self.deck.items[self.start..]
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        &mut self.deck.items[self.start..]
    }

    pub fn push_default_mut(&mut self) -> &mut T {
        let i = self.deck.items.len();
        self.push(T::default());
        // SAFETY: We just pushed an element to his index.
        unsafe { self.deck.items.get_unchecked_mut(i) }
    }
}

/**
If a DeckWriter is created, and dropped with no items inserted, it should still insert
it's mark to indicate an empty array.
 */
impl<'a, T> Drop for DeckWriter<'a, T>
where
    T: Default,
{
    fn drop(&mut self) {
        if let Some(d) = self.cursor.next_depth.take() {
            self.deck.start_new_arr(d);
        }
    }
}

impl<'a, T> Extend<T> for DeckWriter<'a, T>
where
    T: Default,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let depth = self.cursor.next_depth.take().unwrap_or(0);
        self.deck.start_new_arr(depth);
        self.deck.items.extend(iter);
    }
}

impl<'a, T> IndexMut<usize> for DeckWriter<'a, T>
where
    T: Default,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let arr = self.as_slice_mut();
        &mut arr[index]
    }
}

impl<'a, T> Index<usize> for DeckWriter<'a, T>
where
    T: Default,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let arr = self.as_slice();
        &arr[index]
    }
}

fn calc_stride_count(marks: &[OrcMark], stride_offset: &[u64]) -> usize {
    stride_offset.last().copied().unwrap_or(0) as usize
        + marks.last().map(|m| m.depth as usize).unwrap_or(0)
}

fn calc_strides(
    marks: &[OrcMark],
    pegs: &mut Vec<usize>,
    stride_offset: &mut Vec<u64>,
    strides: &mut Vec<u64>,
) {
    pegs.clear();
    stride_offset.clear();
    strides.clear();
    // Exclusive prefix sum of depth.
    stride_offset.extend(marks.iter().scan(0u64, |acc, m| {
        let prev = *acc;
        *acc += m.depth as u64;
        Some(prev)
    }));
    // One stride entry per depth level per mark.
    let total = calc_stride_count(marks, stride_offset);
    strides.resize(total, u64::MAX);
    // Fill strides using pegs.
    for (i, m) in marks.iter().enumerate() {
        let d = m.depth as usize;
        if d > pegs.len() {
            pegs.resize(d, 0);
        }
        for j in 0..d {
            let peg = pegs[j];
            if peg < i {
                let dst = &mut strides[stride_offset[peg] as usize + j];
                *dst = (*dst).min((i - peg) as u64);
            }
            pegs[j] = i;
        }
    }
}

#[derive(Debug)]
pub struct Combinations<'a> {
    input_cursors: Box<[ReadCursor<'a>]>,
    input_depths: Box<[u8]>,
    output_cursors: Box<[WriteCursor]>,
    output_depths: Box<[u8]>,
    stack_depth: usize,
}

impl<'a> Combinations<'a> {
    pub fn from_handles(
        inputs: &[OrcHandle],
        input_depths: &[u8],
        output_depths: &[u8],
    ) -> Result<Self, Error> {
        if input_depths.len() != inputs.len() {
            return Err(Error::InvalidCombinations);
        }
        let n_outputs = output_depths.len();
        if output_depths.len() != n_outputs {
            return Err(Error::InvalidCombinations);
        }
        let max_delta = inputs
            .iter()
            .zip(input_depths.iter())
            .map(|(i, arg_depth)| {
                let marks = unsafe { slice_from_ptr(i.marks, i.n_marks as usize) };
                let depth = marks.first().map(|m| m.depth + 1).unwrap_or(0);
                depth.saturating_sub(*arg_depth)
            })
            .max()
            .unwrap_or(0);
        let stack_depth = max_delta as usize + 1;
        let input_cursors =
            inputs
                .iter()
                .zip(input_depths.iter())
                .flat_map(|(input, &arg_depth)| {
                    // SAFETY: Assembling slices from pointers. The only tricky part is computing
                    // the length of the `strides` slice. Using a helper function for that to make
                    // sure the logic is consistent with the one used in the `Deck` implementation.
                    let (marks, stride_offset, strides) = unsafe {
                        let marks = slice_from_ptr(input.marks, input.n_marks as usize);
                        let stride_offset =
                            slice_from_ptr(input.stride_offset, input.n_marks as usize);
                        let strides =
                            slice_from_ptr(input.strides, calc_stride_count(marks, stride_offset));
                        (marks, stride_offset, strides)
                    };
                    let depth = marks.first().map(|m| m.depth + 1).unwrap_or(0);
                    let end = if depth == 0 {
                        input.n_items
                    } else {
                        input.n_marks
                    } as usize;
                    // Telescope the cursors to all depths up to the required arg_depth.
                    (0..stack_depth).scan(
                        ReadCursor {
                            n_items: input.n_items as usize,
                            marks,
                            strides,
                            stride_offset,
                            depth: arg_depth + max_delta,
                            start: 0,
                            end,
                        },
                        |prev, _d| {
                            let copy = prev.clone();
                            *prev = prev.child();
                            Some(copy)
                        },
                    )
                });
        let mut output_cursors = Vec::with_capacity(n_outputs * stack_depth);
        for arg_depth in output_depths {
            let depth = arg_depth + max_delta;
            output_cursors.push(WriteCursor {
                depth,
                next_depth: Some(depth),
            });
            for _ in 1..stack_depth {
                let last = output_cursors.len() - 1;
                // SAFETY: We unconditionally push an element outside the loop, so it cannot be empty.
                let child = unsafe { output_cursors.get_unchecked_mut(last) }.child();
                output_cursors.push(child);
            }
        }
        Ok(Combinations {
            input_cursors: input_cursors.collect(),
            input_depths: input_depths.into(),
            output_cursors: output_cursors.into_boxed_slice(),
            output_depths: output_depths.into(),
            stack_depth,
        })
    }

    pub fn advance(&mut self) -> bool {
        assert!(
            self.stack_depth > 0,
            "INTERNAL ERROR: Invalid state of combinations. Should never happen."
        );
        // First try to advance the inputs.
        if self
            .input_cursors
            .iter_mut()
            .skip(self.stack_depth - 1)
            .step_by(self.stack_depth)
            .fold(false, |any_advanced, c| {
                let advanced = c.advance();
                any_advanced || advanced
            })
        {
            // At least one input has advanced correctly.
            for w in self
                .output_cursors
                .iter_mut()
                .skip(self.stack_depth - 1)
                .step_by(self.stack_depth)
            {
                w.advance();
            }
            return true;
        }
        // None of the inputs advanced.
        enum State {
            Continue,
            Advanced,
            Exhausted,
        }
        let mut state = State::Continue;
        let mut stack_top = self.stack_depth - 1;
        loop {
            if stack_top == 0 {
                state = State::Exhausted;
                break;
            }
            // Unlike the C implementation, we don't need to close the writers
            // here. If everything went right, the drop implementation of the
            // DeckWriter took care of that already.
            stack_top -= 1;
            // Try to advance at this level of the stack.
            for i in self
                .input_cursors
                .iter_mut()
                .skip(stack_top)
                .step_by(self.stack_depth)
            {
                if i.advance() {
                    state = State::Advanced;
                }
            }
            match state {
                State::Continue => continue,
                State::Advanced => {
                    for w in self
                        .output_cursors
                        .iter_mut()
                        .skip(stack_top)
                        .step_by(self.stack_depth)
                    {
                        w.advance();
                    }
                    break;
                }
                State::Exhausted => break,
            }
        }
        match state {
            State::Continue => panic!(
                "INTERNAL ERROR: This should never happen. The above loop should not stop in this state."
            ),
            State::Advanced => {
                // Telescope the inputs to the stack_depth.
                for i in 0..self.input_depths.len() {
                    let stack = &mut self.input_cursors
                        [(i * self.stack_depth)..((i + 1) * self.stack_depth)];
                    for d in (stack_top + 1)..self.stack_depth {
                        stack[d] = stack[d - 1].child();
                    }
                }
                // Telescope the outputs to the stack_depth.
                for i in 0..self.output_depths.len() {
                    let stack = &mut self.output_cursors
                        [(i * self.stack_depth)..((i + 1) * self.stack_depth)];
                    for d in (stack_top + 1)..self.stack_depth {
                        stack[d] = stack[d - 1].child();
                    }
                }
                true
            }
            State::Exhausted => false,
        }
    }

    pub fn get_input<T: Default>(&self, items: &'a [T], index: usize) -> DeckView<'a, T> {
        let cursor = self.input_cursors[(index + 1) * self.stack_depth - 1].clone();
        DeckView { items, cursor }
    }

    pub fn get_output<'o, T: Default>(
        &mut self,
        deck: &'o mut Deck<T>,
        index: usize,
    ) -> DeckWriter<'o, T> {
        self.output_cursors[(index + 1) * self.stack_depth - 1].writer(deck)
    }
}

#[cfg(test)]
mod test {
    use crate::handle_from_deck;

    use super::*;

    fn binary_deck(depth: u8) -> Deck<usize> {
        let mut deck = Deck::<usize>::default();
        for i in 0usize..(1 << depth) {
            deck.push(i, (i.trailing_zeros() as u8).min(depth));
        }
        deck
    }

    fn mark_depths<T>(deck: &Deck<T>) -> Vec<u8>
    where
        T: Default,
    {
        deck.marks.iter().map(|m| m.depth).collect()
    }

    fn mark_positions<T>(deck: &Deck<T>) -> Vec<usize>
    where
        T: Default,
    {
        deck.marks.iter().map(|m| m.pos as usize).collect()
    }

    /// Collect depth-3 tree: all depth-2 groups, each with their depth-1 children.
    fn tree3(deck: &Deck<u32>) -> Vec<Vec<Vec<u32>>> {
        deck.view(3)
            .advance_iter()
            .flat_map(|v| v.child().advance_iter())
            .map(|v2| {
                v2.child()
                    .advance_iter()
                    .map(|v1| v1.as_slice().to_vec())
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Collect depth-2 tree: all depth-1 children across all depth-2 groups.
    fn tree2(deck: &Deck<u32>) -> Vec<Vec<u32>> {
        deck.view(2)
            .advance_iter()
            .flat_map(|v2| v2.child().advance_iter())
            .map(|v1| v1.as_slice().to_vec())
            .collect()
    }

    #[test]
    fn t_basic_ops() {
        let deck = binary_deck(5);
        assert_eq!(deck.len(), 1 << 5);
        assert_eq!(deck.max_depth(), 5);
        {
            // Iterate from level 5.
            let mut counter = 0;
            for v5 in deck.view(5).advance_iter() {
                assert_eq!(v5.depth(), 5);
                assert_eq!(v5.len(), 32);
                for v4 in v5.child().advance_iter() {
                    assert_eq!(v4.depth(), 4);
                    assert_eq!(v4.len(), 16);
                    for v3 in v4.child().advance_iter() {
                        assert_eq!(v3.depth(), 3);
                        assert_eq!(v3.len(), 8);
                        for v2 in v3.child().advance_iter() {
                            assert_eq!(v2.depth(), 2);
                            assert_eq!(v2.len(), 4);
                            for v1 in v2.child().advance_iter() {
                                assert_eq!(v1.depth(), 1);
                                // First iterate over v0 references.
                                for v0 in v1.child().advance_iter() {
                                    assert_eq!(v0.depth(), 0);
                                    assert_eq!(v0.len(), 1);
                                    assert_eq!(v0.as_ref(), &counter);
                                    counter += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        {
            // Iterate from level 4.
            let mut counter = 0;
            for v4 in deck.view(4).advance_iter() {
                assert_eq!(v4.depth(), 4);
                for v3 in v4.child().advance_iter() {
                    assert_eq!(v3.depth(), 3);
                    for v2 in v3.child().advance_iter() {
                        assert_eq!(v2.depth(), 2);
                        for v1 in v2.child().advance_iter() {
                            assert_eq!(v1.depth(), 1);
                            // First iterate over v0 references.
                            for v0 in v1.child().advance_iter() {
                                assert_eq!(v0.depth(), 0);
                                assert_eq!(v0.len(), 1);
                                assert_eq!(v0.as_ref(), &counter);
                                counter += 1;
                            }
                        }
                    }
                }
            }
        }
        {
            // Iterate from level 3.
            let mut counter = 0;
            for v3 in deck.view(3).advance_iter() {
                assert_eq!(v3.depth(), 3);
                for v2 in v3.child().advance_iter() {
                    assert_eq!(v2.depth(), 2);
                    for v1 in v2.child().advance_iter() {
                        assert_eq!(v1.depth(), 1);
                        // First iterate over v0 references.
                        for v0 in v1.child().advance_iter() {
                            assert_eq!(v0.depth(), 0);
                            assert_eq!(v0.len(), 1);
                            assert_eq!(v0.as_ref(), &counter);
                            counter += 1;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn t_deck_creation_macro() {
        // depth-1: flat list
        let d1 = deck![0usize, 1, 2, 3];
        assert_eq!(d1.len(), 4);
        assert_eq!(d1.max_depth(), 1);
        assert_eq!(d1.items, vec![0, 1, 2, 3]);
        assert_eq!(mark_depths(&d1), vec![0]);
        assert_eq!(mark_positions(&d1), vec![0]);
        // depth-2: list of lists
        let d2 = deck![[0usize, 1], [2, 3]];
        assert_eq!(d2.len(), 4);
        assert_eq!(d2.max_depth(), 2);
        assert_eq!(d2.items, vec![0, 1, 2, 3]);
        assert_eq!(mark_depths(&d2), vec![1, 0]);
        assert_eq!(mark_positions(&d2), vec![0, 2]);
        // depth-3: should match binary_deck(3)
        let d3 = deck![[[0usize, 1], [2, 3]], [[4, 5], [6, 7]]];
        let expected = binary_deck(3);
        assert_eq!(d3.items, expected.items);
        assert_eq!(mark_depths(&d3), mark_depths(&expected));
        assert_eq!(mark_positions(&d3), mark_positions(&expected));
        // depth-3 with 3-way branching at the top
        let d3b = deck![[[0usize, 1], [2, 3]], [[4, 5], [6, 7]], [[8, 9], [10, 11]]];
        assert_eq!(d3b.len(), 12);
        assert_eq!(d3b.max_depth(), 3);
        assert_eq!(mark_depths(&d3b), vec![2, 0, 1, 0, 1, 0]);
        assert_eq!(mark_positions(&d3b), vec![0, 2, 4, 6, 8, 10]);
        // single element
        let d_one = deck![42usize];
        assert_eq!(d_one.len(), 1);
        assert_eq!(d_one.max_depth(), 1);
        assert_eq!(mark_depths(&d_one), vec![0]);
        assert_eq!(mark_positions(&d_one), vec![0]);
    }

    #[test]
    fn t_deck_edge_cases() {
        // Empty deck.
        let empty = Deck::<usize>::default();
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.max_depth(), 0);
        assert_eq!(empty.view(0).advance_iter().count(), 0);
        // Single element at depth 0 (bare leaf).
        let mut d = Deck::default();
        d.push(42usize, 0);
        assert_eq!(d.len(), 1);
        assert_eq!(d.max_depth(), 0);
        assert!(d.marks.is_empty());
        let mut v = d.view(0).advance_iter();
        let item = v.next().unwrap();
        assert_eq!(item.as_ref(), &42);
        assert!(v.next().is_none());
        // Single element at depth 1 (one-element list).
        let mut d = Deck::default();
        d.push(7usize, 1);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(d.view(1).advance_iter().count(), 1);
        assert_eq!(d.view(1).advance_iter().next().unwrap().as_slice(), &[7]);
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
        assert_eq!(d.view(1).advance_iter().next().unwrap().as_slice(), &[1, 2]);
    }

    #[test]
    fn t_deck_flatten() {
        // Flatten a deep deck: all items survive, structure collapses to depth 1.
        let mut d = binary_deck(4);
        let items_before: Vec<usize> = d.items.clone();
        d.flatten();
        assert_eq!(d.items, items_before);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(d.marks.len(), 1);
        assert_eq!(d.marks[0].depth, 0);
        assert_eq!(d.marks[0].pos, 0);
        // Iterating at depth 1 yields all elements as one flat group.
        assert_eq!(d.view(1).advance_iter().count(), 1);
        assert_eq!(
            d.view(1).advance_iter().next().unwrap().as_slice(),
            items_before.as_slice()
        );
        // Flatten a single-element deck: no marks needed.
        let mut d = Deck::default();
        d.push(5usize, 0);
        d.flatten();
        assert_eq!(d.max_depth(), 0);
        assert!(d.marks.is_empty());
        // Flatten an already-flat deck is idempotent.
        let mut d = binary_deck(3);
        d.flatten();
        let marks_after_first: Vec<(u8, usize)> =
            d.marks.iter().map(|m| (m.depth, m.pos as usize)).collect();
        d.flatten();
        let marks_after_second: Vec<(u8, usize)> =
            d.marks.iter().map(|m| (m.depth, m.pos as usize)).collect();
        assert_eq!(marks_after_first, marks_after_second);
    }

    #[test]
    fn t_deck_graft() {
        // Graft increments every mark depth by 1.
        let mut d = binary_deck(3);
        d.graft();
        assert_eq!(d.max_depth(), 4);
        // Items are unchanged.
        assert_eq!(d.items, binary_deck(3).items);
        // Iteration still produces correct sequential values after graft:
        // The deck is now a depth-4 structure; iterating at depth 4 gives 2 groups.
        let mut counter = 0usize;
        for v3 in d.view(4).child().advance_iter() {
            assert_eq!(v3.depth(), 3);
            for v2 in v3.child().advance_iter() {
                assert_eq!(v2.depth(), 2);
                for v1 in v2.child().advance_iter() {
                    assert_eq!(v1.depth(), 1);
                    assert_eq!(v1.len(), 1);
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
        // A deck whose mark depths already use every level 0..=max is unchanged.
        let mut d = binary_deck(3);
        let depths_before = mark_depths(&d);
        d.simplify();
        assert_eq!(mark_depths(&d), depths_before);
        // A deck with gaps: only external depths 0, 2, 5 present.
        // External 5 → mark depth 4, external 2 → mark depth 1.
        // Mark depths present: {1, 4} → remapped to {0, 1}.
        let mut d = Deck::default();
        d.push(0usize, 5);
        d.push(1usize, 0);
        d.push(2usize, 2);
        d.push(3usize, 0);
        assert_eq!(mark_depths(&d), vec![4, 1]);
        d.simplify();
        assert_eq!(d.max_depth(), 2);
        assert_eq!(mark_depths(&d), vec![1, 0]);
        // Iteration still works correctly after remapping.
        assert_eq!(d.view(2).advance_iter().count(), 1);
        let mut counter = 0usize;
        for v1 in d.view(2).child().advance_iter() {
            for val in v1.as_slice() {
                assert_eq!(*val, counter);
                counter += 1;
            }
        }
        assert_eq!(counter, 4);
        // Simplify is idempotent.
        d.simplify();
        assert_eq!(mark_depths(&d), vec![1, 0]);
    }

    #[test]
    fn t_empty_lists() {
        // Depth-1 empty list: a list with no items.
        // Equivalent to: []
        let mut d = Deck::<usize>::default();
        d.start_new_arr(1);
        assert_eq!(d.len(), 0);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(d.marks.len(), 1);
        // Iterating at depth 1 yields one group with zero items.
        let groups: Vec<_> = d.view(1).advance_iter().collect();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].as_slice(), &[]);
        assert_eq!(groups[0].len(), 0);
        // Empty list between non-empty lists at depth 2.
        // Equivalent to: ((1, 2), (), (3, 4))
        let mut d = Deck::default();
        d.push(1usize, 2); // starts outer + first inner group
        d.push(2, 0); // continuation
        d.start_new_arr(1); // empty inner group
        d.push(3, 1); // third inner group
        d.push(4, 0); // continuation
        assert_eq!(d.len(), 4);
        assert_eq!(d.max_depth(), 2);
        assert_eq!(d.items, vec![1, 2, 3, 4]);
        assert_eq!(mark_positions(&d), vec![0, 2, 2]);
        // Iterating: one outer group containing 3 inner groups.
        let outer: Vec<_> = d.view(2).advance_iter().collect();
        assert_eq!(outer.len(), 1);
        let inner: Vec<_> = outer[0].child().advance_iter().collect();
        assert_eq!(inner.len(), 3);
        assert_eq!(inner[0].as_slice(), &[1, 2]);
        assert_eq!(inner[1].as_slice(), &[]); // the empty list
        assert_eq!(inner[1].len(), 0);
        assert_eq!(inner[2].as_slice(), &[3, 4]);
        // Empty list at the beginning.
        // Equivalent to: ((), (1, 2), (3, 4))
        let mut d = Deck::default();
        d.start_new_arr(2); // starts outer + empty first inner group
        d.push(1usize, 1); // second inner group
        d.push(2, 0);
        d.push(3, 1); // third inner group
        d.push(4, 0);
        assert_eq!(d.items, vec![1, 2, 3, 4]);
        let outer: Vec<_> = d.view(2).advance_iter().collect();
        assert_eq!(outer.len(), 1);
        let inner: Vec<_> = outer[0].child().advance_iter().collect();
        assert_eq!(inner.len(), 3);
        assert_eq!(inner[0].as_slice(), &[]);
        assert_eq!(inner[1].as_slice(), &[1, 2]);
        assert_eq!(inner[2].as_slice(), &[3, 4]);
        // Empty list at the end.
        // Equivalent to: ((1, 2), (3, 4), ())
        let mut d = Deck::default();
        d.push(1usize, 2);
        d.push(2, 0);
        d.push(3, 1);
        d.push(4, 0);
        d.start_new_arr(1); // empty last inner group
        assert_eq!(d.items, vec![1, 2, 3, 4]);
        let outer: Vec<_> = d.view(2).advance_iter().collect();
        assert_eq!(outer.len(), 1);
        let inner: Vec<_> = outer[0].child().advance_iter().collect();
        assert_eq!(inner.len(), 3);
        assert_eq!(inner[0].as_slice(), &[1, 2]);
        assert_eq!(inner[1].as_slice(), &[3, 4]);
        assert_eq!(inner[2].as_slice(), &[]);
        // Multiple consecutive empty lists.
        // Equivalent to: ((), (), ())
        let mut d = Deck::<usize>::default();
        d.start_new_arr(2);
        d.start_new_arr(1);
        d.start_new_arr(1);
        assert_eq!(d.len(), 0);
        assert_eq!(d.max_depth(), 2);
        let outer: Vec<_> = d.view(2).advance_iter().collect();
        assert_eq!(outer.len(), 1);
        let inner: Vec<_> = outer[0].child().advance_iter().collect();
        assert_eq!(inner.len(), 3);
        for group in &inner {
            assert_eq!(group.as_slice(), &[]);
        }
        // Depth-3 with nested empty lists.
        // Equivalent to: (((1, 2), ()), ((3, 4)))
        let mut d = Deck::default();
        d.push(1usize, 3); // depth-3 + depth-2 + depth-1 group start
        d.push(2, 0);
        d.start_new_arr(1); // empty inner-most group
        d.push(3, 2); // second depth-2 group
        d.push(4, 0);
        assert_eq!(d.max_depth(), 3);
        let top: Vec<_> = d.view(3).advance_iter().collect();
        assert_eq!(top.len(), 1);
        let mid: Vec<_> = top[0].child().advance_iter().collect();
        assert_eq!(mid.len(), 2);
        // First mid group: ((1, 2), ())
        let first_inner: Vec<_> = mid[0].child().advance_iter().collect();
        assert_eq!(first_inner.len(), 2);
        assert_eq!(first_inner[0].as_slice(), &[1, 2]);
        assert_eq!(first_inner[1].as_slice(), &[]);
        // Second mid group: ((3, 4))
        let second_inner: Vec<_> = mid[1].child().advance_iter().collect();
        assert_eq!(second_inner.len(), 1);
        assert_eq!(second_inner[0].as_slice(), &[3, 4]);
        // Flatten preserves items but drops empty structure.
        let mut d = Deck::default();
        d.push(1usize, 2);
        d.push(2, 0);
        d.start_new_arr(1);
        d.push(3, 1);
        d.push(4, 0);
        d.flatten();
        assert_eq!(d.items, vec![1, 2, 3, 4]);
        assert_eq!(d.max_depth(), 1);
        assert_eq!(
            d.view(1).advance_iter().next().unwrap().as_slice(),
            &[1, 2, 3, 4]
        );
    }

    #[test]
    fn t_empty_lists_macro() {
        // Empty group in the middle: [[1, 2], [], [3, 4]]
        let d = deck![[1usize, 2], [], [3, 4]];
        assert_eq!(d.items, vec![1, 2, 3, 4]);
        assert_eq!(d.max_depth(), 2);
        let outer: Vec<_> = d.view(2).advance_iter().collect();
        assert_eq!(outer.len(), 1);
        let inner: Vec<_> = outer[0].child().advance_iter().collect();
        assert_eq!(inner.len(), 3);
        assert_eq!(inner[0].as_slice(), &[1, 2]);
        assert_eq!(inner[1].as_slice(), &[]);
        assert_eq!(inner[2].as_slice(), &[3, 4]);
        // Empty group at the beginning: [[], [1, 2]]
        let d = deck![[], [1usize, 2]];
        assert_eq!(d.items, vec![1, 2]);
        assert_eq!(d.max_depth(), 2);
        let inner: Vec<_> = d
            .view(2)
            .advance_iter()
            .next()
            .unwrap()
            .child()
            .advance_iter()
            .collect();
        assert_eq!(inner.len(), 2);
        assert_eq!(inner[0].as_slice(), &[]);
        assert_eq!(inner[1].as_slice(), &[1, 2]);
        // Empty group at the end: [[1, 2], []]
        let d = deck![[1usize, 2], []];
        let inner: Vec<_> = d
            .view(2)
            .advance_iter()
            .next()
            .unwrap()
            .child()
            .advance_iter()
            .collect();
        assert_eq!(inner.len(), 2);
        assert_eq!(inner[0].as_slice(), &[1, 2]);
        assert_eq!(inner[1].as_slice(), &[]);
        // All empty: [[], [], []]
        let d: Deck<i32> = deck![[], [], []];
        assert_eq!(d.len(), 0);
        assert_eq!(d.max_depth(), 2);
        let inner: Vec<_> = d
            .view(2)
            .advance_iter()
            .next()
            .unwrap()
            .child()
            .advance_iter()
            .collect();
        assert_eq!(inner.len(), 3);
        for g in &inner {
            assert_eq!(g.as_slice(), &[] as &[i32]);
        }
        // Single empty group: [[]]
        let d: Deck<i32> = deck![[]];
        assert_eq!(d.len(), 0);
        assert_eq!(d.max_depth(), 2);
        let inner: Vec<_> = d
            .view(2)
            .advance_iter()
            .next()
            .unwrap()
            .child()
            .advance_iter()
            .collect();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].as_slice(), &[] as &[i32]);
        // Depth-3 with empty: [[[1, 2], []], [[3, 4]]]
        let d = deck![[[1usize, 2], []], [[3, 4]]];
        assert_eq!(d.max_depth(), 3);
        let mid: Vec<_> = d
            .view(3)
            .advance_iter()
            .next()
            .unwrap()
            .child()
            .advance_iter()
            .collect();
        assert_eq!(mid.len(), 2);
        let first: Vec<_> = mid[0].child().advance_iter().collect();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].as_slice(), &[1, 2]);
        assert_eq!(first[1].as_slice(), &[]);
        let second: Vec<_> = mid[1].child().advance_iter().collect();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].as_slice(), &[3, 4]);
        // Matches the manual construction from t_empty_lists.
        let macro_d = deck![[1usize, 2], [], [3, 4]];
        let mut manual_d = Deck::default();
        manual_d.push(1usize, 2);
        manual_d.push(2, 0);
        manual_d.start_new_arr(1);
        manual_d.push(3, 1);
        manual_d.push(4, 0);
        assert_eq!(macro_d.items, manual_d.items);
        assert_eq!(mark_depths(&macro_d), mark_depths(&manual_d));
        assert_eq!(mark_positions(&macro_d), mark_positions(&manual_d));
    }

    #[test]
    fn t_deck_writer_basic() {
        let mut deck: Deck<u32> = Deck::default();
        let mut counter = 0u32;
        {
            let mut dst3 = deck.writer(3);
            assert_eq!(dst3.depth(), 3);
            for _ in 0..3 {
                let mut dst2 = dst3.child();
                assert_eq!(dst2.depth(), 2);
                for _ in 0..3 {
                    let mut dst1 = dst2.child();
                    assert_eq!(dst1.depth(), 1);
                    for _ in 0..3 {
                        dst1.push(counter);
                        counter += 1;
                    }
                    // ViewMut::len, as_slice, Index, IndexMut
                    assert_eq!(dst1.len(), 3);
                    assert_eq!(dst1.as_slice(), &[counter - 3, counter - 2, counter - 1]);
                    assert_eq!(dst1[0], counter - 3);
                    assert_eq!(dst1[2], counter - 1);
                    dst1[1] = counter - 2 + 100;
                    dst1[1] = counter - 2; // restore
                }
            }
        }
        // Read and check.
        counter = 0;
        assert_eq!(
            tree3(&deck),
            vec![
                vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8]],
                vec![vec![9, 10, 11], vec![12, 13, 14], vec![15, 16, 17]],
                vec![vec![18, 19, 20], vec![21, 22, 23], vec![24, 25, 26]],
            ]
        );
        for v2 in deck
            .view(3)
            .advance_iter()
            .flat_map(|g| g.child().advance_iter())
        {
            // View::Index
            assert_eq!(v2[0], counter);
            for v1 in v2.child().advance_iter() {
                let arr = v1.as_slice();
                // View::Index
                assert_eq!(v1[0], counter);
                for val in arr {
                    assert_eq!(*val, counter);
                    counter += 1;
                }
            }
        }
    }

    #[test]
    fn t_deck_writer_with_extend() {
        let mut deck: Deck<u32> = Deck::default();
        let mut counter = 0u32;
        {
            let mut dst3 = deck.writer(3);
            assert_eq!(dst3.depth(), 3);
            for _ in 0..3 {
                let mut dst2 = dst3.child();
                assert_eq!(dst2.depth(), 2);
                for _ in 0..3 {
                    let mut dst1 = dst2.child();
                    assert_eq!(dst1.depth(), 1);
                    dst1.extend_from_slice(&[counter, counter + 1, counter + 2]);
                    counter += 3;
                    // ViewMut::len, as_slice_mut after extend
                    assert_eq!(dst1.len(), 3);
                    let s = dst1.as_slice_mut();
                    assert_eq!(s.len(), 3);
                    assert_eq!(s[0], counter - 3);
                }
            }
        }
        // Read and check.
        counter = 0;
        for v2 in deck
            .view(3)
            .advance_iter()
            .flat_map(|g| g.child().advance_iter())
        {
            for v1 in v2.child().advance_iter() {
                let arr = v1.as_slice();
                for val in arr {
                    assert_eq!(*val, counter);
                    counter += 1;
                }
            }
        }
    }

    #[test]
    fn t_unbalanced_tree() {
        // Asymmetric: ((1), (2, 3, 4), (5, 6))
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst = deck.writer(2);
            {
                let mut c = dst.child();
                c.push(1);
            }
            {
                let mut c = dst.child();
                c.push(2);
                c.push(3);
                c.push(4);
            }
            {
                let mut c = dst.child();
                c.push(5);
                c.push(6);
            }
        }
        assert_eq!(tree2(&deck), vec![vec![1], vec![2, 3, 4], vec![5, 6]]);
    }

    #[test]
    fn t_empty_groups_via_deck_writer() {
        // Build ((), (1, 2), (), (3), ()) via ViewMut
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst = deck.writer(2);
            {
                dst.child();
            } // empty
            {
                let mut c = dst.child();
                c.push(1);
                c.push(2);
            }
            {
                dst.child();
            } // empty
            {
                let mut c = dst.child();
                c.push(3);
            }
            {
                dst.child();
            } // empty
        }
        assert_eq!(
            tree2(&deck),
            vec![vec![], vec![1, 2], vec![], vec![3], vec![]]
        );
        assert_eq!(deck.items, vec![1, 2, 3]);
    }

    #[test]
    fn t_nested_empty_via_deck_writer() {
        // Build (((), (1, 2)), ((3,)), ((),)) via ViewMut — depth 3
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst3 = deck.writer(3);
            {
                let mut dst2 = dst3.child();
                {
                    dst2.child();
                } // empty inner
                {
                    let mut dst1 = dst2.child();
                    dst1.push(1);
                    dst1.push(2);
                }
            }
            {
                let mut dst2 = dst3.child();
                {
                    let mut dst1 = dst2.child();
                    dst1.push(3);
                }
            }
            {
                let mut dst2 = dst3.child();
                {
                    dst2.child();
                } // empty inner
            }
        }
        assert_eq!(
            tree3(&deck),
            vec![vec![vec![], vec![1, 2]], vec![vec![3]], vec![vec![]],]
        );
    }

    #[test]
    fn t_extend_empty_iterator() {
        // Extend with empty slice should create an empty group.
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst = deck.writer(2);
            {
                let mut c = dst.child();
                c.extend_from_slice(&[1, 2]);
            }
            {
                let mut c = dst.child();
                c.extend_from_slice(&[]); // empty extend
            }
            {
                let mut c = dst.child();
                c.extend(std::iter::empty::<u32>()); // empty Extend trait
            }
            {
                let mut c = dst.child();
                c.extend_from_slice(&[3]);
            }
        }
        assert_eq!(tree2(&deck), vec![vec![1, 2], vec![], vec![], vec![3]]);
    }

    #[test]
    fn t_single_element_deep() {
        // One item wrapped at depth 5: (((((42)))))
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut v5 = deck.writer(5);
            let mut v4 = v5.child();
            let mut v3 = v4.child();
            let mut v2 = v3.child();
            let mut v1 = v2.child();
            v1.push(42);
        }
        assert_eq!(deck.max_depth(), 5);
        assert_eq!(deck.len(), 1);
        // Unwrap all the way down via children().
        let leaf = deck
            .view(5)
            .advance_iter()
            .flat_map(|g| g.child().advance_iter())
            .flat_map(|g| g.child().advance_iter())
            .flat_map(|g| g.child().advance_iter())
            .next()
            .unwrap();
        assert_eq!(leaf.as_slice(), &[42]);
    }

    #[test]
    fn t_append_to_existing() {
        // Build some data manually, then append more via deck_writer.
        let mut deck: Deck<u32> = Deck::default();
        deck.push(1, 2);
        deck.push(2, 0);
        deck.push(3, 1);
        deck.push(4, 0);
        // deck is ((1,2),(3,4)) at depth 2
        assert_eq!(tree2(&deck), vec![vec![1, 2], vec![3, 4]]);
        // Append another group at depth 1 (same outer group).
        {
            let mut dst = deck.writer(1);
            dst.push(5);
            dst.push(6);
        }
        // Now: ((1,2),(3,4),(5,6))
        assert_eq!(tree2(&deck), vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
    }

    #[test]
    fn t_view_intermediate_depth() {
        // Build a depth-3 tree and iterate at depth 2 (flattened view).
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst3 = deck.writer(3);
            for i in 0..2u32 {
                let mut dst2 = dst3.child();
                for j in 0..2u32 {
                    let mut dst1 = dst2.child();
                    dst1.push(i * 4 + j * 2);
                    dst1.push(i * 4 + j * 2 + 1);
                }
            }
        }
        // depth-3 view: 2 groups of 2 groups of 2
        assert_eq!(
            tree3(&deck),
            vec![vec![vec![0, 1], vec![2, 3]], vec![vec![4, 5], vec![6, 7]]]
        );
        // depth-2 view: 4 groups of 2 (one level peeled off)
        assert_eq!(
            tree2(&deck),
            vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7]]
        );
        // depth-1 view: 4 groups of 2 (each mark starts a depth-1 group)
        let flat: Vec<_> = deck
            .view(1)
            .advance_iter()
            .map(|v| v.as_slice().to_vec())
            .collect();
        assert_eq!(flat, vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7]]);
    }

    #[test]
    fn t_graft_simplify_after_deck_writer() {
        // Build via ViewMut, then graft, then verify structure.
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst = deck.writer(2);
            {
                let mut c = dst.child();
                c.push(1);
                c.push(2);
            }
            {
                let mut c = dst.child();
                c.push(3);
            }
        }
        assert_eq!(tree2(&deck), vec![vec![1, 2], vec![3]]);
        // Graft: depth 2 → 3
        deck.graft();
        assert_eq!(deck.max_depth(), 3);
        assert_eq!(tree3(&deck), vec![vec![vec![1], vec![2]], vec![vec![3]]]);
        // Simplify should be a no-op (depths are already contiguous).
        let depths_before = mark_depths(&deck);
        deck.simplify();
        assert_eq!(mark_depths(&deck), depths_before);
        // Flatten.
        deck.flatten();
        assert_eq!(deck.max_depth(), 1);
        assert_eq!(deck.items, vec![1, 2, 3]);
    }

    #[test]
    fn t_all_empty_via_deck_writer() {
        // Depth-3 tree with no items at all: (((), ()), (()))
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst3 = deck.writer(3);
            {
                let mut dst2 = dst3.child();
                {
                    dst2.child();
                }
                {
                    dst2.child();
                }
            }
            {
                let mut dst2 = dst3.child();
                {
                    dst2.child();
                }
            }
        }
        assert_eq!(deck.len(), 0);
        assert_eq!(deck.max_depth(), 3);
        assert_eq!(tree3(&deck), vec![vec![vec![], vec![]], vec![vec![]]]);
    }

    #[test]
    fn t_deck_riter_len_at_each_level() {
        // Verify ViewMut::len reflects items added at each scope.
        let mut deck: Deck<u32> = Deck::default();
        {
            let mut dst3 = deck.writer(3);
            assert_eq!(dst3.len(), 0);
            {
                let mut dst2 = dst3.child();
                assert_eq!(dst2.len(), 0);
                {
                    let mut dst1 = dst2.child();
                    assert_eq!(dst1.len(), 0);
                    dst1.push(10);
                    assert_eq!(dst1.len(), 1);
                    dst1.push(20);
                    assert_eq!(dst1.len(), 2);
                }
                assert_eq!(dst2.len(), 2);
                {
                    let mut dst1 = dst2.child();
                    dst1.push(30);
                    assert_eq!(dst1.len(), 1);
                }
                assert_eq!(dst2.len(), 3);
            }
            assert_eq!(dst3.len(), 3);
            {
                let mut dst2 = dst3.child();
                assert_eq!(dst2.len(), 0);
                {
                    let mut dst1 = dst2.child();
                    dst1.push(40);
                }
                assert_eq!(dst2.len(), 1);
            }
            assert_eq!(dst3.len(), 4);
        }
        assert_eq!(deck.len(), 4);
    }

    #[test]
    fn t_deck_display() {
        let deck = binary_deck(5);
        assert_eq!(
            deck.to_string().trim(),
            "
     5 ──────────────┤ 0
                     ┤ 1
                 1 ──┤ 2
                     ┤ 3
              2 ─────┤ 4
                     ┤ 5
                 1 ──┤ 6
                     ┤ 7
           3 ────────┤ 8
                     ┤ 9
                 1 ──┤ 10
                     ┤ 11
              2 ─────┤ 12
                     ┤ 13
                 1 ──┤ 14
                     ┤ 15
        4 ───────────┤ 16
                     ┤ 17
                 1 ──┤ 18
                     ┤ 19
              2 ─────┤ 20
                     ┤ 21
                 1 ──┤ 22
                     ┤ 23
           3 ────────┤ 24
                     ┤ 25
                 1 ──┤ 26
                     ┤ 27
              2 ─────┤ 28
                     ┤ 29
                 1 ──┤ 30
                     ┤ 31
"
            .trim()
        );
        // Depth-2 with empty groups and multi-item groups.
        let deck: Deck<u32> = deck![[1, 2, 3], [], [4, 5, 6, 7], [], [8, 9, 10, 11], []];
        assert_eq!(
            deck.to_string().trim(),
            "
     2 ─────┤ 1
            ┤ 2
            ┤ 3
        1 ──┤
        1 ──┤ 4
            ┤ 5
            ┤ 6
            ┤ 7
        1 ──┤
        1 ──┤ 8
            ┤ 9
            ┤ 10
            ┤ 11
        1 ──┤
"
            .trim()
        );
        // Depth-1: flat list.
        let deck = deck![10u32, 20, 30];
        assert_eq!(
            deck.to_string().trim(),
            "
     1 ──┤ 10
         ┤ 20
         ┤ 30
"
            .trim()
        );
        // Single element.
        let deck = deck![42u32];
        assert_eq!(
            deck.to_string().trim(),
            "
  1 ──┤ 42
"
            .trim()
        );
        // All-empty depth-2.
        let deck: Deck<u32> = deck![[], [], []];
        assert_eq!(
            deck.to_string().trim(),
            "
     2 ─────┤
        1 ──┤
        1 ──┤
"
            .trim()
        );
        // Empty deck.
        let deck: Deck<u32> = Deck::default();
        assert_eq!(deck.to_string().trim(), "<empty_deck>");
        // Depth-3 with nested empty.
        let deck: Deck<u32> = deck![[[1, 2], []], [[]], [[3]]];
        println!("{}", deck);
        assert_eq!(
            deck.to_string().trim(),
            "
     3 ────────┤ 1
               ┤ 2
           1 ──┤
        2 ─────┤
        2 ─────┤ 3
"
            .trim()
        );
        // Single item, no marks.
        let deck: Deck<i32> = Deck::from_raw_data(&[42], &[]);
        println!("{}", deck);
        assert_eq!(deck.to_string().trim(), "┤ 42");
    }

    #[test]
    fn t_test_list_item_combinations() {
        {
            // Order 2 list, single index.
            let lists: Deck<f64> = deck![[3.1, 6.2, 9.42], [1.11, 2.21, 3.31], [4.45, 3.35, 2.25]];
            let indices: Deck<u64> = deck![2];
            let mut items: Deck<f64> = Deck::default();
            {
                let list_handle: OrcHandle = handle_from_deck(&lists, 0);
                let index_handle: OrcHandle = handle_from_deck(&indices, 1);
                let mut comb =
                    Combinations::from_handles(&[list_handle, index_handle], &[1, 0], &[0])
                        .expect("Failed to create combinations helper struct");
                // List processing iterations.
                loop {
                    let list_view = comb.get_input(&lists.items, 0);
                    let index_view = comb.get_input(&indices.items, 1);
                    let mut item_view = comb.get_output(&mut items, 0);
                    assert_eq!(list_view.depth(), 1);
                    assert_eq!(index_view.depth(), 0);
                    assert_eq!(item_view.depth(), 0);
                    let list = list_view.as_slice();
                    let index = *index_view.as_ref();
                    let item = item_view.push_default_mut();
                    {
                        // The do run of the block.
                        *item = list[index as usize];
                    }
                    if !comb.advance() {
                        break;
                    }
                }
            }
            // Check the outputs.
            // Input had 3 lists, so the output should have 3 items.
            assert_eq!(items.len(), 3);
            // The index is 2, so we should have item #2 from each list.
            assert_eq!(items.items(), &[9.42, 3.31, 2.25]);
        }
        {
            // Order 2 list with order 1 index.
            let lists: Deck<f64> = deck![[3.1, 6.2, 9.42], [1.11, 2.21, 3.31], [4.45, 3.35, 2.25]];
            let indices: Deck<u64> = deck![1, 2, 0];
            let mut items: Deck<f64> = Deck::default();
            {
                let list_handle: OrcHandle = handle_from_deck(&lists, 0);
                let index_handle: OrcHandle = handle_from_deck(&indices, 1);
                let mut comb =
                    Combinations::from_handles(&[list_handle, index_handle], &[1, 0], &[0])
                        .expect("Failed to create combinations helper struct");
                // List processing iterations.
                loop {
                    let list_view = comb.get_input(&lists.items, 0);
                    let index_view = comb.get_input(&indices.items, 1);
                    let mut item_view = comb.get_output(&mut items, 0);
                    assert_eq!(list_view.depth(), 1);
                    assert_eq!(index_view.depth(), 0);
                    assert_eq!(item_view.depth(), 0);
                    let list = list_view.as_slice();
                    let index = *index_view.as_ref();
                    let item = item_view.push_default_mut();
                    {
                        // The do run of the block.
                        *item = list[index as usize];
                    }
                    if !comb.advance() {
                        break;
                    }
                }
            }
            // Check the outputs.
            // Input had 3 lists, so the output should have 3 items.
            assert_eq!(items.len(), 3);
            // The index is 2, so we should have item #2 from each list.
            assert_eq!(items.items(), &[6.2, 3.31, 4.45]);
        }
        {
            // Order 2 list with order 1 index.
            let lists: Deck<f64> = deck![[3.1, 6.2, 9.42], [1.11, 2.21, 3.31], [4.45, 3.35, 2.25]];
            let indices: Deck<u64> = deck![[1, 2, 0], [1, 1, 1]];
            let mut items: Deck<f64> = Deck::default();
            {
                let list_handle: OrcHandle = handle_from_deck(&lists, 0);
                let index_handle: OrcHandle = handle_from_deck(&indices, 1);
                let mut comb =
                    Combinations::from_handles(&[list_handle, index_handle], &[1, 0], &[0])
                        .expect("Failed to create combinations helper struct");
                // List processing iterations.
                loop {
                    let list_view = comb.get_input(&lists.items, 0);
                    let index_view = comb.get_input(&indices.items, 1);
                    let mut item_view = comb.get_output(&mut items, 0);
                    assert_eq!(list_view.depth(), 1);
                    assert_eq!(index_view.depth(), 0);
                    assert_eq!(item_view.depth(), 0);
                    let list = list_view.as_slice();
                    let index = *index_view.as_ref();
                    let item = item_view.push_default_mut();
                    {
                        // The do run of the block.
                        *item = list[index as usize];
                    }
                    if !comb.advance() {
                        break;
                    }
                }
            }
            // Check the outputs.
            assert_eq!(items, deck![[6.2, 3.31, 4.45], [6.2, 2.21, 3.35]]);
        }
    }

    #[test]
    fn t_test_add_f64_combinations() {
        {
            /* Flat equal-length inputs: a and b each have 3 scalars. */
            let a: Deck<f64> = deck![1.0, 2.0, 3.0];
            let b: Deck<f64> = deck![10.0, 20.0, 30.0];
            let mut out: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[0, 0], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(a_view.depth(), 0);
                    assert_eq!(b_view.depth(), 0);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = *a_view.as_ref() + *b_view.as_ref();
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(out.len(), 3);
            assert_eq!(out.items(), &[11.0, 22.0, 33.0]);
        }
        {
            /* Flat inputs, broadcast-last: a has 4 scalars, b has 2. */
            let a: Deck<f64> = deck![1.0, 2.0, 3.0, 4.0];
            let b: Deck<f64> = deck![10.0, 20.0];
            let mut out: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[0, 0], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(a_view.depth(), 0);
                    assert_eq!(b_view.depth(), 0);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = *a_view.as_ref() + *b_view.as_ref();
                    if !comb.advance() {
                        break;
                    }
                }
            }
            // b is exhausted at 20.0 and stays there for the remaining elements of a.
            assert_eq!(out.len(), 4);
            assert_eq!(out.items(), &[11.0, 22.0, 23.0, 24.0]);
        }
        {
            /* Depth-2 inputs, equal groups: 2 inner groups of 2 items each. */
            let a: Deck<f64> = deck![[1.0, 2.0], [3.0, 4.0]];
            let b: Deck<f64> = deck![[10.0, 20.0], [30.0, 40.0]];
            let mut out: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[0, 0], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(a_view.depth(), 0);
                    assert_eq!(b_view.depth(), 0);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = *a_view.as_ref() + *b_view.as_ref();
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(out, deck![[11.0, 22.0], [33.0, 44.0]]);
        }
        {
            /* Depth-2 a, depth-3 b: b's single inner group broadcasts across a's two groups. */
            let a: Deck<f64> = deck![[1.0, 2.0], [3.0, 4.0]];
            let b: Deck<f64> = deck![[[10.0, 20.0]]];
            let mut out: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[0, 0], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(a_view.depth(), 0);
                    assert_eq!(b_view.depth(), 0);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = *a_view.as_ref() + *b_view.as_ref();
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(out, deck![[[11.0, 22.0], [13.0, 24.0]]]);
        }
    }

    #[test]
    fn t_test_list_length_combinations() {
        {
            /* Depth-2 input: 5 lists, some empty. */
            let input: Deck<f64> = deck![[1.0, 2.0, 3.0], [], [4.0], [], [5.0, 6.0]];
            let mut out: Deck<u64> = Deck::default();
            {
                let in_handle: OrcHandle = handle_from_deck(&input, 0);
                let mut comb = Combinations::from_handles(&[in_handle], &[1], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let list_view = comb.get_input(&input.items, 0);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(list_view.depth(), 1);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = list_view.len() as u64;
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(out.len(), 5);
            assert_eq!(out.items(), &[3u64, 0, 1, 0, 2]);
        }
        {
            /* Depth-3 input: two outer groups each containing lists, with empty lists inside. */
            let input: Deck<f64> = deck![[[1.0, 2.0], []], [[3.0, 4.0, 5.0], [], [6.0]]];
            let mut out: Deck<u64> = Deck::default();
            {
                let in_handle: OrcHandle = handle_from_deck(&input, 0);
                let mut comb = Combinations::from_handles(&[in_handle], &[1], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let list_view = comb.get_input(&input.items, 0);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(list_view.depth(), 1);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = list_view.len() as u64;
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(out, deck![[2u64, 0], [3u64, 0, 1]]);
        }
    }

    #[test]
    fn t_test_two_output_combinations() {
        {
            /* One input, two outputs: square and cube of 3 scalars. */
            let input: Deck<f64> = deck![2.0, 3.0, 4.0];
            let mut sq: Deck<f64> = Deck::default();
            let mut cb: Deck<f64> = Deck::default();
            {
                let in_handle: OrcHandle = handle_from_deck(&input, 0);
                let mut comb = Combinations::from_handles(&[in_handle], &[0], &[0, 0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let in_view = comb.get_input(&input.items, 0);
                    let mut sq_view = comb.get_output(&mut sq, 0);
                    let mut cb_view = comb.get_output(&mut cb, 1);
                    assert_eq!(in_view.depth(), 0);
                    assert_eq!(sq_view.depth(), 0);
                    assert_eq!(cb_view.depth(), 0);
                    let x = *in_view.as_ref();
                    let sq_item = sq_view.push_default_mut();
                    let cb_item = cb_view.push_default_mut();
                    *sq_item = x * x;
                    *cb_item = x * x * x;
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(sq.len(), 3);
            assert_eq!(cb.len(), 3);
            assert_eq!(sq.items(), &[4.0, 9.0, 16.0]);
            assert_eq!(cb.items(), &[8.0, 27.0, 64.0]);
        }
        {
            /* Two inputs, two outputs: sum and product of 3 scalars each. */
            let a: Deck<f64> = deck![1.0, 2.0, 3.0];
            let b: Deck<f64> = deck![4.0, 5.0, 6.0];
            let mut sum: Deck<f64> = Deck::default();
            let mut prod: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[0, 0], &[0, 0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut sum_view = comb.get_output(&mut sum, 0);
                    let mut prod_view = comb.get_output(&mut prod, 1);
                    assert_eq!(a_view.depth(), 0);
                    assert_eq!(b_view.depth(), 0);
                    assert_eq!(sum_view.depth(), 0);
                    assert_eq!(prod_view.depth(), 0);
                    let (av, bv) = (*a_view.as_ref(), *b_view.as_ref());
                    let s = sum_view.push_default_mut();
                    let p = prod_view.push_default_mut();
                    *s = av + bv;
                    *p = av * bv;
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(sum.len(), 3);
            assert_eq!(prod.len(), 3);
            assert_eq!(sum.items(), &[5.0, 7.0, 9.0]);
            assert_eq!(prod.items(), &[4.0, 10.0, 18.0]);
        }
    }

    #[test]
    fn t_test_first_add_combinations() {
        {
            /* Equal-length: a and b each have 3 depth-1 groups. */
            let a: Deck<f64> = deck![[1.0, 99.0], [2.0, 99.0], [3.0, 99.0]];
            let b: Deck<f64> = deck![[10.0, 99.0], [20.0, 99.0], [30.0, 99.0]];
            let mut out: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[1, 1], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(a_view.depth(), 1);
                    assert_eq!(b_view.depth(), 1);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = *a_view.as_ref() + *b_view.as_ref();
                    if !comb.advance() {
                        break;
                    }
                }
            }
            assert_eq!(out.len(), 3);
            assert_eq!(out.items(), &[11.0, 22.0, 33.0]);
        }
        {
            /* Broadcast-last at group level: a has 4 groups, b has 2. */
            let a: Deck<f64> = deck![[1.0, 99.0], [2.0, 99.0], [3.0, 99.0], [4.0, 99.0]];
            let b: Deck<f64> = deck![[10.0, 99.0], [20.0, 99.0]];
            let mut out: Deck<f64> = Deck::default();
            {
                let a_handle: OrcHandle = handle_from_deck(&a, 0);
                let b_handle: OrcHandle = handle_from_deck(&b, 1);
                let mut comb = Combinations::from_handles(&[a_handle, b_handle], &[1, 1], &[0])
                    .expect("Failed to create combinations helper struct");
                loop {
                    let a_view = comb.get_input(&a.items, 0);
                    let b_view = comb.get_input(&b.items, 1);
                    let mut out_view = comb.get_output(&mut out, 0);
                    assert_eq!(a_view.depth(), 1);
                    assert_eq!(b_view.depth(), 1);
                    assert_eq!(out_view.depth(), 0);
                    let item = out_view.push_default_mut();
                    *item = *a_view.as_ref() + *b_view.as_ref();
                    if !comb.advance() {
                        break;
                    }
                }
            }
            // b is exhausted after its second group and stays there for the remaining groups of a.
            assert_eq!(out.len(), 4);
            assert_eq!(out.items(), &[11.0, 22.0, 23.0, 24.0]);
        }
    }
}
