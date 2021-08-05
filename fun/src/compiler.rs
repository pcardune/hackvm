use core::panic;
use getset::Getters;
use std::collections::HashMap;
use std::usize;

use crate::ast::{
    AssignmentStatement, BinaryOp, Block, ClassDecl, IfStatement, MethodDecl, Node, Scope, UnaryOp,
    WhileStatement,
};
use crate::ast::{Expression, LetStatement, Module, Op, Statement, Term};
use anyhow::Result;
use anyhow::{anyhow, Context};
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
    #[derive(Debug, Getters, Clone)]
    pub struct ObjectType {
        #[getset(get = "pub")]
        name: String,
        fields: OrderedMap<ObjectTypeField>,
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
        #[allow(dead_code)]
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
    use anyhow::Context;

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
            // TODO: make types support generics and use that
            // instead of number[]
            for type_name in &["number", "bool", "number[]", "string"] {
                self.object_types
                    .add_type(type_name, ObjectType::new(type_name))?;
            }

            for class_decl in self.module.classes() {
                self.object_types.add_type(
                    class_decl.data().name(),
                    ObjectType::new(class_decl.data().name()),
                )?;
            }

            for class_decl in self.module.classes() {
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
    use anyhow::Context;

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
                                .resolve_type(field.data().type_name())
                                .with_context(|| format!("Could not resolve type {} for instance field {} in class {}", field.data().type_name(), field.data().name(), self.class_decl.name()))?,
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
    while_count: usize,
    if_count: usize,
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
            while_count: 0,
            if_count: 0,
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
                self.module_compiler()
                    .resolve_type(parameter.type_name())
                    .with_context(|| {
                        format!(
                            "Could not resolve type name {} for parameter {} in method {}",
                            parameter.type_name(),
                            parameter.name(),
                            self.method.name()
                        )
                    })?,
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
        // add an implicit return is there wasn't an explicit one
        match commands.last() {
            Some(VMToken::Return) => {}
            _ => {
                commands.push(VMToken::Push(VMSegment::Constant, 0));
                commands.push(VMToken::Return);
            }
        }
        Ok(commands)
    }

    fn compile_let_statement(&mut self, let_statement: &LetStatement) -> Result<Vec<VMToken>> {
        let name = let_statement.name();
        let index = self.local_names.register(
            name,
            &VMSegment::Local,
            self.module_compiler()
                .resolve_type(let_statement.type_name())
                .with_context(|| {
                    format!(
                        "Could not resolve type name {} for let statement {}",
                        let_statement.type_name(),
                        let_statement.name(),
                    )
                })?,
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
        let mut dest_tokens: Vec<VMToken> = match dest_term {
            Term::BinaryOp(binop) if *binop.op() == Op::Dot => {
                let left = binop.left();
                let right = binop.right();
                if let Some(left_identifier) = left.as_identifer() {
                    if let Some(field_name) = right.as_identifer() {
                        if left_identifier == "this" {
                            if let Some(mem_ref) =
                                self.class_compiler.get_instance_field(field_name)
                            {
                                vec![mem_ref.as_pop_token()]
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
                            vec![mem_ref.as_pop_token()]
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
                    vec![mem_ref.as_pop_token()]
                } else {
                    return Err(anyhow!("variable \"{}\" has never been declared", name));
                }
            }
            Term::Indexing(identifier_expr, index_expr) => {
                let mut tokens = self.compile_indexing_pointer(identifier_expr, index_expr)?;
                tokens.push(VMToken::Pop(VMSegment::That, 0));
                tokens
            }
            _ => {
                panic!("Don't know how to resolve term {:?}", dest_term)
            }
        };
        tokens.append(&mut dest_tokens);
        Ok(tokens)
    }

    fn compile_if_statement(&mut self, if_statement: &IfStatement) -> Result<Vec<VMToken>> {
        let end_label = format!("IF_{}_END", self.if_count);
        let else_end_label = format!("IF_{}_ELSE_END", self.if_count);
        self.if_count += 1;

        // condition check
        let mut tokens = self.compile_expression(if_statement.condition_expr())?;
        tokens.push(VMToken::Not);
        tokens.push(VMToken::If(end_label.clone()));
        // if block
        tokens.append(&mut self.compile_block(if_statement.block())?);
        if if_statement.else_block().is_some() {
            tokens.push(VMToken::Goto(else_end_label.clone()))
        }
        // else block
        tokens.push(VMToken::Label(end_label));
        if let Some(else_block) = if_statement.else_block() {
            tokens.append(&mut self.compile_block(else_block)?);
            tokens.push(VMToken::Label(else_end_label));
        }
        Ok(tokens)
    }

    fn compile_while_statement(
        &mut self,
        while_statement: &WhileStatement,
    ) -> Result<Vec<VMToken>> {
        let start_label = format!("WHILE_{}", self.while_count);
        self.while_count += 1;
        let end_label = format!("{}_END", start_label);
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
                Statement::Expr(expression) => {
                    commands.append(&mut self.compile_expression(expression)?);
                    commands.push(VMToken::Pop(VMSegment::Temp, 0));
                }
                Statement::If(if_statement) => {
                    commands.append(&mut self.compile_if_statement(if_statement)?);
                }
            }
        }
        Ok(commands)
    }

    fn compile_reference(&mut self, reference: &str) -> Result<Vec<VMToken>> {
        if let Some(mem_ref) = self.local_names.get(reference) {
            return Ok(vec![mem_ref.as_push_token()]);
        }
        if reference == "this" {
            return Ok(vec![VMToken::Push(VMSegment::Pointer, 0)]);
        }
        Err(anyhow!(
            "variable \"{}\" has not been declared with a let statement",
            reference
        ))
    }

    fn compile_term(&mut self, term: &Term) -> Result<Vec<VMToken>> {
        match term {
            Term::Bool(bool) => match bool {
                false => Ok(vec![VMToken::Push(VMSegment::Constant, 0)]),
                true => Ok(vec![VMToken::Push(VMSegment::Constant, 0xffff)]),
            },
            Term::Number(num) => return Ok(vec![VMToken::Push(VMSegment::Constant, *num as u16)]),
            Term::BinaryOp(binop) => self.compile_binary_op(binop),
            Term::UnaryOp(op, operand) => self.compile_unary_op(op, operand),
            Term::Identifier(name) => self.compile_reference(name),
            Term::New(class_name, arguments) => self.compile_call(class_name, "new", arguments),
            Term::String(ascii) => self.compile_string_constant(ascii),
            Term::Indexing(identifer_expr, index_expr) => {
                self.compile_indexing_expr(identifer_expr, index_expr)
            }
            Term::Array(expressions) => self.compile_array_literal(expressions),
            Term::Expr(expression) => self.compile_expression(expression),
            _ => panic!("Don't know how to compile {:?}", term),
        }
    }

    fn compile_indexing_pointer(
        &mut self,
        identifier_expr: &Expression,
        index_expr: &Expression,
    ) -> Result<Vec<VMToken>> {
        let mut tokens = self.compile_expression(index_expr)?;
        tokens.extend(self.compile_expression(identifier_expr)?);
        tokens.push(VMToken::Add);
        tokens.push(VMToken::Pop(VMSegment::Pointer, 1));
        Ok(tokens)
    }

    fn compile_indexing_expr(
        &mut self,
        identifier_expr: &Expression,
        index_expr: &Expression,
    ) -> Result<Vec<VMToken>> {
        let mut tokens = self.compile_indexing_pointer(identifier_expr, index_expr)?;
        tokens.push(VMToken::Push(VMSegment::That, 0));
        Ok(tokens)
    }

    fn compile_array_literal(&mut self, elements: &Vec<Expression>) -> Result<Vec<VMToken>> {
        let mut tokens = vec![];
        for expr in elements {
            tokens.extend(self.compile_expression(expr)?);
        }

        tokens.extend(vec![
            VMToken::Push(VMSegment::Constant, elements.len() as u16),
            VMToken::Call("Array.new".to_string(), 1),
        ]);

        // copy array address into THAT
        tokens.push(VMToken::Pop(VMSegment::Pointer, 1));

        // pop values into array
        for i in 0..elements.len() {
            tokens.push(VMToken::Pop(
                VMSegment::That,
                (elements.len() - 1 - i) as u16,
            ));
        }

        // push pointer to array back onto the stack
        tokens.push(VMToken::Push(VMSegment::Pointer, 1));

        Ok(tokens)
    }

    fn compile_string_constant(&mut self, ascii: &str) -> Result<Vec<VMToken>> {
        let mut tokens = vec![
            VMToken::Push(VMSegment::Constant, ascii.len() as u16),
            VMToken::Call("String.new".to_string(), 1),
        ];

        for &char in ascii.as_bytes() {
            tokens.push(VMToken::Push(VMSegment::Constant, char as u16));
            tokens.push(VMToken::Call("String.appendChar".to_string(), 2));
        }

        Ok(tokens)
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

    fn compile_method_call(
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
            arguments.len() as u16 + 1,
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
                                .expect("wasn't able to get ObjectType from MemRef")
                                .clone(); // TODO: see about removing this clone?
                            let mut dest = match right {
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
                                    vec![VMToken::Push(
                                        VMSegment::That,
                                        *instance_field.index() as u16,
                                    )]
                                }
                                Term::Call(func_name, arguments) => {
                                    let mut tokens = vec![VMToken::Push(VMSegment::Pointer, 1)];
                                    tokens.append(&mut self.compile_method_call(
                                        left_obj_type.name(),
                                        func_name,
                                        arguments,
                                    )?);
                                    tokens
                                }
                                _ => {
                                    todo!(
                                        "Don't know how to resolve instance field lookup {:?}",
                                        right
                                    )
                                }
                            };
                            tokens.append(&mut dest);
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

    fn compile_unary_op(&mut self, op: &UnaryOp, operand: &Term) -> Result<Vec<VMToken>> {
        let mut tokens = self.compile_term(operand)?;
        tokens.push(match op {
            UnaryOp::BitNot | UnaryOp::Not => VMToken::Not,
            UnaryOp::Neg => VMToken::Neg,
        });
        Ok(tokens)
    }

    fn compile_binary_op(&mut self, binop: &BinaryOp) -> Result<Vec<VMToken>> {
        // op: &Op, left: &Term, right: &Term
        if binop.op() == &Op::Dot {
            return self.compile_dot_op(binop.left(), binop.right());
        }
        let mut tokens = self.compile_term(binop.left())?;
        tokens.append(&mut self.compile_term(binop.right())?);
        let op_token = match binop.op() {
            Op::Plus => VMToken::Add,
            Op::Sub => VMToken::Sub,
            Op::Lt => VMToken::Lt,
            Op::Gt => VMToken::Gt,
            Op::Eq => VMToken::Eq,
            Op::Multiply => VMToken::Call("Math.multiply".to_string(), 2),
            Op::Divide => VMToken::Call("Math.divide".to_string(), 2),
            Op::And => VMToken::And,
            Op::Or => VMToken::Or,
            Op::BitAnd => VMToken::And,
            Op::BitOr => VMToken::Or,
            _ => todo!("Don't know how to handle op {:?}", binop.op()),
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
    use std::cmp::min;

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
    fn test_array_assignment() {
        let module = parse_module(
            "
            class Main {
                static main(): void {
                    let ram: number[] = 2048;
                    ram[5] = 3;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 1),
                // set ram to point to 2048.
                VMToken::Push(VMSegment::Constant, 2048),
                VMToken::Pop(VMSegment::Local, 0),
                // now execute:
                //   ram[5] = 3
                // which is the same as
                //   *(ram+5) = 3
                // step 1: calculate the right hand side of assignment
                VMToken::Push(VMSegment::Constant, 3),
                // step 2: calculate left hand side of assignment
                // into THAT pointer
                VMToken::Push(VMSegment::Constant, 5),
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Add,
                VMToken::Pop(VMSegment::Pointer, 1),
                // step 3: pop right hand side into THAT pointer
                VMToken::Pop(VMSegment::That, 0),
                // implicit return
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Return,
            ]
        )
    }

    #[test]
    fn test_array_indexing() {
        let module = parse_module(
            "
            class Main {
                static main(): string {
                    let ram: number[] = 0;
                    return ram[5];
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 1),
                // set ram to point to 0.
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 0),
                // now execute:
                //   return ram[5]
                // which is the same as
                //   return *(ram+5)
                VMToken::Push(VMSegment::Constant, 5),
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Add,
                // now deref
                VMToken::Pop(VMSegment::Pointer, 1),
                // now return
                VMToken::Push(VMSegment::That, 0),
                VMToken::Return,
            ]
        )
    }

    #[test]
    fn test_string_values() {
        let module = parse_module(
            "
            class Main {
                static main(): string {
                    return \"hello\";
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
                // construct string
                VMToken::Push(VMSegment::Constant, 5),
                VMToken::Call("String.new".to_string(), 1),
                // append to string
                VMToken::Push(VMSegment::Constant, 'h' as u16),
                VMToken::Call("String.appendChar".to_string(), 2),
                VMToken::Push(VMSegment::Constant, 'e' as u16),
                VMToken::Call("String.appendChar".to_string(), 2),
                VMToken::Push(VMSegment::Constant, 'l' as u16),
                VMToken::Call("String.appendChar".to_string(), 2),
                VMToken::Push(VMSegment::Constant, 'l' as u16),
                VMToken::Call("String.appendChar".to_string(), 2),
                VMToken::Push(VMSegment::Constant, 'o' as u16),
                VMToken::Call("String.appendChar".to_string(), 2),
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
    fn test_unary_operators() {
        let module = parse_module(
            "
            class Foo {
                static bit_not():number  {
                    return ~1;
                }
                static not(): boolean {
                    return !false;
                }
                static neg(): number {
                    return -10;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_array_eq(
            &vmcode,
            &[
                // bitwise not operator
                VMToken::Function("Foo.bit_not".to_string(), 0),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Not,
                VMToken::Return,
                // not operator
                VMToken::Function("Foo.not".to_string(), 0),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Not,
                VMToken::Return,
                // negate operator
                VMToken::Function("Foo.neg".to_string(), 0),
                VMToken::Push(VMSegment::Constant, 10),
                VMToken::Neg,
                VMToken::Return,
            ],
        );
    }

    #[test]
    fn test_binary_operators() {
        let module = parse_module(
            "
            class Foo {
                static foo():boolean  {
                    let a:number = false || true;
                    let b:number = 1+2*3/5;
                    let c:number = 1 & 2 | 4;
                    return 0 < 1 && 1 > 0 || 3 > 4;
                }
            }
        ",
        )
        .unwrap();
        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_array_eq(
            &vmcode,
            &[
                VMToken::Function("Foo.foo".to_string(), 3),
                // let a = ...
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Push(VMSegment::Constant, 65535),
                VMToken::Or,
                VMToken::Pop(VMSegment::Local, 0),
                // let b = ...
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Push(VMSegment::Constant, 2),
                VMToken::Push(VMSegment::Constant, 3),
                VMToken::Call("Math.multiply".to_string(), 2),
                VMToken::Push(VMSegment::Constant, 5),
                VMToken::Call("Math.divide".to_string(), 2),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 1),
                // let c = ...
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Push(VMSegment::Constant, 2),
                VMToken::And,
                VMToken::Push(VMSegment::Constant, 4),
                VMToken::Or,
                VMToken::Pop(VMSegment::Local, 2),
                // return ...
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Lt,
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Gt,
                VMToken::And,
                VMToken::Push(VMSegment::Constant, 3),
                VMToken::Push(VMSegment::Constant, 4),
                VMToken::Gt,
                VMToken::Or,
                VMToken::Return,
            ],
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
    fn test_expr_statement_without_return() {
        let module = parse_module(
            "
            class Main {
                static main(): void {
                    Output.printInt(1); // side effects
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
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Call("Output.printInt".to_string(), 1),
                VMToken::Pop(VMSegment::Temp, 0),
                VMToken::Push(VMSegment::Constant, 0),
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
    fn test_if() {
        let module = parse_module(
            "
            class Main {
                static main(): number {
                    let a: number = 1;
                    if (a < 10) {
                        a = a+1;
                    } else {
                        a = a-1;
                        if (true) {
                            a = 5;
                        }
                    }
                    return a;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_array_eq(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 1),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Pop(VMSegment::Local, 0),
                // if condition
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 10),
                VMToken::Lt,
                VMToken::Not,
                VMToken::If("IF_0_END".to_string()),
                // true block
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 0),
                VMToken::Goto("IF_0_ELSE_END".to_string()),
                // false block
                VMToken::Label("IF_0_END".to_string()),
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Sub,
                VMToken::Pop(VMSegment::Local, 0),
                // inner if condition
                VMToken::Push(VMSegment::Constant, 0xffff),
                VMToken::Not,
                VMToken::If("IF_1_END".to_string()),
                VMToken::Push(VMSegment::Constant, 5),
                VMToken::Pop(VMSegment::Local, 0),
                VMToken::Label("IF_1_END".to_string()),
                // end inner if condition
                VMToken::Label("IF_0_ELSE_END".to_string()),
                // return
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Return,
            ],
        );
    }

    fn assert_array_eq<T>(left: &[T], right: &[T])
    where
        T: std::fmt::Debug + std::fmt::Display + PartialEq,
    {
        use std::fmt::Write;
        let mut msg = String::new();

        for i in 0..min(left.len(), right.len()) {
            let marker = if left[i] != right[i] { "x" } else { " " };
            let leftstr = format!("{:?}", left[i]);
            let rightstr = format!("{:?}", right[i]);
            writeln!(msg, "  {} {:3}: {:.<20} | {}", marker, i, leftstr, rightstr).unwrap();
        }
        if left.len() != right.len() {
            writeln!(msg, "  x  there's more...").unwrap();
        }
        assert_eq!(left, right, "\n\n{}", msg);
    }

    #[test]
    fn test_loop() {
        let module = parse_module(
            "
            class Main {
                static main(): number {
                    let i: number = 0;
                    let j: number = 0;
                    let sum: number = 0;
                    while (i < 10) {
                        i = i + 1;
                        j = 0;
                        while (j < 10) {
                            j = j + 1;
                            sum = sum + sum;
                        }
                    }
                    return sum;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = ModuleCompiler::new(&module).compile().unwrap();
        assert_array_eq(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 3),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 1),
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 2),
                VMToken::Label("WHILE_0".to_string()),
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 10),
                VMToken::Lt,
                VMToken::Not,
                VMToken::If("WHILE_0_END".to_string()),
                // <outer while>
                //   i = i + 1;
                VMToken::Push(VMSegment::Local, 0),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 0),
                //   j = 0;
                VMToken::Push(VMSegment::Constant, 0),
                VMToken::Pop(VMSegment::Local, 1),
                //   while (j < 10) {
                VMToken::Label("WHILE_1".to_string()),
                VMToken::Push(VMSegment::Local, 1),
                VMToken::Push(VMSegment::Constant, 10),
                VMToken::Lt,
                VMToken::Not,
                VMToken::If("WHILE_1_END".to_string()),
                //     j = j + 1
                VMToken::Push(VMSegment::Local, 1),
                VMToken::Push(VMSegment::Constant, 1),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 1),
                //     sum = sum + sum
                VMToken::Push(VMSegment::Local, 2),
                VMToken::Push(VMSegment::Local, 2),
                VMToken::Add,
                VMToken::Pop(VMSegment::Local, 2),
                //   }
                VMToken::Goto("WHILE_1".to_string()),
                VMToken::Label("WHILE_1_END".to_string()),
                //   </inner while>
                // </outer while>
                VMToken::Goto("WHILE_0".to_string()),
                VMToken::Label("WHILE_0_END".to_string()),
                // return sum;
                VMToken::Push(VMSegment::Local, 2),
                VMToken::Return,
            ],
        )
    }
}
