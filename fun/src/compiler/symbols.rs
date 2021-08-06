use std::{borrow::Borrow, collections::HashMap, hash::Hash};

#[derive(Debug)]
pub struct HashTree<K, V>
where
    K: Eq + Hash,
{
    parent: Option<Box<HashTree<K, V>>>,
    data: HashMap<K, V>,
}

impl<K, V> HashTree<K, V>
where
    K: Eq + Hash,
{
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let value = self.data.get(k);
        if let Some(value) = value {
            Some(value)
        } else if let Some(parent) = self.parent.borrow() {
            parent.get(k)
        } else {
            None
        }
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.data.insert(k, v)
    }
    pub fn with_parent(parent: HashTree<K, V>) -> HashTree<K, V> {
        HashTree {
            parent: Some(Box::new(parent)),
            data: HashMap::default(),
        }
    }
}

impl<K, V> Default for HashTree<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        HashTree {
            parent: None,
            data: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hash_tree() {
        let mut file_scope: HashTree<&str, usize> = HashTree::default();
        file_scope.insert("a", 1);
        file_scope.insert("b", 2);

        let mut function_scope = HashTree::with_parent(file_scope);
        assert_eq!(function_scope.get(&"a"), Some(&1));

        function_scope.insert("a", 3);
        assert_eq!(function_scope.get(&"a"), Some(&3));
    }
}
