extern crate pest;
#[macro_use]
extern crate pest_derive;
use anyhow::{anyhow, Context, Result};

mod ast;
mod compiler;
mod lexer;
mod parser;

use compiler::ModuleCompiler;
use hackvm::VMToken;
use parser::parse_module;

pub fn compile(input: &str) -> Result<Vec<VMToken>> {
    let module =
        parse_module(input).with_context(|| anyhow!("fun::compile: parse_module failed"))?;
    ModuleCompiler::new(&module).compile()
}
