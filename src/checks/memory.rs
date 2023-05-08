use crate::checks::{Check, CheckResult};
use crate::context::Context;
use crate::memory::PAGE_SIZE;
use crate::reporter::Reporter;
use crate::state::Execution;
use crate::value::{ConcVal, Val};
use std::collections::HashMap;
use walrus::ir;

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
            let frame = execution.state.call_stack.last().unwrap();
            let memory = execution.state.memory.as_ref().unwrap();
            let size = &memory.size;

            let load_size = match imm.kind {
                ir::LoadKind::I32 { .. } => 32,
                ir::LoadKind::I32_8 { .. } => 8,
                ir::LoadKind::I32_16 { .. } => 16,
                _ => unimplemented!(),
            };
            let base_index = frame.value_stack.last().unwrap();
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
