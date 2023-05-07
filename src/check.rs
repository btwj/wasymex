use crate::context::Context;
use crate::memory::PAGE_SIZE;
use crate::reporter::Reporter;
use crate::state::Execution;
use crate::value::{ConcVal, SymVal, Val};
use dyn_clone::DynClone;
use std::collections::HashMap;
use walrus::ir;
use z3::ast::Ast;

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

#[derive(Clone, Debug)]
pub struct DivisionByZeroCheck<'ctx> {
    constraints: HashMap<u32, z3::ast::Bool<'ctx>>,
}

impl<'ctx> DivisionByZeroCheck<'ctx> {
    pub fn new() -> Self {
        DivisionByZeroCheck {
            constraints: HashMap::new(),
        }
    }
}

impl<'ctx> Check<'ctx> for DivisionByZeroCheck<'ctx> {
    fn name(&self) -> &'static str {
        "DivisionByZero"
    }

    fn check(
        &mut self,
        context: &'ctx Context,
        execution: &Execution<'ctx>,
        instr: &ir::Instr,
        loc: &ir::InstrLocId,
    ) {
        if let ir::Instr::Binop(imm) = instr {
            if matches!(
                imm.op,
                ir::BinaryOp::I32DivS
                    | ir::BinaryOp::I32DivU
                    | ir::BinaryOp::I32RemS
                    | ir::BinaryOp::I32RemU
            ) {
                let rhs = &execution.state.value_stack[execution.state.value_stack.len() - 1];
                match rhs {
                    Val::Sym(val) => {
                        self.constraints
                            .insert(loc.data(), val.as_i32()._eq(&context.zero(32)));
                    }
                    Val::Conc(val) => {
                        let val = SymVal::from_concrete(&context.context, val);
                        self.constraints
                            .insert(loc.data(), val.as_i32()._eq(&context.zero(32)));
                    }
                }
                // trace!("{:?}", self.constraints);
            }
        }
    }

    fn run(
        &mut self,
        context: &'ctx Context,
        execution: &Execution<'ctx>,
        inputs: &HashMap<ir::LocalId, Val<'ctx>>,
    ) -> CheckResult {
        let solver = execution.get_solver(context);
        for (loc, constraint) in &self.constraints {
            solver.push();
            solver.assert(constraint);

            if solver.check() != z3::SatResult::Unsat {
                return CheckResult::Fail(format!(
                    "division by zero @ +{} with inputs {}",
                    loc,
                    Reporter::format_model(inputs, &solver.get_model().unwrap())
                ));
            }

            solver.pop(1);
        }

        CheckResult::Ok
    }
}

#[derive(Clone, Debug)]
pub struct MemoryCheck<'ctx> {
    constraints: HashMap<u32, z3::ast::Bool<'ctx>>,
}

impl<'ctx> MemoryCheck<'ctx> {
    pub fn new() -> Self {
        MemoryCheck {
            constraints: HashMap::new(),
        }
    }
}

impl<'ctx> Check<'ctx> for MemoryCheck<'ctx> {
    fn name(&self) -> &'static str {
        "Memory"
    }

    fn check(
        &mut self,
        context: &'ctx Context,
        execution: &Execution<'ctx>,
        instr: &ir::Instr,
        loc: &ir::InstrLocId,
    ) {
        if let ir::Instr::Load(imm) = instr {
            let memory = execution.state.memory.as_ref().unwrap();
            let size = &memory.size;

            let load_size = match imm.kind {
                ir::LoadKind::I32 { .. } => 32,
                _ => unimplemented!(),
            };
            let base_index = execution.state.value_stack.last().unwrap();
            let end_index = context
                .bin_op(
                    ir::BinaryOp::I32Add,
                    &Val::Conc(ConcVal(ir::Value::I32(load_size / 8))),
                    &base_index,
                )
                .unwrap();

            let byte_size = context
                .bin_op(
                    ir::BinaryOp::I32Mul,
                    &Val::Conc(ConcVal(ir::Value::I32(PAGE_SIZE as i32))),
                    size,
                )
                .unwrap();

            self.constraints.insert(
                loc.data(),
                end_index
                    .as_sym(&context.context)
                    .as_i32()
                    .bvsge(byte_size.as_sym(&context.context).as_i32()),
            );
        }
    }

    fn run(
        &mut self,
        context: &'ctx Context,
        execution: &Execution<'ctx>,
        inputs: &HashMap<ir::LocalId, Val<'ctx>>,
    ) -> CheckResult {
        let solver = execution.get_solver(context);
        for (loc, constraint) in &self.constraints {
            solver.push();
            solver.assert(constraint);

            if solver.check() != z3::SatResult::Unsat {
                return CheckResult::Fail(format!(
                    "memory out of bounds @ +{} with inputs {}",
                    loc,
                    Reporter::format_model(inputs, &solver.get_model().unwrap())
                ));
            }

            solver.pop(1);
        }

        CheckResult::Ok
    }
}
