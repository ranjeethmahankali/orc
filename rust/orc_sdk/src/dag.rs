use std::{
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    ops::{Deref, DerefMut, Index, IndexMut},
    rc::{Rc, Weak},
};

use crate::FuncInfo;

/// Declarative macro for composing a DAG of plugin function calls.
///
/// Takes a PluginSet, an `&AtomicU64` handle counter, and a block of statements.
/// The counter is used to assign unique handle IDs to all output handles.
/// Supports nesting single-output calls: `(add (mul a b) c)`.
///
/// ```ignore
/// use std::sync::atomic::AtomicU64;
/// let hc = AtomicU64::new(0);
/// orc_dag!(plugin_set, &hc, {
///     (let tmp (mul a b))
///     (let result (add tmp c))
///     (let (mean variance) (compute_stats data))
///     (add (mul a b) c)
/// })
/// ```
#[macro_export]
macro_rules! orc_dag {
    // Entry point: takes a PluginSet, an &AtomicU64 handle counter, and a block of statements.
    ($ps:expr, $hc:expr, { $($body:tt)* }) => {
        (|| -> Result<_, $crate::Error> {
            #[allow(unused_variables)]
            let ps_ref_ = &$ps;
            #[allow(unused_variables)]
            let hc_ref_: &std::sync::atomic::AtomicU64 = $hc;
            orc_dag!(@stmts ps_ref_, hc_ref_, $($body)*)
        })()
    };

    // --- Statements ---

    // let single output, more statements follow
    (@stmts $ps:ident, $hc:ident, (let $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let $name = orc_dag!(@call1 $ps, $hc, $func, $($arg)*)?;
        orc_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // let multiple outputs, more statements follow
    (@stmts $ps:ident, $hc:ident, (let ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let inputs_ = [$(orc_dag!(@expr $ps, $hc, $arg)),*];
        let func_ = $ps.get_function(stringify!($func))
            .ok_or($crate::Error::InvalidFunction)?
            .func
            .ok_or($crate::Error::NullPointer)?;
        const N_OUTS_: u64 = orc_dag!(@count $($name)+) as u64;
        let base_handle_ = $hc.fetch_add(N_OUTS_, std::sync::atomic::Ordering::Relaxed);
        let mut outs_: [OrcHandle; N_OUTS_ as usize] = std::array::from_fn(|i| {
            let mut h = OrcHandle::default();
            h.handle = base_handle_ + i as u64;
            h
        });
        unsafe {
            (func_)(0, inputs_.as_ptr(), inputs_.len() as u64, outs_.as_mut_ptr(), N_OUTS_);
        }
        let mut idx_ = 0usize;
        $(
            let $name = std::mem::take(&mut outs_[idx_]);
            idx_ += 1;
        )+
        let _ = idx_;
        orc_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // Trailing expression — bare function call as the block's return value
    (@stmts $ps:ident, $hc:ident, ($func:ident $($arg:tt)*)) => {
        orc_dag!(@call1 $ps, $hc, $func, $($arg)*)
    };

    // Empty — end of statements
    (@stmts $ps:ident, $hc:ident,) => { Ok::<(), $crate::Error>(()) };

    // --- Expression: either a variable reference or a nested function call ---
    // Nested call: (func args...)
    (@expr $ps:ident, $hc:ident, ($func:ident $($arg:tt)*)) => {
        orc_dag!(@call1 $ps, $hc, $func, $($arg)*)?
    };
    // Variable reference
    (@expr $ps:ident, $hc:ident, $var:ident) => {
        unsafe { $var.non_owning_clone() }
    };

    // --- Single-output function call ---
    (@call1 $ps:ident, $hc:ident, $func:ident, $($arg:tt)*) => {{
        let inputs_ = [$(orc_dag!(@expr $ps, $hc, $arg)),*];
        let mut out_: OrcHandle = OrcHandle::default();
        out_.handle = $hc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let func_ = $ps.get_function(stringify!($func))
            .ok_or($crate::Error::InvalidFunction)?
            .func
            .ok_or($crate::Error::NullPointer)?;
        unsafe {
            (func_)(0, inputs_.as_ptr(), inputs_.len() as u64, &mut out_, 1);
        }
        Ok::<OrcHandle, $crate::Error>(out_)
    }};


    // --- Counting ---
    (@count $x:ident) => { 1usize };
    (@count $x:ident $($rest:ident)+) => { 1usize + orc_dag!(@count $($rest)+) };

    // --- Error messages for wrong usage ---
    ($ps:expr, { $($body:tt)* }) => {
        compile_error!("orc_dag! requires 3 arguments: orc_dag!(plugin_set, handle_counter, { ... })")
    };
    ($ps:expr) => {
        compile_error!("orc_dag! requires 3 arguments: orc_dag!(plugin_set, handle_counter, { ... })")
    };
}

/// All elements of the graph implement this trait. They are identified by their
/// index.
pub trait Handle: From<usize> + Copy + Clone + 'static {
    /// The index of the element.
    fn index(&self) -> usize;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct IH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct OH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct LH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct NH {
    idx: usize,
}

impl From<usize> for IH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for IH {
    fn index(&self) -> usize {
        self.idx
    }
}

impl From<usize> for OH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for OH {
    fn index(&self) -> usize {
        self.idx
    }
}

impl From<usize> for LH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for LH {
    fn index(&self) -> usize {
        self.idx
    }
}

impl From<usize> for NH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for NH {
    fn index(&self) -> usize {
        self.idx
    }
}

#[derive(Default)]
pub(crate) struct PropertyContainer<H>
where
    H: Handle,
{
    props: Vec<Box<dyn GenericProperty<H>>>,
    length: usize,
    _phantom: PhantomData<H>,
}

impl<H> PropertyContainer<H>
where
    H: Handle,
{
    fn push_property(&mut self, prop: Box<dyn GenericProperty<H>>) {
        self.props.push(prop);
    }

    /**
     * Reserve memory to accomodate an additional `n` elements.
     */
    pub fn reserve(&mut self, n: usize) -> Result<(), DagError> {
        for prop in self.props.iter_mut() {
            prop.reserve(n)?;
        }
        Ok(())
    }

    pub fn resize(&mut self, n: usize) -> Result<(), DagError> {
        for prop in self.props.iter_mut() {
            prop.resize(n)?;
        }
        self.length = n;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), DagError> {
        for prop in self.props.iter_mut() {
            prop.clear()?;
        }
        self.length = 0;
        Ok(())
    }

    pub fn push_value(&mut self) -> Result<(), DagError> {
        let (count, err) = self
            .props
            .iter_mut()
            .fold((0usize, Ok(())), |(count, err), prop| match err {
                Ok(()) => match prop.push() {
                    Ok(()) => (count + 1, Ok(())),
                    Err(e) => (count, Err(e)),
                },
                Err(e) => (count, Err(e)),
            });
        // If something went wrong, go back to how things were.
        if err.is_err() {
            for prop in self.props.iter_mut().take(count) {
                prop.resize(self.length)?;
            }
            return err;
        }
        self.length += 1;
        Ok(())
    }

    pub fn swap(&mut self, i: usize, j: usize) -> Result<(), DagError> {
        for prop in self.props.iter_mut() {
            prop.swap(i, j)?;
        }
        Ok(())
    }

    pub fn copy(&mut self, src: H, dst: H) -> Result<(), DagError> {
        for prop in self.props.iter_mut() {
            prop.copy(src.index() as usize, dst.index() as usize)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn garbage_collection(&mut self) {
        self.props.retain(|prop| prop.is_valid())
    }
}

trait GenericProperty<H>
where
    H: Handle,
{
    fn reserve(&mut self, n: usize) -> Result<(), DagError>;

    fn resize(&mut self, n: usize) -> Result<(), DagError>;

    fn clear(&mut self) -> Result<(), DagError>;

    fn push(&mut self) -> Result<(), DagError>;

    fn swap(&mut self, i: usize, j: usize) -> Result<(), DagError>;

    fn copy(&mut self, src: usize, dst: usize) -> Result<(), DagError>;

    fn is_valid(&self) -> bool;
}

/// Buffer containing the property values.
///
/// This is meant to be a thin wrapper around `T` that allows for convenient and type safe indexing
/// with the handle type `H`. If you need a raw slice, you can always convert the property buffer
/// into a `&[T]` at zero cost.
///
/// To access this buffer from the property that owns it, you have it borrow it as either
/// [`Ref`](std::cell::Ref) or [`RefMut`](std::cell::RefMut)
pub struct PropBuf<H, T>
where
    H: Handle,
    T: Clone,
{
    buf: Vec<T>,
    _phantom: PhantomData<H>,
}

/// Input property. A value of type `T` is defined on each input of the graph.
///
/// See the documentation of [`Property<H, T>`] for more context on how
/// properties work.
pub type InputProperty<T> = Property<IH, T>;

/// Halfedge property. A value of type `T` is defined on each output of the graph.
///
/// See the documentation of [`Property<H, T>`] for more context on how
/// properties work.
pub type OutputProperty<T> = Property<OH, T>;

/// Edge property. A value of type `T` is defined on each link of the graph.
///
/// See the documentation of [`Property<H, T>`] for more context on how
/// properties work.
pub type LinkProperty<T> = Property<LH, T>;

/// Face property. A value of type `T` is defined on each node of the graph.
///
/// See the documentation of [`Property<H, T>`] for more context on how
/// properties work.
pub type NodeProperty<T> = Property<NH, T>;

/// Buffer containing the values of a input property.
///
/// To access this buffer from a property, you have to borrow it from the
/// property as either [`Ref`](std::cell::Ref) or [`RefMut`](std::cell::RefMut).
pub type InputPropBuf<T> = PropBuf<IH, T>;

/// Buffer containing the values of a output property.
///
/// To access this buffer from a property, you have to borrow it from the
/// property as either [`Ref`](std::cell::Ref) or [`RefMut`](std::cell::RefMut).
pub type OutputPropBuf<T> = PropBuf<OH, T>;

/// Buffer containing the values of a link property.
///
/// To access this buffer from a property, you have to borrow it from the
/// property as either [`Ref`](std::cell::Ref) or [`RefMut`](std::cell::RefMut).
pub type LinkPropBuf<T> = PropBuf<LH, T>;

/// Buffer containing the values of a node property.
///
/// To access this buffer from a property, you have to borrow it from the
/// property as either [`Ref`](std::cell::Ref) or [`RefMut`](std::cell::RefMut).
pub type NodePropBuf<T> = PropBuf<NH, T>;

/// This is what lives inside the property container. It doesn't control the
/// lifetime of the property, but is able to resize and swap elements of the
/// property buffer by borrowing whenever topological edits are made to the
/// mesh.
struct WeakProperty<H, T>
where
    H: Handle,
    T: Clone,
{
    data: Weak<RefCell<PropBuf<H, T>>>,
    default: T,
}

/// The element handle can be used to index into the property buffer.
impl<H, T> Index<H> for PropBuf<H, T>
where
    H: Handle,
    T: Clone,
{
    type Output = T;

    fn index(&self, handle: H) -> &Self::Output {
        &self.buf[handle.index() as usize]
    }
}

/// The element handle can be used to index into the property buffer.
impl<H, T> IndexMut<H> for PropBuf<H, T>
where
    H: Handle,
    T: Clone + 'static,
{
    /// Get the mutable reference to the property of the element corresponding
    /// to handle `h`.
    fn index_mut(&mut self, h: H) -> &mut Self::Output {
        &mut self.buf[h.index() as usize]
    }
}

/// A property buffer can be turned into a `&[T]` for conveninence.
impl<H, T> Deref for PropBuf<H, T>
where
    H: Handle,
    T: Clone,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl<H, T> GenericProperty<H> for WeakProperty<H, T>
where
    T: Clone,
    H: Handle,
{
    /**
     * Reserve memory for an additional `n` values.
     */
    fn reserve(&mut self, n: usize) -> Result<(), DagError> {
        if let Some(prop) = self.data.upgrade() {
            prop.try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?
                .buf
                .reserve(n);
        }
        Ok(())
    }

    fn resize(&mut self, n: usize) -> Result<(), DagError> {
        if let Some(prop) = self.data.upgrade() {
            prop.try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?
                .buf
                .resize(n, self.default.clone());
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DagError> {
        if let Some(prop) = self.data.upgrade() {
            prop.try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?
                .buf
                .clear();
        }
        Ok(())
    }

    fn push(&mut self) -> Result<(), DagError> {
        if let Some(prop) = self.data.upgrade() {
            prop.try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?
                .buf
                .push(self.default.clone());
        }
        Ok(())
    }

    fn swap(&mut self, i: usize, j: usize) -> Result<(), DagError> {
        if let Some(prop) = self.data.upgrade() {
            prop.try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?
                .swap(i, j);
        }
        Ok(())
    }

    fn copy(&mut self, src: usize, dst: usize) -> Result<(), DagError> {
        if let Some(prop) = self.data.upgrade() {
            let mut buf = prop
                .try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?;
            let buf: &mut [T] = &mut buf;
            buf[dst] = buf[src].clone();
        }
        Ok(())
    }

    fn is_valid(&self) -> bool {
        self.data.upgrade().is_some()
    }
}

/// This represents a property defined on the elements of the mesh. `T` is the
/// type of data associated with each element of the mesh, whose handle type is
/// `H`.
///
/// Why use properties instead of simple [`Vec<T>`] to associate values with
/// elements of a mesh? Say you use a simple [`Vec<T>`] to keep track of
/// properties. If you modify the mesh, by either adding new elements (vertices,
/// faces, etc.), or by deleting elements and garbage collecting. The [`Vec<T>`]
/// will go out of sync with the mesh. Instead using a [`Property<H, T>`]
/// guarantees the properties are always synchronized with the mesh, and that
/// every element of the mesh of type `H`, even the newly added ones will have a
/// value associated with it.
#[derive(Clone)]
pub struct Property<H, T>
where
    H: Handle,
    T: Clone,
{
    data: Rc<RefCell<PropBuf<H, T>>>,
    default: T,
}

impl<H, T> Property<H, T>
where
    H: Handle,
    T: Clone + 'static,
{
    pub(crate) fn new(container: &mut PropertyContainer<H>, default: T) -> Self {
        let prop = Property {
            data: Rc::new(RefCell::new(PropBuf {
                buf: vec![default.clone(); container.len()],
                _phantom: PhantomData,
            })),
            default,
        };
        container.push_property(prop.generic_ref());
        prop
    }

    pub(crate) fn with_capacity(
        n: usize,
        container: &mut PropertyContainer<H>,
        default: T,
    ) -> Self {
        let mut buf = Vec::with_capacity(n);
        buf.resize(container.len(), default.clone());
        let prop = Property {
            data: Rc::new(RefCell::new(PropBuf {
                buf,
                _phantom: PhantomData,
            })),
            default,
        };
        container.push_property(prop.generic_ref());
        prop
    }

    fn generic_ref(&self) -> Box<dyn GenericProperty<H>> {
        Box::new(WeakProperty::<H, T> {
            data: Rc::downgrade(&self.data),
            default: self.default.clone(),
        })
    }

    /// Try to borrow the property with read-only access.
    ///
    /// Properties use interior mutability pattern using a [`RefCell<T>`] to
    /// enforce runtime borrow checking rules. If borrowing fails,
    /// [`Error::BorrowedPropertyAccess`] is returned, otherwise a reference to
    /// the property is returned.
    pub fn try_borrow(&'_ self) -> Result<Ref<'_, PropBuf<H, T>>, DagError> {
        self.data
            .try_borrow()
            .map_err(|_| DagError::BorrowedPropertyAccess)
    }

    /// Try to borrow the property with mutable access.
    ///
    /// Properties use interior mutability pattern using a [`RefCell<T>`] to
    /// enforce runtime borrow checking rules. If borrowing fails,
    /// [`DagError::BorrowedPropertyAccess`] is returned, otherwise a mutable
    /// reference to the property is returned.
    pub fn try_borrow_mut(&'_ mut self) -> Result<RefMut<'_, PropBuf<H, T>>, DagError> {
        self.data
            .try_borrow_mut()
            .map_err(|_| DagError::BorrowedPropertyAccess)
    }

    /// Get a reference to the property value of the mesh element `h`.
    ///
    /// This function internally tries to borrow the property and returns an
    /// error if borrowing fails.
    pub fn get(&'_ self, h: H) -> Result<Ref<'_, T>, DagError> {
        Ok(Ref::map(
            self.data
                .try_borrow()
                .map_err(|_| DagError::BorrowedPropertyAccess)?,
            |v| &v.buf[h.index() as usize],
        ))
    }

    /// Get the cloned property value of the mesh element `h`.
    ///
    /// The function internally tries to borrow the property and returns an
    /// error if borrowing fails.
    pub fn get_cloned(&self, h: H) -> Result<T, DagError> {
        let buf = self.try_borrow()?;
        Ok(buf[h].clone())
    }

    /// Get a mutable reference to the property value of a mesh element.
    ///
    /// This function internally tries to mutably borrow the property and
    /// returns an error if borrowing fails.
    pub fn get_mut(&'_ mut self, h: H) -> Result<RefMut<'_, T>, DagError> {
        Ok(RefMut::map(
            self.data
                .try_borrow_mut()
                .map_err(|_| DagError::BorrowedPropertyAccess)?,
            |v| &mut v.buf[h.index() as usize],
        ))
    }

    /// Set the property value of a mesh element.
    ///
    /// This function internally tries to mutably borrow the property and
    /// returns an error if borrowing fails.
    pub fn set(&mut self, h: H, val: T) -> Result<(), DagError> {
        (*self.get_mut(h)?) = val;
        Ok(())
    }
}

/// A mutable property buffer can be turned into a `&mut [T]` for conveninence.
impl<H, T> DerefMut for PropBuf<H, T>
where
    H: Handle,
    T: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buf
    }
}

#[derive(Clone)]
struct Input {
    node: NH,
    link: Option<LH>,
    prev: Option<IH>,
    next: Option<IH>,
    deleted: bool,
}

#[derive(Clone)]
struct Output {
    node: NH,
    link: Option<LH>,
    prev: Option<OH>,
    next: Option<OH>,
    deleted: bool,
}

#[derive(Clone)]
struct Link {
    start: OH,
    end: IH,
    prev: Option<LH>,
    next: Option<LH>,
    deleted: bool,
}

#[derive(Clone)]
struct Node {
    input: Option<IH>,
    output: Option<OH>,
    deleted: bool,
}

#[derive(Debug)]
pub enum DagError {
    BorrowedPropertyAccess,
    MismatchedArrayLengths(usize, usize),
    CycleDetected,
}

#[derive(Default)]
struct Graph {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    links: Vec<Link>,
    nodes: Vec<Node>,
    // Property containers.
    node_props: PropertyContainer<NH>,
    link_props: PropertyContainer<LH>,
    input_props: PropertyContainer<IH>,
    output_props: PropertyContainer<OH>,
}

impl Graph {
    pub fn with_capacity(
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Self {
        Graph {
            inputs: Vec::with_capacity(n_inputs),
            outputs: Vec::with_capacity(n_outputs),
            links: Vec::with_capacity(n_links),
            nodes: Vec::with_capacity(n_nodes),
            node_props: Default::default(),
            link_props: Default::default(),
            input_props: Default::default(),
            output_props: Default::default(),
        }
    }

    pub fn push_node(
        &mut self,
        input_handles: &mut [IH],
        output_handles: &mut [OH],
    ) -> Result<NH, DagError> {
        let nh = NH {
            idx: self.nodes.len(),
        };
        // Push the input pins.
        for input_handle in input_handles.iter_mut() {
            input_handle.idx = self.inputs.len();
            self.push_input_impl(Input {
                node: nh,
                link: None,
                prev: None,
                next: None,
                deleted: false,
            })?;
        }
        // Link the consecutive input pins together.
        for [prev, next] in input_handles.array_windows::<2>() {
            self.link_consecutive_inputs(*prev, *next);
        }
        // Push output pins.
        for output_handle in output_handles.iter_mut() {
            output_handle.idx = self.outputs.len();
            self.push_output_impl(Output {
                node: nh,
                link: None,
                prev: None,
                next: None,
                deleted: false,
            })?;
        }
        // Link the consecutive output pins together.
        for [prev, next] in output_handles.array_windows::<2>() {
            self.link_consecutive_outputs(*prev, *next);
        }
        self.push_node_impl(Node {
            input: input_handles.first().copied(),
            output: output_handles.first().copied(),
            deleted: false,
        })
        .map(|()| nh)
    }

    fn push_input_impl(&mut self, input: Input) -> Result<(), DagError> {
        self.inputs.push(input);
        self.input_props.push_value()
    }

    fn push_output_impl(&mut self, output: Output) -> Result<(), DagError> {
        self.outputs.push(output);
        self.output_props.push_value()
    }

    fn push_node_impl(&mut self, node: Node) -> Result<(), DagError> {
        self.nodes.push(node);
        self.node_props.push_value()
    }

    pub fn push_link(&mut self, from: OH, to: IH) -> Result<LH, DagError> {
        let mut last: Option<LH> = None;
        {
            let mut tail = &self.outputs[from.idx].link;
            debug_assert!(
                match tail {
                    Some(li) => self.links[li.idx].prev == None,
                    None => true,
                },
                "The first link should not have a previous link. The topology is broken."
            );
            while let Some(li) = *tail {
                last = Some(li);
                tail = &self.links[li.idx].next;
            }
        }
        // Push the link first so it exists in the vec before we reference it.
        let lh = LH {
            idx: self.links.len(),
        };
        self.push_link_impl(Link {
            start: from,
            end: to,
            prev: last,
            next: None,
            deleted: false,
        })?;
        // Connect the link to the input. Inputs only have one incoming link.
        self.disconnect_input(to);
        self.inputs[to.idx].link = Some(lh);
        // Append to the output's linked list.
        match last {
            Some(p) => self.links[p.idx].next = Some(lh),
            None => self.outputs[from.idx].link = Some(lh),
        }
        Ok(lh)
    }

    fn disconnect_input(&mut self, input: IH) {
        if let Some(old) = std::mem::replace(&mut self.inputs[input.idx].link, None) {
            self.links[old.idx].deleted = true;
            // Remove references to the deleted link.
            let prev = std::mem::replace(&mut self.links[old.idx].prev, None);
            let next = std::mem::replace(&mut self.links[old.idx].next, None);
            let src = self.links[old.idx].start;
            if self.outputs[src.idx].link == Some(old) {
                self.outputs[src.idx].link = next;
            }
            if let Some(prev) = prev {
                self.links[prev.idx].next = next;
            }
            if let Some(next) = next {
                self.links[next.idx].prev = prev;
            }
        }
    }

    fn push_link_impl(&mut self, link: Link) -> Result<(), DagError> {
        self.links.push(link);
        self.link_props.push_value()
    }

    pub fn create_input_property<T>(&mut self, default: T) -> InputProperty<T>
    where
        T: Clone + 'static,
    {
        InputProperty::new(&mut self.input_props, default)
    }

    pub fn create_output_property<T>(&mut self, default: T) -> OutputProperty<T>
    where
        T: Clone + 'static,
    {
        OutputProperty::new(&mut self.output_props, default)
    }

    pub fn create_link_property<T>(&mut self, default: T) -> LinkProperty<T>
    where
        T: Clone + 'static,
    {
        LinkProperty::new(&mut self.link_props, default)
    }

    pub fn create_node_property<T>(&mut self, default: T) -> NodeProperty<T>
    where
        T: Clone + 'static,
    {
        NodeProperty::new(&mut self.node_props, default)
    }

    fn link_consecutive_inputs(&mut self, prev: IH, next: IH) {
        self.inputs[prev.idx].next = Some(next);
        self.inputs[next.idx].prev = Some(prev);
    }

    fn link_consecutive_outputs(&mut self, prev: OH, next: OH) {
        self.outputs[prev.idx].next = Some(next);
        self.outputs[next.idx].prev = Some(prev);
    }

    fn reserve(
        &mut self,
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Result<(), DagError> {
        self.inputs.reserve(n_inputs);
        self.outputs.reserve(n_outputs);
        self.links.reserve(n_links);
        self.nodes.reserve(n_nodes);
        // Also reserve the property containers.
        self.input_props.reserve(n_inputs)?;
        self.output_props.reserve(n_outputs)?;
        self.link_props.reserve(n_links)?;
        self.node_props.reserve(n_nodes)?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DagError> {
        self.inputs.clear();
        self.outputs.clear();
        self.links.clear();
        self.nodes.clear();
        // Also clear the property containers.
        self.input_props.clear()?;
        self.output_props.clear()?;
        self.link_props.clear()?;
        self.node_props.clear()?;
        Ok(())
    }

    pub fn garbage_collection(&mut self) -> Result<(), DagError> {
        let mut imap: Vec<IH> = (0usize..self.inputs.len()).map(|i| IH { idx: i }).collect();
        let mut omap: Vec<OH> = (0usize..self.outputs.len())
            .map(|i| OH { idx: i })
            .collect();
        let mut lmap: Vec<LH> = (0usize..self.links.len()).map(|i| LH { idx: i }).collect();
        let mut nmap: Vec<NH> = (0usize..self.nodes.len()).map(|i| NH { idx: i }).collect();
        // Clean up inputs.
        if !self.inputs.is_empty() {
            let mut left = 0usize;
            let mut right = self.inputs.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.inputs[left].deleted && left < right {
                    left += 1;
                }
                while self.inputs[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.inputs[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.inputs.swap(left, right);
                self.input_props.swap(left, right)?;
                imap.swap(left, right);
            };
            self.inputs.truncate(newlen);
            self.input_props.resize(newlen)?;
        }
        // Clean up outputs.
        if !self.outputs.is_empty() {
            let mut left = 0usize;
            let mut right = self.outputs.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.outputs[left].deleted && left < right {
                    left += 1;
                }
                while self.outputs[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.outputs[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.outputs.swap(left, right);
                self.output_props.swap(left, right)?;
                omap.swap(left, right);
            };
            self.outputs.truncate(newlen);
            self.output_props.resize(newlen)?;
        }
        // Clean up links.
        if !self.links.is_empty() {
            let mut left = 0usize;
            let mut right = self.links.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.links[left].deleted && left < right {
                    left += 1;
                }
                while self.links[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.links[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.links.swap(left, right);
                self.link_props.swap(left, right)?;
                lmap.swap(left, right);
            };
            self.links.truncate(newlen);
            self.link_props.resize(newlen)?;
        }
        // Clean up nodes.
        if !self.nodes.is_empty() {
            let mut left = 0usize;
            let mut right = self.nodes.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.nodes[left].deleted && left < right {
                    left += 1;
                }
                while self.nodes[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.nodes[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.nodes.swap(left, right);
                self.node_props.swap(left, right)?;
                nmap.swap(left, right);
            };
            self.nodes.truncate(newlen);
            self.node_props.resize(newlen)?;
        }
        // Update inputs.
        for input in self.inputs.iter_mut() {
            debug_assert!(!input.deleted, "Garbage collection isn't working properly");
            input.node = nmap[input.node.idx];
            if let Some(l) = &mut input.link {
                *l = lmap[l.idx]
            }
            if let Some(p) = &mut input.prev {
                *p = imap[p.idx]
            }
            if let Some(n) = &mut input.next {
                *n = imap[n.idx]
            }
        }
        // Update Outputs.
        for output in self.outputs.iter_mut() {
            debug_assert!(!output.deleted, "Garbage collection isn't working properly");
            output.node = nmap[output.node.idx];
            if let Some(l) = &mut output.link {
                *l = lmap[l.idx];
            }
            if let Some(p) = &mut output.prev {
                *p = omap[p.idx]
            }
            if let Some(n) = &mut output.next {
                *n = omap[n.idx]
            }
        }
        // Update links.
        for link in self.links.iter_mut() {
            debug_assert!(!link.deleted, "Garbage collection isn't working properly");
            link.start = omap[link.start.idx];
            link.end = imap[link.end.idx];
            if let Some(l) = &mut link.prev {
                *l = lmap[l.idx];
            }
            if let Some(l) = &mut link.next {
                *l = lmap[l.idx];
            }
        }
        // Update nodes.
        for node in self.nodes.iter_mut() {
            debug_assert!(!node.deleted, "Garbage collection isn't working properly");
            if let Some(i) = &mut node.input {
                *i = imap[i.idx];
            }
            if let Some(o) = &mut node.output {
                *o = omap[o.idx];
            }
        }
        // Clean up properties.
        self.input_props.garbage_collection();
        self.output_props.garbage_collection();
        self.link_props.garbage_collection();
        self.node_props.garbage_collection();
        Ok(())
    }

    pub fn node_inputs(&self, n: NH) -> impl Iterator<Item = IH> {
        std::iter::successors(self.nodes[n.idx].input, |i| self.inputs[i.idx].next)
    }

    pub fn node_outputs(&self, n: NH) -> impl Iterator<Item = OH> {
        std::iter::successors(self.nodes[n.idx].output, |o| self.outputs[o.idx].next)
    }
}

pub struct Workflow {
    graph: Graph,
    node_infos: NodeProperty<FuncInfo>,
}

impl Default for Workflow {
    fn default() -> Self {
        let mut graph = Graph::default();
        let node_infos = graph.create_node_property(FuncInfo::default());
        Self { graph, node_infos }
    }
}

impl Workflow {
    pub fn with_capacity(
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Self {
        let mut graph = Graph::with_capacity(n_inputs, n_outputs, n_links, n_nodes);
        let node_infos =
            Property::with_capacity(n_nodes, &mut graph.node_props, FuncInfo::default());
        Workflow { graph, node_infos }
    }

    pub fn add_node(
        &mut self,
        info: FuncInfo,
        input_handles: &mut [IH],
        output_handles: &mut [OH],
    ) -> Result<NH, DagError> {
        let n = self.graph.push_node(input_handles, output_handles)?;
        {
            let mut node_infos = self
                .node_infos
                .try_borrow_mut()
                .map_err(|_e| DagError::BorrowedPropertyAccess)?;
            node_infos[n] = info;
        }
        Ok(n)
    }

    pub fn connect(&mut self, from: OH, to: IH) -> Result<LH, DagError> {
        self.graph.push_link(from, to)
    }

    pub fn reserve(
        &mut self,
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Result<(), DagError> {
        self.graph.reserve(n_inputs, n_outputs, n_links, n_nodes)
    }

    pub fn clear(&mut self) -> Result<(), DagError> {
        self.graph.clear()
    }

    pub fn create_input_property<T>(&mut self, default: T) -> InputProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_input_property(default)
    }

    pub fn create_output_property<T>(&mut self, default: T) -> OutputProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_output_property(default)
    }

    pub fn create_link_property<T>(&mut self, default: T) -> LinkProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_link_property(default)
    }

    pub fn create_node_property<T>(&mut self, default: T) -> NodeProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_node_property(default)
    }

    pub fn garbage_collection(&mut self) -> Result<(), DagError> {
        self.graph.garbage_collection()
    }

    pub fn duplicate_node(&mut self, old: NH) -> Result<NH, DagError> {
        let src_inputs = self.graph.node_inputs(old).collect::<Box<[_]>>();
        let mut input_handles = vec![IH::default(); src_inputs.len()];
        let src_outputs = self.graph.node_outputs(old).collect::<Box<[_]>>();
        let mut output_handles = vec![OH::default(); src_outputs.len()];
        let new = self
            .graph
            .push_node(&mut input_handles, &mut output_handles)?;
        self.graph.node_props.copy(old, new)?;
        for (&new, &old) in input_handles.iter().zip(src_inputs.iter()) {
            self.graph.input_props.copy(old, new)?;
        }
        for (&new, &old) in output_handles.iter().zip(src_outputs.iter()) {
            self.graph.output_props.copy(old, new)?;
        }
        Ok(new)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::FuncInfo;

    // ============================================================
    // Graph — building and querying
    // ============================================================

    /// Build a simple chain: A(0 in, 1 out) -> B(1 in, 1 out) -> C(1 in, 0 out).
    fn chain_graph() -> (Graph, [NH; 3], [IH; 2], [OH; 2], [LH; 2]) {
        let mut g = Graph::default();
        let mut a_in = [];
        let mut a_out = [OH::default()];
        let na = g.push_node(&mut a_in, &mut a_out).unwrap();
        let mut b_in = [IH::default()];
        let mut b_out = [OH::default()];
        let nb = g.push_node(&mut b_in, &mut b_out).unwrap();
        let mut c_in = [IH::default()];
        let mut c_out = [];
        let nc = g.push_node(&mut c_in, &mut c_out).unwrap();
        let l0 = g.push_link(a_out[0], b_in[0]).unwrap();
        let l1 = g.push_link(b_out[0], c_in[0]).unwrap();

        (
            g,
            [na, nb, nc],
            [b_in[0], c_in[0]],
            [a_out[0], b_out[0]],
            [l0, l1],
        )
    }

    #[test]
    fn t_push_node_assigns_handles() {
        let mut g = Graph::default();
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 3];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert_eq!(n.idx, 0);
        assert_eq!(ins[0].idx, 0);
        assert_eq!(ins[1].idx, 1);
        assert_eq!(outs[0].idx, 0);
        assert_eq!(outs[1].idx, 1);
        assert_eq!(outs[2].idx, 2);
    }

    #[test]
    fn t_node_inputs_outputs_iteration() {
        let mut g = Graph::default();
        let mut ins = [IH::default(); 3];
        let mut outs = [OH::default(); 2];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert!(g.node_inputs(n).eq(ins));
        assert!(g.node_outputs(n).eq(outs));
    }

    #[test]
    fn t_num_node_inputs_outputs() {
        let mut g = Graph::default();
        let mut ins = [IH::default(); 4];
        let mut outs = [OH::default(); 1];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert_eq!(g.node_inputs(n).count(), 4);
        assert_eq!(g.node_outputs(n).count(), 1);
    }

    #[test]
    fn t_zero_inputs_zero_outputs() {
        let mut g = Graph::default();
        let n = g.push_node(&mut [], &mut []).unwrap();
        assert_eq!(g.node_inputs(n).count(), 0);
        assert_eq!(g.node_outputs(n).count(), 0);
        assert_eq!(g.node_inputs(n).count(), 0);
        assert_eq!(g.node_outputs(n).count(), 0);
    }

    #[test]
    fn t_push_link_connects() {
        let (g, _, ins, outs, links) = chain_graph();
        // A's output links to B's input.
        assert_eq!(g.links[links[0].idx].start, outs[0]);
        assert_eq!(g.links[links[0].idx].end, ins[0]);
        // B's input references the link.
        assert_eq!(g.inputs[ins[0].idx].link, Some(links[0]));
    }

    #[test]
    fn t_multiple_links_from_one_output() {
        let mut g = Graph::default();
        let mut a_out = [OH::default()];
        let _na = g.push_node(&mut [], &mut a_out).unwrap();

        let mut b_in = [IH::default()];
        let _nb = g.push_node(&mut b_in, &mut []).unwrap();

        let mut c_in = [IH::default()];
        let _nc = g.push_node(&mut c_in, &mut []).unwrap();

        let l0 = g.push_link(a_out[0], b_in[0]).unwrap();
        let l1 = g.push_link(a_out[0], c_in[0]).unwrap();

        // Output's linked list: l0 -> l1.
        assert_eq!(g.outputs[a_out[0].idx].link, Some(l0));
        assert_eq!(g.links[l0.idx].next, Some(l1));
        assert_eq!(g.links[l1.idx].prev, Some(l0));
        assert_eq!(g.links[l1.idx].next, None);
    }

    #[test]
    fn t_chain_structure() {
        let (g, nodes, ins, outs, _) = chain_graph();
        // A has 0 inputs, 1 output.
        assert_eq!(g.node_inputs(nodes[0]).count(), 0);
        assert_eq!(g.node_outputs(nodes[0]).count(), 1);
        // B has 1 input, 1 output.
        assert_eq!(g.node_inputs(nodes[1]).count(), 1);
        assert_eq!(g.node_outputs(nodes[1]).count(), 1);
        // C has 1 input, 0 outputs.
        assert_eq!(g.node_inputs(nodes[2]).count(), 1);
        assert_eq!(g.node_outputs(nodes[2]).count(), 0);
        // B's input belongs to node B.
        assert_eq!(g.inputs[ins[0].idx].node, nodes[1]);
        // A's output belongs to node A.
        assert_eq!(g.outputs[outs[0].idx].node, nodes[0]);
    }

    #[test]
    fn t_clear_empties_everything() {
        let (mut g, _, _, _, _) = chain_graph();
        g.clear().unwrap();
        assert!(g.inputs.is_empty());
        assert!(g.outputs.is_empty());
        assert!(g.links.is_empty());
        assert!(g.nodes.is_empty());
    }

    // ============================================================
    // Properties — lifecycle
    // ============================================================

    #[test]
    fn t_property_default_on_creation() {
        let mut g = Graph::default();
        let labels = g.create_node_property("unnamed".to_string());
        let mut ins = [IH::default()];
        let mut outs = [OH::default()];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert_eq!(*labels.get(n).unwrap(), "unnamed");
    }

    #[test]
    fn t_property_set_and_get() {
        let mut g = Graph::default();
        let mut labels = g.create_node_property(0i32);
        let n0 = g.push_node(&mut [], &mut []).unwrap();
        let n1 = g.push_node(&mut [], &mut []).unwrap();
        labels.set(n0, 42).unwrap();
        labels.set(n1, 99).unwrap();
        assert_eq!(*labels.get(n0).unwrap(), 42);
        assert_eq!(*labels.get(n1).unwrap(), 99);
    }

    #[test]
    fn t_property_get_cloned() {
        let mut g = Graph::default();
        let mut labels = g.create_node_property(String::new());
        let n = g.push_node(&mut [], &mut []).unwrap();
        labels.set(n, "hello".into()).unwrap();
        let val = labels.get_cloned(n).unwrap();
        assert_eq!(val, "hello");
    }

    #[test]
    fn t_property_created_after_elements() {
        let mut g = Graph::default();
        let n0 = g.push_node(&mut [], &mut []).unwrap();
        let n1 = g.push_node(&mut [], &mut []).unwrap();
        // Property created after 2 nodes exist — should have 2 entries with default.
        let labels = g.create_node_property(7i32);
        assert_eq!(*labels.get(n0).unwrap(), 7);
        assert_eq!(*labels.get(n1).unwrap(), 7);
    }

    #[test]
    fn t_property_grows_with_new_elements() {
        let mut g = Graph::default();
        let mut labels = g.create_node_property(-1i32);
        let n0 = g.push_node(&mut [], &mut []).unwrap();
        labels.set(n0, 100).unwrap();
        // Add another node — property should grow with default.
        let n1 = g.push_node(&mut [], &mut []).unwrap();
        assert_eq!(*labels.get(n0).unwrap(), 100);
        assert_eq!(*labels.get(n1).unwrap(), -1);
    }

    #[test]
    fn t_input_property_on_multi_input_node() {
        let mut g = Graph::default();
        let mut costs = g.create_input_property(0.0f64);
        let mut ins = [IH::default(); 3];
        let _n = g.push_node(&mut ins, &mut []).unwrap();
        costs.set(ins[0], 1.0).unwrap();
        costs.set(ins[1], 2.0).unwrap();
        costs.set(ins[2], 3.0).unwrap();
        assert_eq!(*costs.get(ins[0]).unwrap(), 1.0);
        assert_eq!(*costs.get(ins[1]).unwrap(), 2.0);
        assert_eq!(*costs.get(ins[2]).unwrap(), 3.0);
    }

    #[test]
    fn t_link_property() {
        let (mut g, _, ins, outs, links) = chain_graph();
        let mut weights = g.create_link_property(1.0f64);
        weights.set(links[0], 0.5).unwrap();
        weights.set(links[1], 0.8).unwrap();
        assert_eq!(*weights.get(links[0]).unwrap(), 0.5);
        assert_eq!(*weights.get(links[1]).unwrap(), 0.8);
        // Links have correct topology.
        assert_eq!(g.links[links[0].idx].start, outs[0]);
        assert_eq!(g.links[links[0].idx].end, ins[0]);
    }

    #[test]
    fn t_multiple_properties_on_same_handle_type() {
        let mut g = Graph::default();
        let mut names = g.create_node_property(String::new());
        let mut flags = g.create_node_property(false);
        let n = g.push_node(&mut [], &mut []).unwrap();
        names.set(n, "add".into()).unwrap();
        flags.set(n, true).unwrap();
        assert_eq!(*names.get(n).unwrap(), "add");
        assert_eq!(*flags.get(n).unwrap(), true);
    }

    #[test]
    fn t_property_drop_cleans_up_in_gc() {
        let mut g = Graph::default();
        let _n = g.push_node(&mut [], &mut []).unwrap();
        {
            let _temp = g.create_node_property(0i32);
            assert_eq!(g.node_props.props.len(), 1);
            // _temp dropped here
        }
        g.node_props.garbage_collection();
        assert_eq!(g.node_props.props.len(), 0);
    }

    #[test]
    fn t_propbuf_deref_to_slice() {
        let mut g = Graph::default();
        let mut vals = g.create_node_property(0i32);
        let n0 = g.push_node(&mut [], &mut []).unwrap();
        let n1 = g.push_node(&mut [], &mut []).unwrap();
        vals.set(n0, 10).unwrap();
        vals.set(n1, 20).unwrap();
        let buf = vals.try_borrow().unwrap();
        let slice: &[i32] = &buf;
        assert_eq!(slice, &[10, 20]);
    }

    #[test]
    fn t_propbuf_index_by_handle() {
        let mut g = Graph::default();
        let mut vals = g.create_node_property(0i32);
        let n0 = g.push_node(&mut [], &mut []).unwrap();
        let n1 = g.push_node(&mut [], &mut []).unwrap();
        vals.set(n0, 10).unwrap();
        vals.set(n1, 20).unwrap();
        let buf = vals.try_borrow().unwrap();
        assert_eq!(buf[n0], 10);
        assert_eq!(buf[n1], 20);
    }

    // ============================================================
    // Workflow
    // ============================================================

    fn make_func_info(name: &str) -> FuncInfo {
        FuncInfo {
            name: name.to_string(),
            desc: String::new(),
            func: None,
        }
    }

    #[test]
    fn t_workflow_add_node() {
        let mut w = Workflow::default();
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 1];
        let n = w
            .add_node(make_func_info("add"), &mut ins, &mut outs)
            .unwrap();
        let info = w.node_infos.get(n).unwrap();
        assert_eq!(info.name, "add");
    }

    #[test]
    fn t_workflow_connect() {
        let mut w = Workflow::default();
        let mut a_out = [OH::default()];
        let _na = w
            .add_node(make_func_info("source"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let _nb = w
            .add_node(make_func_info("sink"), &mut b_in, &mut [])
            .unwrap();
        let lh = w.connect(a_out[0], b_in[0]).unwrap();
        assert_eq!(w.graph.links[lh.idx].start, a_out[0]);
        assert_eq!(w.graph.links[lh.idx].end, b_in[0]);
    }

    #[test]
    fn t_workflow_duplicate_node() {
        let mut w = Workflow::default();
        let mut input_labels = w.create_input_property("default_in".to_string());
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 1];
        let orig = w
            .add_node(make_func_info("mul"), &mut ins, &mut outs)
            .unwrap();
        input_labels.set(ins[0], "x".into()).unwrap();
        input_labels.set(ins[1], "y".into()).unwrap();
        let dup = w.duplicate_node(orig).unwrap();
        assert_ne!(orig, dup);
        // Duplicated node has same func info.
        let orig_info = w.node_infos.get_cloned(orig).unwrap();
        let dup_info = w.node_infos.get_cloned(dup).unwrap();
        assert_eq!(orig_info.name, dup_info.name);
        // Duplicated node has same number of inputs/outputs.
        assert_eq!(w.graph.node_inputs(dup).count(), 2);
        assert_eq!(w.graph.node_outputs(dup).count(), 1);
        // Input properties are copied.
        let mut dup_ins = w.graph.node_inputs(dup);
        assert_eq!(*input_labels.get(dup_ins.next().unwrap()).unwrap(), "x");
        assert_eq!(*input_labels.get(dup_ins.next().unwrap()).unwrap(), "y");
        assert!(dup_ins.next().is_none());
        // Duplicated node is disconnected.
        for ih in w.graph.node_inputs(dup) {
            assert!(w.graph.inputs[ih.idx].link.is_none());
        }
    }

    #[test]
    fn t_workflow_diamond_topology() {
        // A -> B, A -> C, B -> D, C -> D
        let mut w = Workflow::default();
        let mut a_out = [OH::default(); 2];
        let na = w
            .add_node(make_func_info("A"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let mut b_out = [OH::default()];
        let nb = w
            .add_node(make_func_info("B"), &mut b_in, &mut b_out)
            .unwrap();
        let mut c_in = [IH::default()];
        let mut c_out = [OH::default()];
        let nc = w
            .add_node(make_func_info("C"), &mut c_in, &mut c_out)
            .unwrap();
        let mut d_in = [IH::default(); 2];
        let nd = w.add_node(make_func_info("D"), &mut d_in, &mut []).unwrap();

        w.connect(a_out[0], b_in[0]).unwrap();
        w.connect(a_out[1], c_in[0]).unwrap();
        w.connect(b_out[0], d_in[0]).unwrap();
        w.connect(c_out[0], d_in[1]).unwrap();

        // Verify fan-out from A.
        assert_eq!(w.graph.node_outputs(na).count(), 2);
        // Verify D has 2 inputs, both linked.
        assert_eq!(w.graph.node_inputs(nd).count(), 2);
        for ih in w.graph.node_inputs(nd) {
            assert!(w.graph.inputs[ih.idx].link.is_some());
        }
        // Verify B and C each have 1 input, 1 output.
        assert_eq!(w.graph.node_inputs(nb).count(), 1);
        assert_eq!(w.graph.node_outputs(nb).count(), 1);
        assert_eq!(w.graph.node_inputs(nc).count(), 1);
        assert_eq!(w.graph.node_outputs(nc).count(), 1);
    }

    #[test]
    fn t_workflow_clear() {
        let mut w = Workflow::default();
        let mut outs = [OH::default()];
        let _n = w.add_node(make_func_info("x"), &mut [], &mut outs).unwrap();
        w.clear().unwrap();
        assert!(w.graph.nodes.is_empty());
        assert!(w.graph.inputs.is_empty());
        assert!(w.graph.outputs.is_empty());
        assert!(w.graph.links.is_empty());
    }

    #[test]
    fn t_workflow_with_custom_properties() {
        let mut w = Workflow::default();
        let mut dirty = w.create_node_property(false);
        let mut input_vals = w.create_input_property(0.0f64);

        let mut a_out = [OH::default()];
        let na = w
            .add_node(make_func_info("source"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default(); 2];
        let nb = w
            .add_node(make_func_info("add"), &mut b_in, &mut [])
            .unwrap();

        dirty.set(na, true).unwrap();
        input_vals.set(b_in[0], 3.14).unwrap();
        input_vals.set(b_in[1], 2.72).unwrap();

        assert_eq!(*dirty.get(na).unwrap(), true);
        assert_eq!(*dirty.get(nb).unwrap(), false); // default
        assert_eq!(*input_vals.get(b_in[0]).unwrap(), 3.14);
        assert_eq!(*input_vals.get(b_in[1]).unwrap(), 2.72);
    }
}
