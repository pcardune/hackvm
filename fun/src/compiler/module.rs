use std::mem;

use anyhow::Context;

use super::*;
use symbols::HashTree;

mod statics {
    use super::*;
    #[derive(Default)]
    pub struct StaticsTable {
        index: usize,
        static_names: HashMap<String, HashMap<String, MemRef>>,
    }

    impl StaticsTable {
        pub fn insert(
            &mut self,
            class_name: &str,
            field_name: &str,
            type_id: TypeId,
        ) -> Option<MemRef> {
            let mut inner_table = self.static_names.get_mut(class_name);
            if inner_table.is_none() {
                self.static_names
                    .insert(class_name.to_string(), HashMap::new());
                inner_table = self.static_names.get_mut(class_name);
            }
            let inner_table = inner_table.unwrap();

            let existing = inner_table.insert(
                field_name.to_string(),
                MemRef {
                    segment: VMSegment::Static,
                    index: self.index,
                    type_id,
                },
            );
            if existing.is_none() {
                self.index += 1;
            }
            existing
        }

        pub fn get(&self, class_name: &str, field_name: &str) -> Option<MemRef> {
            self.static_names
                .get(class_name)
                .map(|inner_map| inner_map.get(field_name))
                .flatten()
                .copied()
        }
    }
}
use statics::*;

#[derive(Debug)]
pub enum Symbol {
    Virtual(HashMap<String, Symbol>),
    Mem(MemRef),
}

impl Symbol {
    pub fn resolve(&self, names: &[&str]) -> Result<MemRef> {
        let mut s = self;
        let names = names.iter().peekable();
        loop {
            match s {
                Symbol::Mem(mem_ref) => {
                    if names.next().is_some() {
                        return Err(anyhow!("Arrived at bound variable before finishing descent!"))
                    }
                    return Ok(*mem_ref);
                }
                Symbol::Virtual(map) => {
                    match names.next() {
                        Some(name) => {
                            s = map.get(*name).ok_or_else(|| anyhow!(""))?;
                        },
                        None => {
                            return Err(anyhow!("Ran out of names to traverse before reaching MemRef"))
                        }
                    }
                }
            }
        }
    }
}

pub struct ModuleCompiler<'m> {
    statics_table: StaticsTable,
    types: TypeArena,
    type_names: HashTree<String, TypeId>,
    symbols: HashTree<String, Symbol>,
    module: &'m Module,
}

impl<'m> ModuleCompiler<'m> {
    pub fn new(module: &Module) -> ModuleCompiler {
        let mut types = TypeArena::default();

        // start by adding built-in types
        // TODO: make types support generics and use that
        // instead of number[]
        let mut primitive_types = HashTree::default();
        for type_name in &["number", "boolean", "void", "number[]", "string"] {
            let id = types
                .add_type(Type::Primitive(type_name.to_string()))
                .unwrap();
            primitive_types.insert(type_name.to_string(), id);
        }

        ModuleCompiler {
            statics_table: StaticsTable::default(),
            types,
            type_names: HashTree::with_parent(primitive_types),
            module,
            symbols: HashTree::default(),
        }
    }

    pub fn get_object_types(&self) -> &TypeArena {
        &self.types
    }

    pub fn resolve_type(&self, type_name: &str) -> Result<TypeId> {
        match self.type_names.get(type_name) {
            Some(field) => Ok(*field),
            None => Err(anyhow!("{} is not a known type", type_name)),
        }
    }

    pub fn populate_types(&mut self) -> Result<()> {
        // add empty class types
        let class_types = self
            .module
            .classes()
            .iter()
            .map(|class_decl| -> Result<(TypeId, &Node<ClassDecl>)> {
                let type_id = self
                    .types
                    .add_type(Type::Object(ObjectType::new(class_decl.data().name())))?;

                self.type_names
                    .insert(class_decl.name().to_string(), type_id);

                Ok((type_id, class_decl))
            })
            .collect::<Result<Vec<_>>>()?;

        // go back through the class types and populate their fields
        for (instance_type_id, class_decl) in class_types {
            let mut fields_to_add: Vec<(&str, TypeId)> = Vec::new();
            for field in class_decl.fields() {
                let name = field.data().name();
                let type_name = field.data().type_name();
                let field_type_id = self.resolve_type(type_name).with_context(|| {
                    format!(
                        "Could not resolve type {} for field {} in class {}",
                        type_name,
                        name,
                        class_decl.name()
                    )
                })?;
                match field.data().scope() {
                    Scope::Instance => {
                        fields_to_add.push((name, field_type_id));
                    }
                    _ => {}
                }
            }
            for method in class_decl.methods() {
                let name = method.name();
                let return_type = self.resolve_type(method.type_name())?;
                let mut parameters: Vec<TypeId> = Vec::new();
                for p in method.parameters() {
                    parameters.push(self.resolve_type(p.type_name())?);
                }
                let type_id = self
                    .types
                    .add_type(Type::Method(MethodType::new(parameters, return_type)))?;
                fields_to_add.push((name, type_id));
            }
            let instance_type = self.types.get_mut(instance_type_id).unwrap();
            for (name, field_type_id) in fields_to_add {
                instance_type.add_field_unchecked(name, field_type_id)?;
            }
        }
        Ok(())
    }

    fn map_statics(&mut self) {
        let mut index: usize = 0;
        for class_decl in self.module.classes() {
            let mut static_map: HashMap<String, Symbol> = HashMap::new();

            for field in class_decl.fields() {
                if field.data().scope() == &Scope::Static {
                    let field_type_id = self.resolve_type(field.data().type_name()).unwrap();

                    let mem_ref = MemRef {
                        segment: VMSegment::Static,
                        index,
                        type_id: field_type_id,
                    };
                    static_map.insert(field.data().name().to_string(), Symbol::Mem(mem_ref));
                    index += 1;

                    self.statics_table.insert(
                        class_decl.data().name(),
                        field.data().name(),
                        field_type_id,
                    );
                }
            }
            self.symbols
                .insert(class_decl.name().to_string(), Symbol::Virtual(static_map));
        }
    }

    pub fn get_static_field(&self, class_name: &str, field_name: &str) -> Option<MemRef> {
        self.symbols.get(class_name).map(|s| match s {
            Symbol::Virtual(fields) => {

            }
        })
        self.statics_table.get(class_name, field_name)
    }

    pub fn compile(mut self) -> Result<Vec<VMToken>> {
        self.populate_types()?;
        self.map_statics();
        let mut class_compilers = self
            .module
            .classes()
            .iter()
            .map(|c| ClassDeclCompiler::new(&self, c))
            .collect::<Vec<_>>();

        let mut commands: Vec<VMToken> = Vec::new();
        for class_decl in class_compilers.iter_mut() {
            commands.append(&mut class_decl.compile()?)
        }
        return Ok(commands);
    }
}
