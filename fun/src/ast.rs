use getset::Getters;

pub struct Module {
    pub classes: Vec<ClassDecl>,
}

#[derive(Getters)]
pub struct ClassDecl {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    fields: Vec<FieldDecl>,

    #[getset(get = "pub")]
    methods: Vec<MethodDecl>,
}

impl ClassDecl {
    pub fn new(name: String, fields: Vec<FieldDecl>, methods: Vec<MethodDecl>) -> ClassDecl {
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
    name: String,

    #[getset(get = "pub")]
    type_name: String,
}

impl FieldDecl {
    pub fn new(name: String, type_name: String) -> FieldDecl {
        FieldDecl { name, type_name }
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

#[derive(Getters)]
pub struct Block {
    #[getset(get = "pub")]
    statements: Vec<Statement>,
}
impl Block {
    pub fn new(statements: Vec<Statement>) -> Block {
        Block { statements }
    }
}

pub enum Statement {
    Let(LetStatement),
    While,
    Return,
    Assignment,
    Expr,
}

#[derive(Getters)]
pub struct LetStatement {
    #[getset(get = "pub")]
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
}

#[derive(Debug)]
pub enum Term {
    Number(i64),
    Bool(bool),
    String(String),
    Array(Vec<Expression>),
    BinaryOp(Box<Term>, Box<Term>),
}

impl Term {
    pub fn as_number(&self) -> Option<i64> {
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
    // pub fn as_array() -> Vec<Expression>
}

#[derive(Getters, Debug)]
pub struct Expression {
    #[getset(get = "pub")]
    term: Term,
}

impl Expression {
    pub fn new(term: Term) -> Expression {
        Expression { term }
    }
}
