extern crate pest;
#[macro_use]
extern crate pest_derive;
use anyhow::{anyhow, Context, Result};

pub mod ast;
mod compiler;
mod lexer;
mod parser;

use ast::Module;
use compiler::ModuleCompiler;
use hackvm::VMToken;
use parser::parse_module;

pub struct CompilerOutput {
    pub vmtokens: Vec<VMToken>,
    pub ast: Module,
}

pub fn compile(input: &str) -> Result<CompilerOutput> {
    let module =
        parse_module(input).with_context(|| anyhow!("fun::compile: parse_module failed"))?;
    let vmtokens = ModuleCompiler::new(&module).compile()?;
    Ok(CompilerOutput {
        vmtokens,
        ast: module,
    })
}
