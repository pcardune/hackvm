use pest::Parser;
use std::{borrow::Borrow, convert::TryFrom, str::FromStr};

use crate::{
    ast::{BinaryOp, LetStatement, Op, Term},
    lexer::{FUNLexer, Rule},
    parser::{parse_let_statement, parse_term},
};
use anyhow::Result;
use hackvm::{VMSegment, VMToken};
use pest::iterators::Pair;

impl FromStr for LetStatement {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pair = FUNLexer::parse(Rule::let_statement, s)?.next().unwrap();
        LetStatement::try_from(pair)
    }
}

impl FromStr for Term {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pair = FUNLexer::parse(Rule::term, s)?.next().unwrap();
        Term::try_from(pair)
    }
}

impl<'i> TryFrom<Pair<'i, Rule>> for LetStatement {
    type Error = anyhow::Error;

    fn try_from(value: Pair<Rule>) -> Result<Self, Self::Error> {
        parse_let_statement(value)
    }
}

impl<'i> TryFrom<Pair<'i, Rule>> for Term {
    type Error = anyhow::Error;

    fn try_from(value: Pair<Rule>) -> Result<Self, Self::Error> {
        parse_term(value)
    }
}

struct TermCompiler<'a> {
    term: &'a Term,
}
impl<'a> TermCompiler<'a> {
    fn vmcode(&self) -> Result<Vec<VMToken>> {
        match self.term {
            Term::Number(num) => Ok(vec![VMToken::Push(VMSegment::Constant, *num as u16)]),
            Term::Bool(_) => todo!(),
            Term::String(_) => todo!(),
            Term::Array(_) => todo!(),
            Term::New(_, _) => todo!(),
            Term::Call(_, _) => todo!(),
            Term::Indexing(_, _) => todo!(),
            Term::Identifier(_) => todo!(),
            Term::Expr(expr) => todo!(),
            Term::BinaryOp(_) => todo!(),
            Term::UnaryOp(_, _) => todo!(),
        }
    }
}
impl<'a> From<&'a Term> for TermCompiler<'a> {
    fn from(term: &'a Term) -> Self {
        TermCompiler { term }
    }
}

struct BinaryOpCompiler<'a> {
    binop: &'a BinaryOp,
}
impl<'a> BinaryOpCompiler<'a> {
    fn left(&self) -> TermCompiler {
        TermCompiler::from(self.binop.left().borrow())
    }
    fn right(&self) -> TermCompiler {
        TermCompiler::from(self.binop.right().borrow())
    }
    fn op(&self) -> VMToken {
        match self.binop.op() {
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
            _ => todo!("Don't know how to handle op {:?}", self.binop.op()),
        }
    }
    fn vmcode(&self) -> Result<Vec<VMToken>> {
        let op_token = self.op();
        let left = self.left().vmcode()?;
        let right = self.right().vmcode()?;
        let mut tokens: Vec<VMToken> = Vec::new();
        tokens.extend(left);
        tokens.extend(right);
        tokens.push(op_token);
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{LetStatement, Term},
        parser::parse_module,
    };

    use super::*;

    #[test]
    fn test_let_statement() {
        let statement = "let a:number = 1+1;".parse::<LetStatement>().unwrap();
        let term = "1+1".parse::<Term>().unwrap();
        let compiler = TermCompiler::from(&term);
    }
}
