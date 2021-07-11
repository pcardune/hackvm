extern crate pest;
#[macro_use]
extern crate pest_derive;
use anyhow::{anyhow, Result};

mod ast;
mod compiler;
mod parser;

use ast::{
    AssignmentStatement, Block, ClassDecl, Expression, Module, Node, Parameter, Scope, Term,
};
use compiler::ModuleCompiler;
use hackvm::VMToken;
use parser::{FUNParser, Rule};
use pest::Parser;
use pest::{
    iterators::Pair,
    prec_climber::{Assoc, Operator, PrecClimber},
};

use crate::ast::{FieldDecl, LetStatement, MethodDecl, Op, Statement, WhileStatement};

pub fn compile(input: &str) -> Result<Vec<VMToken>> {
    let module = parse_module(input)?;
    ModuleCompiler::new(&module).compile()
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

impl Node<ClassDecl> {
    pub fn name(&self) -> &str {
        self.data().name()
    }

    pub fn fields(&self) -> &Vec<Node<FieldDecl>> {
        self.data().fields()
    }

    pub fn methods(&self) -> &Vec<MethodDecl> {
        self.data().methods()
    }

    fn from_pair(pair: Pair<Rule>) -> Result<Node<ClassDecl>> {
        let span = pair.as_span();
        let mut pairs = pair.into_inner();
        let name = pairs
            .next()
            .expect("No class name found")
            .as_str()
            .to_string();
        let mut fields = vec![];
        let mut methods: Vec<MethodDecl> = vec![];
        let mut constructor: Option<MethodDecl> = None;
        for pair in pairs {
            match pair.as_rule() {
                Rule::static_field => {
                    fields.push(parse_field_decl(
                        pair.into_inner().next().unwrap(),
                        Scope::Static,
                    )?);
                }
                Rule::static_method => {
                    methods.push(parse_method_decl(
                        pair.into_inner().next().unwrap(),
                        Scope::Static,
                    )?);
                }
                Rule::class_field => {
                    fields.push(parse_field_decl(pair, Scope::Instance)?);
                }
                Rule::class_method => {
                    methods.push(parse_method_decl(pair, Scope::Instance)?);
                }
                Rule::constructor_decl => {
                    if constructor.is_some() {
                        return Err(anyhow!("constructor declared more than once"));
                    }
                    constructor = Some(parse_constructor_decl(pair)?);
                }
                _ => panic!("Not sure what to do with {:?}", pair),
            }
        }
        Ok(Node::new(ClassDecl::new(
            name,
            fields,
            methods,
            constructor,
        )))
    }
}

fn parse_field_decl<'a>(pair: Pair<'a, Rule>, scope: Scope) -> Result<Node<FieldDecl>> {
    let span = pair.as_span();
    let typed_identifier = pair.into_inner().next().expect("no typed identifier...");
    let (name, type_name) = parse_typed_identifier(typed_identifier)?;
    Ok(Node::new(FieldDecl::new(scope, name, type_name)))
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

fn parse_constructor_decl(pair: Pair<Rule>) -> Result<MethodDecl> {
    let mut pairs = pair.into_inner();
    let parameters = parse_parameter_decl(pairs.next().expect("no parameter declaration found"))?;
    let block = parse_block(pairs.next().expect("no block found"))?;
    Ok(MethodDecl::new(Scope::Static, "new", parameters, "", block))
}

fn parse_method_decl(pair: Pair<Rule>, scope: Scope) -> Result<MethodDecl> {
    let mut pairs = pair.into_inner();
    let name = pairs.next().expect("no identifier found").as_str();
    let parameters = parse_parameter_decl(pairs.next().expect("no parameter declaration found"))?;
    let type_name = pairs.next().expect("no identifier found").as_str();
    let block = parse_block(pairs.next().expect("no block found"))?;
    Ok(MethodDecl::new(scope, name, parameters, type_name, block))
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
    let climber: PrecClimber<Rule> = PrecClimber::new(vec![
        Operator::new(Rule::cmp_lt, Assoc::Left) | Operator::new(Rule::cmp_gt, Assoc::Left),
        Operator::new(Rule::plus, Assoc::Left) | Operator::new(Rule::sub, Assoc::Left),
        Operator::new(Rule::dot, Assoc::Left),
    ]);

    let primary = |pair: Pair<Rule>| parse_term(pair).unwrap();
    let infix = |left: Term, op: Pair<Rule>, right: Term| {
        let op = match op.as_rule() {
            Rule::plus => Op::Plus,
            Rule::sub => Op::Sub,
            Rule::cmp_lt => Op::Lt,
            Rule::cmp_gt => Op::Gt,
            Rule::dot => Op::Dot,
            other => panic!("Unrecognized operator {:?}", other),
        };
        Term::binary_op(op, left, right)
    };
    let pairs = pair.into_inner();
    let result = climber.climb(pairs, primary, infix);
    Ok(Expression::new(result))
}

fn parse_new_expr(pair: Pair<Rule>) -> Result<Term> {
    assert!(pair.as_rule() == Rule::new_expr);
    let mut pairs = pair.into_inner();
    let call_term = parse_call_expr(
        pairs
            .next()
            .expect("new expression didn't start with call_expr"),
    )?;
    if let Term::Call(func_name, arguments) = call_term {
        Ok(Term::New(func_name, arguments))
    } else {
        panic!("Didn't get a call term back from parse_call_expr");
    }
}

fn parse_call_expr(pair: Pair<Rule>) -> Result<Term> {
    assert!(pair.as_rule() == Rule::call_expr);
    let mut pairs = pair.into_inner();
    let func_name = pairs
        .next()
        .expect("call expression didn't start with identifier")
        .as_str();
    let mut arguments: Vec<Expression> = Vec::new();
    for pair in pairs {
        arguments.push(parse_expr(pair)?);
    }
    Ok(Term::Call(func_name.to_owned(), arguments))
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
            Rule::call_expr => {
                return parse_call_expr(pair);
            }
            Rule::new_expr => {
                return parse_new_expr(pair);
            }
            Rule::identifier => return Ok(Term::Identifier(pair.as_str().to_string())),
            _ => panic!(
                "Not sure what to do with rule {:?}: {}",
                pair.as_rule(),
                pair.as_str()
            ),
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
    fn test_class_static_fields() {
        let module = parse_module(
            "
            class Main {
                static counter: number;
                step: number;
            }
        ",
        )
        .unwrap();
        let fields = module.classes()[0].fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].data().scope(), &Scope::Static);
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
    fn test_class_constructor() {
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
        let constructor = module.classes()[0].data().constructor().as_ref();
        assert!(constructor.is_some());
        assert_eq!(constructor.unwrap().parameters().len(), 2);
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
        fn test_call_expr() {
            let expr = parse_expr_from_str("someFunc(a, b)");
            assert_eq!(
                expr.term(),
                &Term::Call(
                    "someFunc".to_string(),
                    vec![
                        Expression::new(Term::identifier("a")),
                        Expression::new(Term::identifier("b"))
                    ]
                )
            )
        }

        #[test]
        fn test_new_expr() {
            let expr = parse_expr_from_str("new Vector(x, y)");
            assert_eq!(
                expr.term(),
                &Term::New(
                    "Vector".to_string(),
                    vec![
                        Expression::new(Term::identifier("x")),
                        Expression::new(Term::identifier("y"))
                    ]
                )
            )
        }

        #[test]
        fn test_multipart_expr() {
            let expr = parse_expr_from_str("3+4");
            assert_eq!(
                expr.term(),
                &Term::binary_op(Op::Plus, Term::Number(3), Term::Number(4))
            );
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

        #[test]
        fn test_operator_precendence() {
            let expr = parse_expr_from_str("foo.bar + other.baz.zap");
            assert_eq!(
                expr.term(),
                &Term::binary_op(
                    Op::Plus,
                    Term::binary_op(Op::Dot, Term::identifier("foo"), Term::identifier("bar")),
                    Term::binary_op(
                        Op::Dot,
                        Term::binary_op(
                            Op::Dot,
                            Term::identifier("other"),
                            Term::identifier("baz")
                        ),
                        Term::identifier("zap")
                    )
                )
            )
        }
    }
}
