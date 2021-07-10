extern crate pest;
#[macro_use]
extern crate pest_derive;
use anyhow::Result;

mod ast;
mod compiler;
mod parser;

use ast::{
    AssignmentStatement, Block, ClassDecl, Expression, Module, Node, Parameter, Scope, Term,
};
use compiler::Compiler;
use hackvm::VMToken;
use parser::{FUNParser, Rule};
use pest::iterators::Pair;
use pest::Parser;

use crate::ast::{FieldDecl, LetStatement, MethodDecl, Op, Statement, WhileStatement};

pub fn compile(input: &str) -> Result<Vec<VMToken>> {
    let module = parse_module(input)?;
    Compiler::new().compile_module(module)
}

pub fn parse_module(input: &str) -> Result<Module> {
    let mut classes = vec![];
    let pairs = FUNParser::parse(Rule::file, input)?;
    for pair in pairs {
        match pair.as_rule() {
            Rule::file => {
                for pair in pair.into_inner() {
                    match pair.as_rule() {
                        Rule::class_decl => classes.push(Node::from_pair(pair)?),
                        Rule::EOI => {
                            break;
                        }
                        _ => panic!("Not sure what to do with {:?}", pair),
                    }
                }
            }
            _ => panic!("Not sure what to do with {:?}", pair),
        }
    }
    return Ok(Module::new(classes));
}

impl<'a> Node<'a, ClassDecl<'a>> {
    pub fn name(&self) -> &str {
        self.data().name()
    }

    pub fn fields(&self) -> &Vec<Node<FieldDecl>> {
        self.data().fields()
    }

    pub fn methods(&self) -> &Vec<MethodDecl> {
        self.data().methods()
    }

    fn from_pair(pair: Pair<'a, Rule>) -> Result<Node<'a, ClassDecl>> {
        let span = pair.as_span();
        let mut pairs = pair.into_inner();
        let name = pairs
            .next()
            .expect("No class name found")
            .as_str()
            .to_string();
        let mut fields = vec![];
        let mut methods: Vec<MethodDecl> = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::static_method => {
                    methods.push(parse_method_decl(
                        pair.into_inner().next().unwrap(),
                        Scope::Static,
                    )?);
                }
                Rule::class_field => {
                    fields.push(parse_field_decl(pair)?);
                }
                Rule::class_method => {
                    methods.push(parse_method_decl(pair, Scope::Instance)?);
                }
                _ => panic!("Not sure what to do with {:?}", pair),
            }
        }
        Ok(Node::new(span, ClassDecl::new(name, fields, methods)))
    }
}

fn parse_field_decl<'a>(pair: Pair<'a, Rule>) -> Result<Node<'a, FieldDecl>> {
    let span = pair.as_span();
    let typed_identifier = pair.into_inner().next().expect("no typed identifier...");
    let (name, type_name) = parse_typed_identifier(typed_identifier)?;
    Ok(Node::new(span, FieldDecl::new(name, type_name)))
}

fn parse_typed_identifier(pair: Pair<Rule>) -> Result<(String, String)> {
    let mut pairs = pair.into_inner();
    Ok((
        pairs
            .next()
            .expect("no identifier found")
            .as_str()
            .to_string(),
        pairs
            .next()
            .expect("no type identifier found")
            .as_str()
            .to_string(),
    ))
}

fn parse_method_decl(pair: Pair<Rule>, scope: Scope) -> Result<MethodDecl> {
    let mut pairs = pair.into_inner();
    let name = pairs
        .next()
        .expect("no identifier found")
        .as_str()
        .to_string();
    let parameters = parse_parameter_decl(pairs.next().expect("no parameter declaration found"))?;
    let type_name = pairs
        .next()
        .expect("no identifier found")
        .as_str()
        .to_string();
    Ok(MethodDecl::new(
        scope,
        name,
        parameters,
        type_name,
        parse_block(pairs.next().expect("no block found"))?,
    ))
}

fn parse_parameter_decl(pair: Pair<Rule>) -> Result<Vec<Parameter>> {
    let mut params: Vec<Parameter> = vec![];
    for pair in pair.into_inner() {
        let (name, type_name) = parse_typed_identifier(pair)?;
        params.push(Parameter::new(name, type_name));
    }
    Ok(params)
}

fn parse_block(pair: Pair<Rule>) -> Result<Block> {
    let mut statements: Vec<Statement> = vec![];
    for pair in pair.into_inner() {
        for pair in pair.into_inner() {
            let statement: Statement = match pair.as_rule() {
                Rule::let_statement => Statement::Let(parse_let_statement(pair)?),
                Rule::while_statement => Statement::While(parse_while_statement(pair)?),
                Rule::return_statement => {
                    let expr = if let Some(pair) = pair.into_inner().next() {
                        parse_expr(pair)?
                    } else {
                        Expression::new(Term::Number(0))
                    };
                    Statement::Return(expr)
                }
                Rule::assignment_statement => {
                    Statement::Assignment(parse_assignment_statement(pair)?)
                }
                Rule::expr_statement => Statement::Expr,
                _ => panic!("Not sure what to do with {}", pair),
            };
            statements.push(statement);
        }
    }
    Ok(Block::new(statements))
}

fn parse_let_statement(pair: Pair<Rule>) -> Result<LetStatement> {
    let mut pairs = pair.into_inner();
    let (name, type_name) =
        parse_typed_identifier(pairs.next().expect("no typed identifier found"))?;
    let value_expr = parse_expr(pairs.next().expect("no expression found"))?;
    Ok(LetStatement::new(name, type_name, value_expr))
}

fn parse_assignment_statement(pair: Pair<Rule>) -> Result<AssignmentStatement> {
    let mut pairs = pair.into_inner();
    let left = pairs.next().expect("no expression to assign to");
    let right = pairs.next().expect("no value to assign to expression");

    let dest_expr = parse_expr(left)?;
    let value_expr = parse_expr(right)?;
    Ok(AssignmentStatement::new(dest_expr, value_expr))
}

fn parse_while_statement(pair: Pair<Rule>) -> Result<WhileStatement> {
    let mut pairs = pair.into_inner();
    let condition_expr = parse_expr(pairs.next().expect("no condition expression found"))?;
    let block = parse_block(pairs.next().expect("no block found in while statement"))?;
    Ok(WhileStatement::new(condition_expr, block))
}

fn parse_expr(pair: Pair<Rule>) -> Result<Expression> {
    let mut pairs = pair.into_inner();
    let mut term = parse_term(pairs.next().expect("Expression unexpectedly has no terms"))?;
    while let Some(pair) = pairs.next() {
        match pair.as_rule() {
            Rule::binary_operator => {
                let op = match pair.as_str() {
                    "+" => Op::Plus,
                    "-" => Op::Sub,
                    "<" => Op::Lt,
                    ">" => Op::Gt,
                    "." => Op::Dot,
                    other => panic!("Unrecognized operator {}", other),
                };
                let term_pair = pairs.next().expect("Operator without second term");
                let other_term = parse_term(term_pair)?;
                term = Term::BinaryOp(op, Box::from(term), Box::from(other_term));
            }
            _ => unreachable!(),
        }
    }
    Ok(Expression::new(term))
}

fn parse_term(pair: Pair<Rule>) -> Result<Term> {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::number => {
                let num = pair.as_str().parse::<u64>().unwrap();
                return Ok(Term::Number(num));
            }
            Rule::bool => {
                let value = match pair.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => panic!("Unexpected boolean {}", pair),
                };
                return Ok(Term::Bool(value));
            }
            Rule::string => {
                let s = pair.as_str();
                return Ok(Term::String(s[1..s.len() - 1].to_string()));
            }
            Rule::array => {
                let mut expressions = vec![];
                for pair in pair.into_inner() {
                    expressions.push(parse_expr(pair)?);
                }
                return Ok(Term::Array(expressions));
            }
            Rule::identifier => return Ok(Term::Identifier(pair.as_str().to_string())),
            _ => panic!("Not sure what to do with {:?}", pair),
        }
    }
    panic!("No term found?");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_empty_module() {
        let module = parse_module("").expect("Empty module did not parse");
        assert_eq!(module.classes().len(), 0, "Empty modules have 0 classes");
    }

    #[test]
    fn test_empty_classes() {
        let module = parse_module(
            "
            class Foo{}
            class Bar{}
        ",
        )
        .unwrap();
        assert_eq!(module.classes().len(), 2);
        assert_eq!(module.classes()[0].data().name(), "Foo");
        assert_eq!(module.classes()[1].data().name(), "Bar");
    }

    #[test]
    fn test_class_fields() {
        let module = parse_module(
            "
            class Vector {
                x: number;
                y: number;
            }
        ",
        )
        .unwrap();
        let fields = module.classes()[0].fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].data().name(), "x");
        assert_eq!(fields[0].data().type_name(), "number");
    }

    #[test]
    fn test_static_methods() {
        let module = parse_module(
            "
            class Main {
                static main(): number {
                    return 1;
                }
            }
        ",
        )
        .unwrap();
        let methods = module.classes()[0].methods();
        assert_eq!(methods[0].scope(), &Scope::Static);
    }

    #[test]
    fn test_class_methods() {
        let module = parse_module(
            "
            class Vector {
                magnitude():number {}
                plus2(other1: Vector, other2: Vector): Vector {}
            }
        ",
        )
        .unwrap();
        let methods = module.classes()[0].methods();
        assert_eq!(methods.len(), 2);
        let magnitude = &methods[0];
        assert_eq!(magnitude.scope(), &Scope::Instance);
        assert_eq!(magnitude.name(), "magnitude");
        assert_eq!(magnitude.type_name(), "number");

        let plus = &methods[1];
        assert_eq!(plus.name(), "plus2");
        let params = plus.parameters();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name(), "other1");
        assert_eq!(params[0].type_name(), "Vector");
    }

    #[test]
    fn test_block() {
        let pair = FUNParser::parse(
            Rule::block,
            "{
                let i: number = 0;
                i = i + 1;
                let sum: number = 0;
                while (i < 10) {
                    sum = sum + i;
                    i = i + 1;
                }
                Logger.log(sum);
                return sum;
            }",
        )
        .expect("failed to parse")
        .next()
        .unwrap();
        let block = parse_block(pair).unwrap();
        assert_eq!(block.statements().len(), 6);
    }

    #[test]
    fn test_let_statement() {
        let pair = FUNParser::parse(Rule::let_statement, "let foo: number = 0;")
            .expect("failed to parse")
            .next()
            .unwrap();
        let let_statement = parse_let_statement(pair).unwrap();
        assert_eq!(let_statement.name(), "foo");
        assert_eq!(let_statement.type_name(), "number");
        let term = let_statement.value_expr().term();
        assert_eq!(term.as_number(), Some(0));
    }

    #[test]
    fn test_assignment_statement() {
        let pair = FUNParser::parse(Rule::assignment_statement, "foo = 0;")
            .expect("failed to parse")
            .next()
            .unwrap();
        let assignment = parse_assignment_statement(pair).unwrap();
        let dest = assignment
            .dest_expr()
            .term()
            .as_identifer()
            .expect("destination expression wasn't an identifier");
        assert_eq!(dest, "foo");
        let term = assignment.value_expr().term();
        assert_eq!(term.as_number(), Some(0));
    }

    mod expr {
        use super::*;
        fn parse_expr_from_str(s: &str) -> Expression {
            let pair = FUNParser::parse(Rule::expr, s).unwrap().next().unwrap();
            parse_expr(pair).unwrap()
        }

        #[test]
        fn test_const_expr() {
            assert_eq!(parse_expr_from_str("0").term().as_number(), Some(0));
            assert_eq!(parse_expr_from_str("true").term().as_bool(), Some(true));
            assert_eq!(
                parse_expr_from_str("\"foo\"").term().as_string(),
                Some("foo")
            );

            let expr = parse_expr_from_str("[1,2,3]");
            let array_term = expr.term().as_array().unwrap();
            assert_eq!(array_term[0].term().as_number(), Some(1));
            assert_eq!(array_term[1].term().as_number(), Some(2));
            assert_eq!(array_term[2].term().as_number(), Some(3));
        }

        #[test]
        fn test_multipart_expr() {
            let expr = parse_expr_from_str("3+4");
            let (op, a, b) = expr.term().as_binary_op().unwrap();
            assert_eq!(op, &Op::Plus);
            assert_eq!(a.as_number(), Some(3));
            assert_eq!(b.as_number(), Some(4));
        }

        #[test]
        fn test_nested_expr() {
            // this should become (3 + 4) + 5
            let expr = parse_expr_from_str("3+4+5");
            let (op, a, b) = expr.term().as_binary_op().unwrap();
            assert_eq!(op, &Op::Plus);
            assert_eq!(b.as_number(), Some(5));
            let (op, a, b) = a.as_binary_op().unwrap();
            assert_eq!(op, &Op::Plus);
            assert_eq!(a.as_number(), Some(3));
            assert_eq!(b.as_number(), Some(4));
        }

        #[test]
        fn test_dot_operator() {
            let expr = parse_expr_from_str("foo.bar.baz");
            assert_eq!(
                expr.term(),
                &Term::binary_op(
                    Op::Dot,
                    Term::binary_op(Op::Dot, Term::identifier("foo"), Term::identifier("bar")),
                    Term::identifier("baz")
                )
            );
        }
    }
}
