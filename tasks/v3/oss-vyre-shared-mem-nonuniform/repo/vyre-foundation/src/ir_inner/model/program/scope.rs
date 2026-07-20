use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

/// Refcount-shared, copy-on-write lexical scope map for IR validation.
///
/// `Scope` lets branch validators share an unchanged parent binding table and
/// allocate only when a branch actually inserts or updates a binding. Cloning a
/// scope is O(1); the first write after a clone copies the underlying map.
///
/// # Examples
///
/// ```
/// use vyre::ir::model::program::Scope;
///
/// let mut root = Scope::new();
/// root.insert("idx".to_string(), 0_u32);
///
/// let mut branch = root.child();
/// branch.insert("tmp".to_string(), 1_u32);
///
/// assert_eq!(root.get("idx"), Some(&0));
/// assert!(!root.contains_key("tmp"));
/// assert_eq!(branch.get("tmp"), Some(&1));
/// ```
#[derive(Debug, Clone)]
pub struct Scope<K, V> {
    bindings: Rc<HashMap<K, V>>,
}

impl<K, V> Default for Scope<K, V> {
    fn default() -> Self {
        Self {
            bindings: Rc::new(HashMap::new()),
        }
    }
}

impl<K, V> Scope<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    /// Create an empty scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let scope: Scope<String, u32> = Scope::new();
    /// assert!(scope.is_empty());
    /// ```
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope from an existing binding map.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use vyre::ir::model::program::Scope;
    ///
    /// let scope = Scope::from_map(HashMap::from([("idx".to_string(), 0_u32)]));
    /// assert_eq!(scope.get("idx"), Some(&0));
    /// ```
    #[must_use]
    #[inline]
    pub fn from_map(bindings: HashMap<K, V>) -> Self {
        Self {
            bindings: Rc::new(bindings),
        }
    }

    /// Create an O(1) child scope sharing the same bindings until mutation.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let mut parent = Scope::new();
    /// parent.insert("a".to_string(), 1_u32);
    /// let child = parent.child();
    /// assert_eq!(child.get("a"), Some(&1));
    /// ```
    #[must_use]
    #[inline]
    pub fn child(&self) -> Self {
        Self {
            bindings: Rc::clone(&self.bindings),
        }
    }

    /// Return a binding by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let mut scope = Scope::new();
    /// scope.insert("flag".to_string(), true);
    /// assert_eq!(scope.get("flag"), Some(&true));
    /// ```
    #[must_use]
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.bindings.get(key)
    }

    /// Return true when a binding exists for `key`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let mut scope = Scope::new();
    /// scope.insert("x".to_string(), 7_u32);
    /// assert!(scope.contains_key("x"));
    /// ```
    #[must_use]
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.bindings.contains_key(key)
    }

    /// Insert or replace one binding, cloning shared storage only if needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let mut scope = Scope::new();
    /// assert_eq!(scope.insert("x".to_string(), 1_u32), None);
    /// assert_eq!(scope.insert("x".to_string(), 2_u32), Some(1));
    /// assert_eq!(scope.get("x"), Some(&2));
    /// ```
    #[inline]
    #[must_use]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        Rc::make_mut(&mut self.bindings).insert(key, value)
    }

    /// Number of bindings visible in this scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let mut scope = Scope::new();
    /// scope.insert("x".to_string(), 1_u32);
    /// assert_eq!(scope.len(), 1);
    /// ```
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Return true when the scope contains no bindings.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::model::program::Scope;
    ///
    /// let scope: Scope<String, u32> = Scope::new();
    /// assert!(scope.is_empty());
    /// ```
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}
