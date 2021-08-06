use anyhow::Context;

use super::*;
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

pub struct ModuleCompiler<'m> {
    statics_table: StaticsTable,
    object_types: TypeTable,
    module: &'m Module,
}

impl<'m> ModuleCompiler<'m> {
    pub fn new(module: &Module) -> ModuleCompiler {
        ModuleCompiler {
            statics_table: StaticsTable::default(),
            object_types: TypeTable::default(),
            module,
        }
    }

    pub fn get_static_field(&self, class_name: &str, field_name: &str) -> Option<MemRef> {
        self.statics_table.get(class_name, field_name)
    }

    pub fn get_object_types(&self) -> &TypeTable {
        &self.object_types
    }

    pub fn resolve_type(&self, type_name: &str) -> Result<TypeId> {
        let field_type_id = match self.object_types.id_for_type(type_name) {
            Some(id) => id,
            None => return Err(anyhow!("{} is not a known type", type_name)),
        };
        Ok(field_type_id)
    }

    pub fn populate_types(&mut self) -> Result<()> {
        // start by adding built-in types
        // TODO: make types support generics and use that
        // instead of number[]
        for type_name in &["number", "boolean", "void", "number[]", "string"] {
            self.object_types
                .add_type(type_name, Type::Primitive(type_name.to_string()))?;
        }

        // add empty class types
        let class_types = self
            .module
            .classes()
            .iter()
            .map(|class_decl| -> Result<(TypeId, &Node<ClassDecl>)> {
                let type_id = self.object_types.add_type(
                    class_decl.data().name(),
                    Type::Object(ObjectType::new(class_decl.data().name())),
                )?;
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
                    Scope::Static => {
                        if let Some(_) =
                            self.statics_table
                                .insert(class_decl.data().name(), name, field_type_id)
                        {
                            return Err(anyhow!("Static field \"{}\" declared twice", name));
                        }
                    }
                    Scope::Instance => {
                        fields_to_add.push((name, field_type_id));
                    }
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
                    .object_types
                    .add_type(name, Type::Method(MethodType::new(parameters, return_type)))?;
                fields_to_add.push((name, type_id));
            }
            let instance_type = match self.object_types.get_by_id_mut(instance_type_id).unwrap() {
                Type::Object(o) => o,
                _ => unreachable!(),
            };
            for (name, field_type_id) in fields_to_add {
                instance_type.add_field(name, field_type_id)?;
            }
        }
        Ok(())
    }

    pub fn compile(mut self) -> Result<Vec<VMToken>> {
        self.populate_types()?;
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
