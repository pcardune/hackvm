use getset::Getters;
use pest::Span;

#[derive(Getters)]
pub struct Module<'a> {
    #[getset(get = "pub")]
    classes: Vec<Node<'a, ClassDecl<'a>>>,
}

impl<'a> Module<'a> {
    pub fn new(classes: Vec<Node<'a, ClassDecl<'a>>>) -> Module<'a> {
        Module { classes }
    }
}

#[derive(Getters)]
pub struct Node<'a, T: ?Sized> {
    #[getset(get = "pub")]
    span: Span<'a>,
    #[getset(get = "pub")]
    data: Box<T>,
}

impl<'a, T> Node<'a, T> {
    pub fn new(span: Span<'a>, data: T) -> Node<'a, T> {
        Node {
            span,
            data: Box::from(data),
        }
    }
}

#[derive(Getters)]
pub struct ClassDecl<'a> {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    fields: Vec<Node<'a, FieldDecl>>,

    #[getset(get = "pub")]
    methods: Vec<MethodDecl>,
}

impl<'a> ClassDecl<'a> {
    pub fn new(
        name: String,
        fields: Vec<Node<'a, FieldDecl>>,
        methods: Vec<MethodDecl>,
    ) -> ClassDecl<'a> {
        ClassDecl {
            name,
            fields,
            methods,
        }
    }
}

#[derive(Getters)]
pub struct FieldDecl {
    #[getset(get = "pub")]
    scope: Scope,

    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    type_name: String,
}

impl FieldDecl {
    pub fn new(scope: Scope, name: String, type_name: String) -> FieldDecl {
        FieldDecl {
            scope,
            name,
            type_name,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum Scope {
    Static,
    Instance,
}

#[derive(Getters)]
pub struct MethodDecl {
    #[getset(get = "pub")]
    scope: Scope,

    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    type_name: String,

    #[getset(get = "pub")]
    parameters: Vec<Parameter>,

    #[getset(get = "pub")]
    block: Block,
}

impl MethodDecl {
    pub fn new(
        scope: Scope,
        name: String,
        parameters: Vec<Parameter>,
        type_name: String,
        block: Block,
    ) -> MethodDecl {
        MethodDecl {
            scope,
            name,
            parameters,
            type_name,
            block,
        }
    }
}

#[derive(Getters)]
pub struct Parameter {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    type_name: String,
}
impl Parameter {
    pub fn new(name: String, type_name: String) -> Parameter {
        Parameter { name, type_name }
    }
}

#[derive(Getters, Debug)]
pub struct Block {
    #[getset(get = "pub")]
    statements: Vec<Statement>,
}
impl Block {
    pub fn new(statements: Vec<Statement>) -> Block {
        Block { statements }
    }
}

#[derive(Debug)]
pub enum Statement {
    Let(LetStatement),
    While(WhileStatement),
    Return(Expression),
    Assignment(AssignmentStatement),
    Expr,
}

#[derive(Getters, Debug)]
pub struct LetStatement {
    name: String,

    #[getset(get = "pub")]
    type_name: String,

    #[getset(get = "pub")]
    value_expr: Expression,
}

impl LetStatement {
    pub fn new(name: String, type_name: String, value_expr: Expression) -> LetStatement {
        LetStatement {
            name,
            type_name,
            value_expr,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Getters, Debug)]
pub struct AssignmentStatement {
    #[getset(get = "pub")]
    dest_expr: Expression,

    #[getset(get = "pub")]
    value_expr: Expression,
}

impl AssignmentStatement {
    pub fn new(dest_expr: Expression, value_expr: Expression) -> AssignmentStatement {
        AssignmentStatement {
            dest_expr,
            value_expr,
        }
    }
}

#[derive(Getters, Debug)]
pub struct WhileStatement {
    #[getset(get = "pub")]
    condition_expr: Expression,
    #[getset(get = "pub")]
    block: Block,
}

impl WhileStatement {
    pub fn new(condition_expr: Expression, block: Block) -> WhileStatement {
        WhileStatement {
            condition_expr,
            block,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Op {
    Plus,
    Sub,
    Lt,
    Gt,
    Dot,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Term {
    Number(u64),
    Bool(bool),
    String(String),
    Array(Vec<Expression>),
    Identifier(String),
    BinaryOp(Op, Box<Term>, Box<Term>),
    Call(String, Vec<Expression>),
}

impl Term {
    pub fn binary_op(op: Op, left: Term, right: Term) -> Term {
        Term::BinaryOp(op, Box::from(left), Box::from(right))
    }

    pub fn identifier(s: &str) -> Term {
        Term::Identifier(s.to_string())
    }

    pub fn as_identifer(&self) -> Option<&str> {
        if let Term::Identifier(val) = self {
            Some(val)
        } else {
            None
        }
    }
    pub fn as_number(&self) -> Option<u64> {
        if let Term::Number(val) = self {
            Some(*val)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Term::Bool(val) = self {
            Some(*val)
        } else {
            None
        }
    }
    pub fn as_string(&self) -> Option<&str> {
        if let Term::String(val) = self {
            Some(val)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&[Expression]> {
        if let Term::Array(val) = self {
            Some(&val[..])
        } else {
            None
        }
    }
    pub fn as_binary_op(&self) -> Option<(&Op, &Box<Term>, &Box<Term>)> {
        if let Term::BinaryOp(op, t1, t2) = self {
            Some((op, t1, t2))
        } else {
            None
        }
    }
}

#[derive(Getters, Debug, PartialEq, Clone)]
pub struct Expression {
    #[getset(get = "pub")]
    term: Term,
}

impl Expression {
    pub fn new(term: Term) -> Expression {
        Expression { term }
    }
}
