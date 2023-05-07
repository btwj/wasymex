use crate::checks::{Check, CheckResult};
use crate::context::Context;
use crate::reporter::Reporter;
use crate::state::Execution;
use crate::value::{SymVal, Val};
use std::collections::HashMap;
use walrus::ir;
use z3::ast::Ast;

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
            let frame = execution.state.call_stack.last().unwrap();
            if matches!(
                imm.op,
                ir::BinaryOp::I32DivS
                    | ir::BinaryOp::I32DivU
                    | ir::BinaryOp::I32RemS
                    | ir::BinaryOp::I32RemU
            ) {
                let rhs = &frame.value_stack[frame.value_stack.len() - 1];
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
