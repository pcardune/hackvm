use std::collections::HashMap;

use crate::ast::{AssignmentStatement, Block, WhileStatement};
use crate::ast::{Expression, LetStatement, Module, Op, Statement, Term};
use anyhow::anyhow;
use anyhow::Result;
use hackvm::{VMSegment, VMToken};

pub struct Compiler {
    local_names: HashMap<String, usize>,
}

impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            local_names: HashMap::new(),
        }
    }

    pub fn compile_module(&mut self, module: Module) -> Result<Vec<VMToken>> {
        let mut commands: Vec<VMToken> = Vec::new();
        for class_decl in module.classes() {
            for method in class_decl.methods() {
                self.local_names.clear();

                let block_tokens = self.compile_block(method.block())?;

                let num_locals = self.local_names.len();
                let token = VMToken::Function(
                    format!("{}.{}", class_decl.name(), method.name()),
                    num_locals as u16,
                );
                commands.push(token);
                commands.append(&mut block_tokens.into());
            }
        }
        return Ok(commands);
    }

    fn compile_let_statement(&mut self, let_statement: &LetStatement) -> Result<Vec<VMToken>> {
        let name = let_statement.name();
        if self.local_names.contains_key(name) {
            return Err(anyhow!(
                "a variable with the name \"{}\" has already been declared",
                name
            ));
        }
        let index = self.local_names.len();
        self.local_names.insert(name.to_string(), index);
        let mut tokens = self.compile_expression(let_statement.value_expr())?;
        tokens.push(VMToken::Pop(VMSegment::Local, index as u16));
        return Ok(tokens);
    }

    fn compile_assignment_statement(
        &mut self,
        assignment_statement: &AssignmentStatement,
    ) -> Result<Vec<VMToken>> {
        let name = &assignment_statement.name()[..];
        if let Some(&index) = self.local_names.get(name) {
            let mut tokens = self.compile_expression(assignment_statement.value_expr())?;
            tokens.push(VMToken::Pop(VMSegment::Local, index as u16));
            Ok(tokens)
        } else {
            Err(anyhow!("variable \"{}\" has never been declared", name))
        }
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

    fn compile_reference(&mut self, reference: &String) -> Result<Vec<VMToken>> {
        if let Some(index) = self.local_names.get(reference) {
            return Ok(vec![VMToken::Push(VMSegment::Local, *index as u16)]);
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
            Term::Reference(name) => self.compile_reference(name),
            _ => panic!("Don't know how to compile {:?}", term),
        }
    }

    fn compile_binary_op(&mut self, op: &Op, left: &Term, right: &Term) -> Result<Vec<VMToken>> {
        let mut tokens = self.compile_term(left)?;
        tokens.append(&mut self.compile_term(right)?);
        let op_token = match op {
            Op::Plus => VMToken::Add,
            Op::Sub => VMToken::Sub,
            Op::Lt => VMToken::Lt,
            Op::Gt => VMToken::Gt,
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

        let vmcode = Compiler::new().compile_module(module).unwrap();
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

        let vmcode = Compiler::new().compile_module(module).unwrap();
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
