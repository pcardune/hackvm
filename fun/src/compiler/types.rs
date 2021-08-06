use super::*;
use std::{borrow::Borrow, collections::HashMap, fmt::Debug, hash::Hash};

#[derive(Debug, Clone)]
pub struct OrderedMap<K, V>
where
    K: Eq + Hash,
{
    key_map: HashMap<K, usize>,
    items: Vec<V>,
}
impl<K, V> OrderedMap<K, V>
where
    K: Eq + Hash + Debug,
{
    pub fn index_of<Q: ?Sized>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.key_map.get(key).copied()
    }
    #[allow(dead_code)]
    pub fn get_at(&self, index: usize) -> Option<&V> {
        self.items.get(index)
    }
    #[allow(dead_code)]
    pub fn get_at_mut(&mut self, index: usize) -> Option<&mut V> {
        self.items.get_mut(index)
    }
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.index_of(key)
            .map(|index| self.items.get(index))
            .flatten()
    }
    #[allow(dead_code)]
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.index_of(key) {
            Some(index) => self.items.get_mut(index),
            None => None,
        }
    }
    pub fn push(&mut self, key: K, value: V) -> Result<usize> {
        if self.key_map.contains_key(&key) {
            Err(anyhow!("key {:?} was already pushed", key))
        } else {
            let index = self.items.len();
            self.items.push(value);
            self.key_map.insert(key, index);
            Ok(index)
        }
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
}
impl<K, V> Default for OrderedMap<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        OrderedMap {
            key_map: HashMap::new(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Getters)]
pub struct ObjectTypeField {
    #[getset(get = "pub")]
    type_id: TypeId,
    #[getset(get = "pub")]
    index: usize,
}
#[derive(Debug, Getters, Clone)]
pub struct ObjectType {
    #[getset(get = "pub")]
    name: String,
    fields: OrderedMap<String, ObjectTypeField>,
}
impl ObjectType {
    pub fn new(name: &str) -> ObjectType {
        ObjectType {
            name: name.to_string(),
            fields: OrderedMap::default(),
        }
    }
    pub fn get_field(&self, field_name: &str) -> Option<&ObjectTypeField> {
        self.fields.get(field_name)
    }
    pub fn add_field(&mut self, name: &str, type_id: TypeId) -> Result<usize> {
        let index = self.fields.len();
        let field = ObjectTypeField { type_id, index };
        self.fields
            .push(name.to_string(), field)
            .map_err(|_| anyhow!("field {} already declared", name))
    }
}

#[derive(Debug)]
pub struct MethodType {
    parameters: Vec<TypeId>,
    return_type: TypeId,
}
impl MethodType {
    pub fn new(parameters: Vec<TypeId>, return_type: TypeId) -> MethodType {
        MethodType {
            parameters,
            return_type,
        }
    }
}

#[derive(Debug)]
pub struct ConstructorType {
    parameters: Vec<Type>,
}

#[derive(Debug)]
pub enum Type {
    Primitive(String),
    Alias(usize),
    Method(MethodType),
    Constructor(ConstructorType),
    Object(ObjectType),
}

impl Type {
    pub fn object(&self) -> &ObjectType {
        match self {
            Self::Object(val) => val,
            _ => panic!("called `Type::object()` on a `{:?}` value", self),
        }
    }
    pub fn add_field_unchecked(&mut self, name: &str, type_id: TypeId) -> Result<usize> {
        if let Self::Object(obj) = self {
            obj.add_field(name, type_id)
        } else {
            panic!("add_field() can only be called on object types");
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeId(usize);

#[derive(Debug, Default)]
pub struct TypeArena {
    types: Vec<Type>,
}
impl TypeArena {
    pub fn get(&self, id: TypeId) -> Option<&Type> {
        self.types.get(id.0)
    }
    pub fn get_mut(&mut self, id: TypeId) -> Option<&mut Type> {
        self.types.get_mut(id.0)
    }
    pub fn add_type(&mut self, obj_type: Type) -> Result<TypeId> {
        self.types.push(obj_type);
        Ok(TypeId(self.types.len() - 1))
    }
}
