use anyhow::{anyhow, Context, Result};
use hackvm::{VMSegment, VMToken};

use crate::{ast::*, compiler::Type};

use super::{
    namespace::{MemRef, Namespace},
    MethodDeclCompiler,
};

pub struct BlockCompiler<'block> {
    method_compiler: &'block MethodDeclCompiler<'block>,
    parent_block: Option<&'block BlockCompiler<'block>>,
    block: &'block Block,
    while_count: usize,
    if_count: usize,
    block_count: usize,
    local_names: Namespace,
    label_prefix: String,
    num_locals: usize,
}

impl<'block> BlockCompiler<'block> {
    pub fn new<'c>(
        method_compiler: &'c MethodDeclCompiler<'c>,
        block: &'c Block,
    ) -> BlockCompiler<'c> {
        BlockCompiler {
            method_compiler,
            parent_block: None,
            block,
            while_count: 0,
            if_count: 0,
            block_count: 0,
            local_names: Namespace::default(),
            label_prefix: String::new(),
            num_locals: 0,
        }
    }

    fn compile_block(&'block self, block: &'block Block) -> Result<(usize, Vec<VMToken>)> {
        let mut subcompile = BlockCompiler::new(self.method_compiler, block);
        subcompile.label_prefix = self.get_label(&format!("BLOCK_{}", self.block_count));
        subcompile
            .local_names
            .set_offset(&VMSegment::Local, &self.local_names);
        subcompile.parent_block = Some(self);
        subcompile.compile()
    }

    fn get_local_name(&self, name: &str) -> Option<MemRef> {
        let value = self.local_names.get(name);
        if value.is_some() {
            value
        } else {
            if let Some(parent) = self.parent_block {
                parent.get_local_name(name)
            } else {
                self.method_compiler.local_names.get(name)
            }
        }
    }

    fn get_label(&self, label: &str) -> String {
        if self.label_prefix.len() > 0 {
            format!("{}_{}", self.label_prefix, label)
        } else {
            label.to_string()
        }
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
                        match self
                            .method_compiler
                            .class_compiler
                            .get_instance_field(instance_field_name)
                        {
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
                    match self.get_local_name(left_identifier) {
                        Some(left_mem_ref) => {
                            // we're doing an instance field lookup on a local/argument variable
                            // that must be a pointer, so update the That segment to point to it.
                            let mut tokens = vec![
                                left_mem_ref.as_push_token(),
                                VMToken::Pop(VMSegment::Pointer, 1),
                            ];
                            // now we need to resolve the field based on the type that it is.
                            let left_obj_type = self
                                .method_compiler
                                .module_compiler()
                                .get_object_types()
                                .get_by_id(left_mem_ref.type_id)
                                .expect("wasn't able to get ObjectType from MemRef")
                                .clone(); // TODO: see about removing this clone?
                            let left_obj_type = match left_obj_type {
                                Type::Object(t) => t,
                                _ => todo!(),
                            };
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
                                        .method_compiler
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

    fn compile_unary_op(&mut self, op: &UnaryOp, operand: &Term) -> Result<Vec<VMToken>> {
        let mut tokens = self.compile_term(operand)?;
        tokens.push(match op {
            UnaryOp::BitNot | UnaryOp::Not => VMToken::Not,
            UnaryOp::Neg => VMToken::Neg,
        });
        Ok(tokens)
    }

    fn compile_reference(&mut self, reference: &str) -> Result<Vec<VMToken>> {
        if let Some(mem_ref) = self.get_local_name(reference) {
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
    fn compile_expression(&mut self, expression: &Expression) -> Result<Vec<VMToken>> {
        self.compile_term(expression.term())
    }

    fn compile_let_statement(&mut self, let_statement: &LetStatement) -> Result<Vec<VMToken>> {
        let name = let_statement.name();
        let index = self.local_names.register(
            name,
            &VMSegment::Local,
            self.method_compiler
                .module_compiler()
                .resolve_type(let_statement.type_name())
                .with_context(|| {
                    format!(
                        "Could not resolve type name {} for let statement {}",
                        let_statement.type_name(),
                        let_statement.name(),
                    )
                })?,
        );
        self.num_locals += 1;
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
                            if let Some(mem_ref) = self
                                .method_compiler
                                .class_compiler
                                .get_instance_field(field_name)
                            {
                                vec![mem_ref.as_pop_token()]
                            } else {
                                return Err(anyhow!(
                                    "instance field \"{}\" is not declared",
                                    field_name
                                ));
                            }
                        } else if let Some(mem_ref) = self
                            .method_compiler
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
                if let Some(mem_ref) = self.get_local_name(name) {
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
        let end_label = self.get_label(&format!("IF_{}_END", self.if_count));
        let else_end_label = self.get_label(&format!("IF_{}_ELSE_END", self.if_count));
        self.if_count += 1;

        // condition check
        let mut tokens = self.compile_expression(if_statement.condition_expr())?;
        tokens.push(VMToken::Not);
        tokens.push(VMToken::If(end_label.clone()));
        // if block
        let (num_locals, mut block_tokens) = self.compile_block(if_statement.block())?;
        self.num_locals += num_locals;

        tokens.append(&mut block_tokens);
        if if_statement.else_block().is_some() {
            tokens.push(VMToken::Goto(else_end_label.clone()))
        }
        // else block
        tokens.push(VMToken::Label(end_label));
        if let Some(else_block) = if_statement.else_block() {
            let (num_locals, mut block_tokens) = self.compile_block(else_block)?;
            self.num_locals += num_locals;
            tokens.append(&mut block_tokens);
            tokens.push(VMToken::Label(else_end_label));
        }
        Ok(tokens)
    }

    fn compile_while_statement(
        &mut self,
        while_statement: &WhileStatement,
    ) -> Result<Vec<VMToken>> {
        let start_label = self.get_label(&format!("WHILE_{}", self.while_count));
        self.while_count += 1;
        let end_label = self.get_label(&format!("{}_END", start_label));
        let mut tokens = vec![VMToken::Label(start_label.clone())];
        tokens.append(&mut self.compile_expression(while_statement.condition_expr())?);
        tokens.push(VMToken::Not);
        tokens.push(VMToken::If(end_label.clone()));
        let (num_locals, mut block_tokens) = self.compile_block(while_statement.block())?;
        self.num_locals += num_locals;
        tokens.append(&mut block_tokens);
        tokens.push(VMToken::Goto(start_label.clone()));
        tokens.push(VMToken::Label(end_label));
        return Ok(tokens);
    }

    pub fn compile(&'block mut self) -> Result<(usize, Vec<VMToken>)> {
        let mut commands = Vec::new();
        for statement in self.block.statements() {
            match statement {
                Statement::Return(expression) => {
                    for command in self.compile_expression(expression)? {
                        commands.push(command);
                    }
                    commands.push(VMToken::Return);
                }
                Statement::Let(let_statement) => {
                    let mut tokens = self.compile_let_statement(let_statement)?;
                    commands.append(&mut tokens);
                }
                Statement::While(while_statement) => {
                    let mut tokens = self.compile_while_statement(while_statement)?;
                    commands.append(&mut tokens);
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
        Ok((self.num_locals, commands))
    }
}
