use crate::context::Context;
use crate::state::Execution;
use crate::value::Val;
use dyn_clone::DynClone;
use std::collections::HashMap;
use walrus::ir;

mod div;
mod memory;

pub use div::*;
pub use memory::*;

pub enum CheckResult {
    Ok,
    PossibleFail(String),
    Fail(String),
}

pub trait Check<'ctx>: DynClone + std::fmt::Debug {
    fn name(&self) -> &'static str;

    fn check(
        &mut self,
        context: &'ctx Context,
        execution: &Execution<'ctx>,
        instr: &ir::Instr,
        loc: &ir::InstrLocId,
    );

    fn run(
        &mut self,
        context: &'ctx Context,
        execution: &Execution<'ctx>,
        inputs: &HashMap<ir::LocalId, Val<'ctx>>,
    ) -> CheckResult;
}

impl<'ctx> Clone for Box<dyn Check<'ctx> + 'ctx> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}
