use std::collections::HashMap;

use hackvm::{VMSegment, VMToken};

use super::TypeId;

#[derive(Debug, Clone, Copy)]
pub struct MemRef {
    pub segment: VMSegment,
    pub index: usize,
    pub type_id: TypeId,
}
impl MemRef {
    pub fn as_pop_token(&self) -> VMToken {
        VMToken::Pop(self.segment, self.index as u16)
    }
    pub fn as_push_token(&self) -> VMToken {
        VMToken::Push(self.segment, self.index as u16)
    }
}

#[derive(Default)]
pub struct Namespace {
    names: HashMap<String, MemRef>,
    offset: HashMap<VMSegment, usize>,
}
impl Namespace {
    pub fn segment_size(&self, segment: &VMSegment) -> usize {
        self.names
            .values()
            .filter(|v| &v.segment == segment)
            .count()
    }

    fn get_offset(&self, segment: &VMSegment) -> usize {
        *self.offset.get(segment).unwrap_or(&0)
    }

    fn get_index(&self, segment: &VMSegment) -> usize {
        self.segment_size(segment) + self.get_offset(segment)
    }

    pub fn set_offset(&mut self, segment: &VMSegment, namespace: &Namespace) {
        self.offset.insert(*segment, namespace.get_index(segment));
    }

    pub fn register(&mut self, name: &str, segment: &VMSegment, type_id: TypeId) -> Option<usize> {
        if self.names.contains_key(name) {
            None
        } else {
            let index = self.get_index(segment);
            self.names.insert(
                name.to_owned(),
                MemRef {
                    segment: *segment,
                    index,
                    type_id,
                },
            );
            Some(index)
        }
    }
    pub fn get(&self, name: &str) -> Option<MemRef> {
        self.names.get(name).copied()
    }
    pub fn clear(&mut self) {
        self.names.clear();
    }
}
