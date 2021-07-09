use crate::ast::{Expression, Module, Statement, Term};
use anyhow::Result;
use hackvm::{VMSegment, VMToken};

pub fn compile_module(module: Module) -> Result<Vec<VMToken>> {
    let mut commands: Vec<VMToken> = Vec::new();
    for class_decl in module.classes() {
        for method in class_decl.methods() {
            let num_locals = 0;
            let token = VMToken::Function(
                format!("{}.{}", class_decl.name(), method.name()),
                num_locals,
            );
            commands.push(token);

            for statement in method.block().statements() {
                match statement {
                    Statement::Return(expression) => {
                        for command in compile_expression(expression)? {
                            commands.push(command);
                        }
                        commands.push(VMToken::Return);
                    }
                    _ => panic!("Can't handle {:?}", statement),
                }
            }
        }
    }
    return Ok(commands);
}

pub fn compile_expression(expression: &Expression) -> Result<Vec<VMToken>> {
    match expression.term() {
        Term::Number(num) => return Ok(vec![VMToken::Push(VMSegment::Constant, *num as u16)]),
        _ => panic!("Don't know how to compile {:?}", expression.term()),
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
                    return 3+4;
                }
            }
        ",
        )
        .unwrap();

        let vmcode = compile_module(module).unwrap();
        assert_eq!(
            &vmcode,
            &[
                VMToken::Function("Main.main".to_string(), 0),
                VMToken::Push(VMSegment::Constant, 3),
                VMToken::Return
            ]
        )
    }
}
