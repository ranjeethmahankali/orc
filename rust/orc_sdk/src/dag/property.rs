use super::{DagError, Handle, IH, LH, NH, OH};
use std::{
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    ops::{Deref, DerefMut, Index, IndexMut},
    rc::{Rc, Weak},
};

#[derive(Default)]
pub struct PropertyContainer<H>
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
            prop.copy(src.index(), dst.index())?;
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
        &self.buf[handle.index()]
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
        &mut self.buf[h.index()]
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
    /// [`DagError::BorrowedPropertyAccess`] is returned, otherwise a reference to
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
            |v| &v.buf[h.index()],
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
            |v| &mut v.buf[h.index()],
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

#[cfg(test)]
mod test {
    use crate::{
        IH, OH,
        dag::{Graph, Handle, test::chain_graph},
    };

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
        assert_eq!(g.links[links[0].index()].start, outs[0]);
        assert_eq!(g.links[links[0].index()].end, ins[0]);
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
        assert!(*flags.get(n).unwrap());
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
}
