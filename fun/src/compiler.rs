use core::panic;
use getset::Getters;
use std::collections::HashMap;
use std::usize;

use crate::ast::{AssignmentStatement, Block, ClassDecl, MethodDecl, Node, Scope, WhileStatement};
use crate::ast::{Expression, LetStatement, Module, Op, Statement, Term};
use anyhow::anyhow;
use anyhow::Result;
use hackvm::{VMSegment, VMToken};

#[derive(Default)]
struct Namespace {
    names: HashMap<String, MemRef>,
}
impl Namespace {
    fn segment_size(&self, segment: &VMSegment) -> usize {
        self.names
            .values()
            .filter(|v| &v.segment == segment)
            .count()
    }
    fn register(&mut self, name: &str, segment: &VMSegment, type_id: usize) -> Option<usize> {
        if self.names.contains_key(name) {
            None
        } else {
            let index = self.segment_size(segment);
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
    fn get(&self, name: &str) -> Option<MemRef> {
        self.names.get(name).copied()
    }
    fn clear(&mut self) {
        self.names.clear();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemRef {
    segment: VMSegment,
    index: usize,
    type_id: usize,
}
impl MemRef {
    pub fn as_pop_token(&self) -> VMToken {
        VMToken::Pop(self.segment, self.index as u16)
    }
    pub fn as_push_token(&self) -> VMToken {
        VMToken::Push(self.segment, self.index as u16)
    }
}

#[derive(Debug, Clone)]
pub struct OrderedMap<V> {
    key_map: HashMap<String, usize>,
    items: Vec<V>,
}
impl<V> OrderedMap<V> {
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.key_map.get(key).copied()
    }
    pub fn get_at(&self, index: usize) -> Option<&V> {
        self.items.get(index)
    }
    pub fn get(&self, key: &str) -> Option<&V> {
        self.index_of(key)
            .map(|index| self.items.get(index))
            .flatten()
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        match self.index_of(key) {
            Some(index) => self.items.get_mut(index),
            None => None,
        }
    }
    pub fn push(&mut self, key: &str, value: V) -> Result<()> {
        if self.key_map.contains_key(key) {
            Err(anyhow!("key {} was already pushed", key))
        } else {
            let index = self.items.len();
            self.items.push(value);
            self.key_map.insert(key.to_string(), index);
            Ok(())
        }
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
}
impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        OrderedMap {
            key_map: HashMap::new(),
            items: Vec::new(),
        }
    }
}

pub use types::*;
mod types {
    use super::*;

    #[derive(Debug, Clone, Getters)]
    pub struct ObjectTypeField {
        #[getset(get = "pub")]
        type_id: usize,
        #[getset(get = "pub")]
        index: usize,
    }
    #[derive(Debug, Default)]
    pub struct ObjectType {
        fields: OrderedMap<ObjectTypeField>,
    }
    impl ObjectType {
        pub fn get_field(&self, field_name: &str) -> Option<&ObjectTypeField> {
            self.fields.get(field_name)
        }
        pub fn add_field(&mut self, name: &str, type_id: usize) -> Result<()> {
            let index = self.fields.len();
            let field = ObjectTypeField { type_id, index };
            self.fields
                .push(name, field)
                .map_err(|_| anyhow!("field {} already declared", name))
        }
    }

    #[derive(Debug, Default)]
    pub struct ObjectTypeTable {
        types: OrderedMap<ObjectType>,
    }
    impl ObjectTypeTable {
        pub fn get_mut(&mut self, name: &str) -> Option<&mut ObjectType> {
            self.types.get_mut(name)
        }
        pub fn get(&self, name: &str) -> Option<&ObjectType> {
            self.types.get(name)
        }
        pub fn get_by_id(&self, id: usize) -> Option<&ObjectType> {
            self.types.get_at(id)
        }
        pub fn add_type(&mut self, name: &str, obj_type: ObjectType) -> Result<()> {
            self.types
                .push(name, obj_type)
                .map_err(|_| anyhow!("Type {} already declared", name))
        }
        pub fn id_for_type(&self, name: &str) -> Option<usize> {
            self.types.index_of(name)
        }
    }
}

pub use module::ModuleCompiler;

mod module {
    use super::*;

    #[derive(Default)]
    struct StaticsTable {
        index: usize,
        static_names: HashMap<String, HashMap<String, MemRef>>,
    }

    impl StaticsTable {
        pub fn insert(
            &mut self,
            class_name: &str,
            field_name: &str,
            type_id: usize,
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

    pub struct ModuleCompiler<'m> {
        statics_table: StaticsTable,
        object_types: ObjectTypeTable,
        module: &'m Module,
    }

    impl<'m> ModuleCompiler<'m> {
        pub fn new(module: &Module) -> ModuleCompiler {
            ModuleCompiler {
                statics_table: StaticsTable::default(),
                object_types: ObjectTypeTable::default(),
                module,
            }
        }

        pub fn get_static_field(&self, class_name: &str, field_name: &str) -> Option<MemRef> {
            self.statics_table.get(class_name, field_name)
        }

        pub fn get_object_types(&self) -> &ObjectTypeTable {
            &self.object_types
        }

        pub fn resolve_type(&self, type_name: &str) -> Result<usize> {
            let field_type_id = match self.object_types.id_for_type(type_name) {
                Some(id) => id,
                None => return Err(anyhow!("{} is not a known type", type_name)),
            };
            Ok(field_type_id)
        }

        pub fn compile(mut self) -> Result<Vec<VMToken>> {
            // start by adding built-in types
            for type_name in &["number", "bool"] {
                self.object_types
                    .add_type(type_name, ObjectType::default())?;
            }

            for class_decl in self.module.classes() {
                self.object_types
                    .add_type(class_decl.data().name(), ObjectType::default())?;
            }

            for class_decl in self.module.classes() {
                for field in class_decl.fields() {
                    let name = field.data().name();
                    let type_name = field.data().type_name();
                    let field_type_id = self.resolve_type(type_name)?;
                    match field.data().scope() {
                        Scope::Static => {
                            if let Some(_) = self.statics_table.insert(
                                class_decl.data().name(),
                                name,
                                field_type_id,
                            ) {
                                return Err(anyhow!("Static field \"{}\" declared twice", name));
                            }
                        }
                        Scope::Instance => {
                            let obj_type = self
                                .object_types
                                .get_mut(class_decl.data().name())
                                .expect("object types should be found");
                            obj_type.add_field(name, field_type_id)?;
                        }
                    }
                }
            }

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
}

mod class {
    use super::*;
    pub struct ClassDeclCompiler<'module> {
        module_compiler: &'module ModuleCompiler<'module>,
        class_decl: &'module Node<ClassDecl>,
        instance_names: Namespace,
    }
    impl<'module> ClassDeclCompiler<'module> {
        pub fn new(
            module_compiler: &'module ModuleCompiler,
            class_decl: &'module Node<ClassDecl>,
        ) -> ClassDeclCompiler<'module> {
            ClassDeclCompiler {
                module_compiler,
                class_decl,
                instance_names: Namespace::default(),
            }
        }

        pub fn module_compiler(&self) -> &ModuleCompiler {
            self.module_compiler
        }

        pub fn get_instance_field(&self, field_name: &str) -> Option<MemRef> {
            self.instance_names.get(field_name)
        }

        pub fn get_num_instance_fields(&self) -> usize {
            self.instance_names.segment_size(&VMSegment::This)
        }

        pub fn get_class_name(&self) -> &str {
            self.class_decl.name()
        }

        pub fn compile(&mut self) -> Result<Vec<VMToken>> {
            self.instance_names.clear();
            for field in self.class_decl.fields() {
                let name = field.data().name();
                match field.data().scope() {
                    Scope::Static => {
                        // handled at the module compilation level
                    }
                    Scope::Instance => {
                        let index = self.instance_names.register(
                            name,
                            &VMSegment::This,
                            self.module_compiler
                                .resolve_type(field.data().type_name())?,
                        );
                        if index.is_none() {
                            return Err(anyhow!("Instance field \"{}\" declared twice", name));
                        }
                    }
                }
            }
            let mut commands: Vec<VMToken> = Vec::new();
            if let Some(constructor) = self.class_decl.data().constructor() {
                commands.append(&mut MethodDeclCompiler::constructor(self, constructor)?);
            }
            for method in self.class_decl.methods() {
                commands.append(&mut MethodDeclCompiler::method(self, method)?);
            }
            Ok(commands)
        }
    }
}

use class::ClassDeclCompiler;
pub struct MethodDeclCompiler<'class> {
    class_compiler: &'class ClassDeclCompiler<'class>,
    method: &'class MethodDecl,
    local_names: Namespace,
}
impl<'class> MethodDeclCompiler<'class> {
    fn new(
        class_compiler: &'class ClassDeclCompiler,
        method: &'class MethodDecl,
    ) -> MethodDeclCompiler<'class> {
        MethodDeclCompiler {
            class_compiler,
            method,
            local_names: Namespace::default(),
        }
    }

    pub fn constructor(
        class_compiler: &'class ClassDeclCompiler,
        constructor: &'class MethodDecl,
    ) -> Result<Vec<VMToken>> {
        Self::new(class_compiler, constructor).compile_constructor()
    }

    pub fn method(
        class_compiler: &'class ClassDeclCompiler,
        method: &'class MethodDecl,
    ) -> Result<Vec<VMToken>> {
        Self::new(class_compiler, method).compile_method()
    }

    fn module_compiler(&self) -> &ModuleCompiler {
        self.class_compiler.module_compiler()
    }

    fn start_method(&mut self) -> Result<(Vec<VMToken>, usize)> {
        for parameter in self.method.parameters() {
            self.local_names.register(
                parameter.name(),
                &VMSegment::Argument,
                self.module_compiler().resolve_type(parameter.type_name())?,
            );
        }

        let block_tokens = self.compile_block(self.method.block())?;

        let num_locals = self.local_names.segment_size(&VMSegment::Local);
        Ok((block_tokens, num_locals))
    }

    fn compile_constructor(&mut self) -> Result<Vec<VMToken>> {
        let (block_tokens, num_locals) = self.start_method()?;
        let num_instance_fields = self.class_compiler.get_num_instance_fields();
        let mut commands = vec![
            VMToken::Function(
                format!("{}.new", self.class_compiler.get_class_name()),
                num_locals as u16,
            ),
            VMToken::Push(VMSegment::Constant, num_instance_fields as u16),
            VMToken::Call("Memory.alloc".to_string(), 1),
            VMToken::Pop(VMSegment::Pointer, 0),
        ];
        commands.append(&mut block_tokens.into());
        commands.push(VMToken::Push(VMSegment::Pointer, 0));
        commands.push(VMToken::Return);
        Ok(commands)
    }

    fn compile_method(&mut self) -> Result<Vec<VMToken>> {
        let (block_tokens, num_locals) = self.start_method()?;

        let mut commands = vec![VMToken::Function(
            format!(
                "{}.{}",
                self.class_compiler.get_class_name(),
                self.method.name()
            ),
            num_locals as u16,
        )];
        if self.method.scope() == &Scope::Instance {
            commands.push(VMToken::Push(VMSegment::Argument, 0));
            commands.push(VMToken::Pop(VMSegment::Pointer, 0));
        }
        commands.append(&mut block_tokens.into());
        Ok(commands)
    }

    fn compile_let_statement(&mut self, let_statement: &LetStatement) -> Result<Vec<VMToken>> {
        let name = let_statement.name();
        let index = self.local_names.register(
            name,
            &VMSegment::Local,
            self.module_compiler()
                .resolve_type(let_statement.type_name())?,
        );
        if let Some(index) = index {
            let mut tokens = self.compile_expression(let_statement.value_expr())?;
            tokens.push(VMToken::Pop(VMSegment::Local, index as u16));
            return Ok(tokens);
        } else {
            return Err(anyhow!(
                "a variable with the name \"{}\" has already been declared",
                name
            ));
        }
    }

    fn compile_assignment_statement(
        &mut self,
        assignment_statement: &AssignmentStatement,
    ) -> Result<Vec<VMToken>> {
        let mut tokens = self.compile_expression(assignment_statement.value_expr())?;
        let dest_term = assignment_statement.dest_expr().term();
        let pop_token: VMToken = match dest_term {
            Term::BinaryOp(Op::Dot, left, right) => {
                if let Some(left_identifier) = left.as_identifer() {
                    if let Some(field_name) = right.as_identifer() {
                        if left_identifier == "this" {
                            if let Some(mem_ref) =
                                self.class_compiler.get_instance_field(field_name)
                            {
                                mem_ref.as_pop_token()
                            } else {
                                return Err(anyhow!(
                                    "instance field \"{}\" is not declared",
                                    field_name
                                ));
                            }
                        } else if let Some(mem_ref) = self
                            .module_compiler()
                            .get_static_field(left_identifier, field_name)
                        {
                            mem_ref.as_pop_token()
                        } else {
                            panic!("Not sure how to assign to {:?}.{:?}", left, right);
                        }
                    } else {
                        panic!("Not sure how to assign to {:?}.{:?}", left, right);
                    }
                } else {
                    panic!("Not sure how to assign to {:?}.{:?}", left, right);
                }
            }
            Term::Identifier(name) => {
                if let Some(mem_ref) = self.local_names.get(name) {
                    mem_ref.as_pop_token()
                } else {
                    return Err(anyhow!("variable \"{}\" has never been declared", name));
                }
            }
            _ => {
                panic!("Don't know how to resolve term {:?}", dest_term)
            }
        };
        tokens.push(pop_token);
        Ok(tokens)
    }

    fn compile_while_statement(
        &mut self,
        while_statement: &WhileStatement,
    ) -> Result<Vec<VMToken>> {
        let start_label = "WHILE".to_string();
        let end_label = "WHILE_END".to_string();
        let mut tokens = vec![VMToken::Label(start_label.clone())];
        tokens.append(&mut self.compile_expression(while_statement.condition_expr())?);
        tokens.push(VMToken::Not);
        tokens.push(VMToken::If(end_label.clone()));
        tokens.append(&mut self.compile_block(while_statement.block())?);
        tokens.push(VMToken::Goto(start_label.clone()));
        tokens.push(VMToken::Label(end_label));
        return Ok(tokens);
    }

    fn compile_block(&mut self, block: &Block) -> Result<Vec<VMToken>> {
        let mut commands = Vec::new();
        for statement in block.statements() {
            match statement {
                Statement::Return(expression) => {
                    for command in self.compile_expression(expression)? {
                        commands.push(command);
                    }
                    commands.push(VMToken::Return);
                }
                Statement::Let(let_statement) => {
                    commands.append(&mut self.compile_let_statement(let_statement)?);
                }
                Statement::While(while_statement) => {
                    commands.append(&mut self.compile_while_statement(while_statement)?);
                }
                Statement::Assignment(assignment_statement) => {
                    commands.append(&mut self.compile_assignment_statement(assignment_statement)?);
                }
                _ => panic!("Can't handle {:?}", statement),
            }
        }
        Ok(commands)
    }

    fn compile_reference(&mut self, reference: &str) -> Result<Vec<VMToken>> {
        if let Some(mem_ref) = self.local_names.get(reference) {
            return Ok(vec![mem_ref.as_push_token()]);
        }
        Err(anyhow!(
            "variable \"{}\" has not been declared with a let statement",
            reference
        ))
    }

    fn compile_term(&mut self, term: &Term) -> Result<Vec<VMToken>> {
        match term {
            Term::Number(num) => return Ok(vec![VMToken::Push(VMSegment::Constant, *num as u16)]),
            Term::BinaryOp(op, left, right) => self.compile_binary_op(op, left, right),
            Term::Identifier(name) => self.compile_reference(name),
            Term::New(class_name, arguments) => self.compile_call(class_name, "new", arguments),
            _ => panic!("Don't know how to compile {:?}", term),
        }
    }

    fn compile_call(
        &mut self,
        class_name: &str,
        func_name: &str,
        arguments: &[Expression],
    ) -> Result<Vec<VMToken>> {
        let mut tokens: Vec<VMToken> = Vec::new();
        for expression in arguments {
            tokens.append(&mut self.compile_expression(expression)?);
        }
        tokens.push(VMToken::Call(
            format!("{}.{}", class_name, func_name),
            arguments.len() as u16,
        ));
        return Ok(tokens);
    }

    fn compile_dot_op(&mut self, left: &Term, right: &Term) -> Result<Vec<VMToken>> {
        match left {
            Term::Identifier(left_identifier) => match &left_identifier[..] {
                "this" => match right {
                    Term::Identifier(instance_field_name) => {
                        match self.class_compiler.get_instance_field(instance_field_name) {
                            Some(mem_ref) => Ok(vec![mem_ref.as_push_token()]),
                            None => Err(anyhow!(
                                "instance field \"{}\" has not been declared",
                                instance_field_name
                            )),
                        }
                    }
                    Term::Call(_func_name, _arguments) => {
                        todo!("Not sure how to call instance methods yet");
                    }
                    _ => {
                        todo!("Not sure how to deal with this.{:?}", right);
                    }
                },
                left_identifier => {
                    // first try local variables
                    match self.local_names.get(left_identifier) {
                        Some(left_mem_ref) => {
                            // we're doing an instance field lookup on a local/argument variable
                            // that must be a pointer, so update the That segment to point to it.
                            let mut tokens = vec![
                                left_mem_ref.as_push_token(),
                                VMToken::Pop(VMSegment::Pointer, 1),
                            ];
                            // now we need to resolve the field based on the type that it is.
                            let left_obj_type = self
                                .module_compiler()
                                .get_object_types()
                                .get_by_id(left_mem_ref.type_id)
                                .expect("wasn't able to get ObjectType from MemRef");
                            let dest = match right {
                                Term::Identifier(instance_field_name) => {
                                    let instance_field =
                                        match left_obj_type.get_field(instance_field_name) {
                                            Some(field) => field,
                                            None => {
                                                return Err(anyhow!(
                                                    "Field {} does not exist on {}",
                                                    instance_field_name,
                                                    left_identifier
                                                ))
                                            }
                                        };
                                    VMToken::Push(VMSegment::That, *instance_field.index() as u16)
                                }
                                _ => {
                                    todo!(
                                        "Don't know how to resolve instance field lookup {:?}",
                                        right
                                    )
                                }
                            };
                            tokens.push(dest);
                            Ok(tokens)
                        }
                        None => {
                            // we're doing a static field lookup on a class
                            let tokens = match right {
                                Term::Identifier(static_field_name) => {
                                    if let Some(mem_ref) = self
                                        .module_compiler()
                                        .get_static_field(left_identifier, static_field_name)
                                    {
                                        vec![mem_ref.as_push_token()]
                                    } else {
                                        panic!(
                                    "Not sure how to resolve identifier lookup {:?} dot {:?}",
                                    left, right
                                );
                                    }
                                }
                                Term::Call(func_name, arguments) => {
                                    self.compile_call(left_identifier, func_name, arguments)?
                                }
                                _ => panic!("Not sure what to do with {:?} dot {:?}", left, right),
                            };
                            Ok(tokens)
                        }
                    }
                }
            },
            _ => {
                panic!("Not sure how to resolve {:?} dot {:?}", left, right);
            }
        }
    }

    fn compile_binary_op(&mut self, op: &Op, left: &Term, right: &Term) -> Result<Vec<VMToken>> {
        if op == &Op::Dot {
            return self.compile_dot_op(left, right);
        }
        let mut tokens = self.compile_term(left)?;
        tokens.append(&mut self.compile_term(right)?);
        let op_token = match op {
            Op::Plus => VMToken::Add,
            Op::Sub => VMToken::Sub,
            Op::Lt => VMToken::Lt,
            Op::Gt => VMToken::Gt,
            Op::Eq => VMToken::Eq,
            _ => todo!("Don't know how to handle op {:?}", op),
        };
        tokens.push(op_token);
        Ok(tokens)
    }

    pub fn compile_expression(&mut self, expression: &Expression) -> Result<Vec<VMToken>> {
        self.compile_term(expression.term())
    }
}

#[cfg(test)]
mod tests {
    use super::module::*;
    use super::*;
    use crate::parse_module;

    #[test]
    fn test_simplest_program() {
        let module = parse_module(
            "
            class Main {
                static main(): number {
                    return 3+4-1;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 0),
                VMToken::Push(VMSegment::Constant, 3),
                VMToken::Push(VMSegment::Constant, 4),
                VMToken::Add,
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Sub,
                VMToken::Return
            ]
        )
    }

    #[test]
    fn test_static_vars() {
        let module = parse_module(
            "
            class Main {
                static sum: number;

                static main(): number {
                    Main.sum = 3;
                    return Main.sum;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 0),
                VMToken::Push(VMSegment::Constant, 3),
                VMToken::Pop(VMSegment::Static, 0),
                VMToken::Push(VMSegment::Static, 0),
                VMToken::Return
            ]
        )
    }

    #[test]
    fn test_function_parameters() {
        let module = parse_module(
            "
            class Math {
                static add(a: number, b: number): number {
                    return a + b;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Math.add".to_string(), 0),
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Push(VMSegment::Argument, 1),
                VMToken::Add,
                VMToken::Return
            ]
        );
    }

    #[test]
    fn test_function_calls() {
        let module = parse_module(
            "
            class Math {
                static square(a: number): number {
                    return Math.add(a, a);
                }
                static add(a: number, b: number): number {
                    return a + b;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Math.square".to_string(), 0),
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Call("Math.add".to_string(), 2),
                VMToken::Return,
                VMToken::Function("Math.add".to_string(), 0),
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Push(VMSegment::Argument, 1),
                VMToken::Add,
                VMToken::Return,
            ]
        );
    }

    #[test]
    fn test_new_expr() {
        let module = parse_module(
            "
            class Counter {
                static create(): Vector {
                    return new Counter();
                }
                n: number;
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Counter.create".to_string(), 0),
                VMToken::Call("Counter.new".to_string(), 0),
                VMToken::Return,
            ]
        );
    }

    #[test]
    fn test_constructor() {
        let module = parse_module(
            "
            class Vector {
                x: number;
                y: number;
                constructor(x: number, y: number) {
                    this.x = x;
                    this.y = y;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Vector.new".to_string(), 0),
                // allocation for this
                VMToken::Push(VMSegment::Constant, 2),
                VMToken::Call("Memory.alloc".to_string(), 1),
                VMToken::Pop(VMSegment::Pointer, 0),
                // initialization
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Pop(VMSegment::This, 0),
                VMToken::Push(VMSegment::Argument, 1),
                VMToken::Pop(VMSegment::This, 1),
                // implicit return this
                VMToken::Push(VMSegment::Pointer, 0),
                VMToken::Return,
            ]
        );
    }

    #[test]
    fn test_this_resolution() {
        let module = parse_module(
            "
            class Vector {
                x: number;
                y: number;
                getY(): number {
                    return this.y;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Vector.getY".to_string(), 0),
                // implicit this segment
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Pop(VMSegment::Pointer, 0),
                // resolution of this.y
                VMToken::Push(VMSegment::This, 1),
                // return
                VMToken::Return,
            ]
        );
    }

    #[test]
    fn test_dot_precedence() {
        let module = parse_module(
            "
            class Counter {
                static count: number;
                static atTheEnd(): void {
                    return Counter.count == 10;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Counter.atTheEnd".to_string(), 0),
                VMToken::Push(VMSegment::Static, 0),
                VMToken::Push(VMSegment::Constant, 10),
                VMToken::Eq,
                VMToken::Return,
            ]
        );
    }

    #[test]
    fn test_dot_resolution_for_local_vars() {
        let module = parse_module(
            "
            class Vector {
                static add(v1: Vector, v2: Vector): Vector {
                    return new Vector(v1.x+v2.x, v1.y+v2.y);
                }

                x: number;
                y: number;
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Vector.add".to_string(), 0),
                // push v1.x
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Pop(VMSegment::Pointer, 1),
                VMToken::Push(VMSegment::That, 0),
                // push v2.x
                VMToken::Push(VMSegment::Argument, 1),
                VMToken::Pop(VMSegment::Pointer, 1),
                VMToken::Push(VMSegment::That, 0),
                // v1.x + v2.x
                VMToken::Add,
                // push v1.y
                VMToken::Push(VMSegment::Argument, 0),
                VMToken::Pop(VMSegment::Pointer, 1),
                VMToken::Push(VMSegment::That, 1),
                // push v2.y
                VMToken::Push(VMSegment::Argument, 1),
                VMToken::Pop(VMSegment::Pointer, 1),
                VMToken::Push(VMSegment::That, 1),
                // v1.y + v2.y
                VMToken::Add,
                VMToken::Call("Vector.new".to_string(), 2),
                // return
                VMToken::Return,
            ]
        );
    }

    #[test]
    fn test_loop() {
        let module = parse_module(
            "
            class Main {
                static main(): number {
                    let i: number = 0;
                    let sum: number = 0;
                    while (i < 10) {
                        i = i + 1;
                        sum = sum + sum;
                    }
                    return sum;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 2),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 1),
                VMToken::Label("WHILE".to_string()),
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 10),
                VMToken::Lt,
                VMToken::Not,
                VMToken::If("WHILE_END".to_string()),
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Local, 1),
                VMToken::Push(VMSegment::Local, 1),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 1),
                VMToken::Goto("WHILE".to_string()),
                VMToken::Label("WHILE_END".to_string()),
                VMToken::Push(VMSegment::Local, 1),
                VMToken::Return
            ]
        )
    }
}
