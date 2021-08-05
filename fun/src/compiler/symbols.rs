use std::{collections::HashMap, hash::Hash};

#[derive(Debug, Default)]
pub struct HashTree<'p, K, V>
where
    K: Eq + Hash,
{
    parent: Option<&'p HashTree<'p, K, V>>,
    data: HashMap<K, V>,
}

impl<'p, K, V> HashTree<'p, K, V>
where
    K: Eq + Hash,
{
    pub fn get(&self, k: &K) -> Option<&V> {
        let value = self.data.get(k);
        if let Some(value) = value {
            Some(value)
        } else if let Some(parent) = self.parent {
            parent.get(k)
        } else {
            None
        }
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.data.insert(k, v)
    }
    pub fn with_parent(parent: &'p HashTree<K, V>) -> HashTree<'p, K, V> {
        HashTree {
            parent: Some(parent),
            data: HashMap::default(),
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

        let mut function_scope = HashTree::with_parent(&file_scope);
        assert_eq!(function_scope.get(&"a"), Some(&1));

        function_scope.insert("a", 3);
        assert_eq!(function_scope.get(&"a"), Some(&3));
    }
}
