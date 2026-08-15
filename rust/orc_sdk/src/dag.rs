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
#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct IH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct OH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct LH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
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
        let mut prev: Option<LH> = None;
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
                prev = Some(li);
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
            prev,
            next: None,
            deleted: false,
        })?;
        // Connect the link to the input. Inputs only have one incoming link.
        self.inputs[to.idx].link = Some(lh);
        // Append to the output's linked list.
        match prev {
            Some(p) => self.links[p.idx].next = Some(lh),
            None => self.outputs[from.idx].link = Some(lh),
        }
        Ok(lh)
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

    pub fn num_node_inputs(&self, n: NH) -> usize {
        self.node_inputs(n).count()
    }

    pub fn num_node_outputs(&self, n: NH) -> usize {
        self.node_outputs(n).count()
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
        let mut input_handles = vec![IH::default(); self.graph.num_node_inputs(old)];
        let mut output_handles = vec![OH::default(); self.graph.num_node_outputs(old)];
        let new = self
            .graph
            .push_node(&mut input_handles, &mut output_handles)?;
        self.graph.node_props.copy(old, new).map(|()| new)
    }
}
