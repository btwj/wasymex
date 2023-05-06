use crate::check::{Check, CheckResult};
use crate::context::Context;
use crate::reporter::Reporter;
use crate::state::{Execution, State, Status, TrapReason};
use crate::value::{ConcVal, SymVal, Val};
use log::{debug, info, trace};
use std::collections::{HashMap, VecDeque};
use walrus::ir;
use z3::ast::Ast;

pub struct Engine<'ctx, 'm> {
    context: &'ctx Context<'m>,
    func: Option<&'m walrus::LocalFunction>,
    executions: VecDeque<Execution<'ctx>>,
    checks: Vec<Box<dyn Check<'ctx> + 'ctx>>,
}

impl<'ctx, 'm> Engine<'ctx, 'm> {
    pub fn new(context: &'ctx Context<'m>) -> Self {
        Engine {
            context,
            func: None,
            executions: VecDeque::new(),
            checks: Vec::new(),
        }
    }

    pub fn add_check(&mut self, check: Box<dyn Check<'ctx> + 'ctx>) {
        self.checks.push(check);
    }

    pub fn analyze_module(&mut self) {
        for func in self.context.module.funcs.iter() {
            let name = func
                .name
                .clone()
                .unwrap_or(format!("#{}", func.id().index()));
            match &func.kind {
                walrus::FunctionKind::Import(_) => info!("Skipping import function {}", name),
                walrus::FunctionKind::Uninitialized(_) => {
                    info!("Skipping uninitialized function {}", name)
                }
                walrus::FunctionKind::Local(local_func) => {
                    self.analyze_func(local_func, &name);
                }
            }
        }
    }

    pub fn analyze_func(&mut self, func: &'m walrus::LocalFunction, name: &str) {
        info!("Analyzing function #{}", name);

        self.func = Some(func);
        let mut inputs = HashMap::new();

        let mut state = State::new();
        for param_id in func.args.iter() {
            let param = self.context.module.locals.get(*param_id);
            let param_ty = param.ty();
            let symbolic_param = Val::Sym(SymVal::from_valtype(
                &self.context.context,
                param_ty,
                format!("local{}", param_id.index()),
            ));
            state.locals.insert(*param_id, symbolic_param.clone());
            inputs.insert(*param_id, symbolic_param);
        }
        let mut execution = Execution::new(state, func.entry_block());
        for check in &self.checks {
            execution.add_check(dyn_clone::clone_box(&**check));
        }

        self.push_execution(execution);

        let context = self.context;
        let reporter = Reporter::new();

        let mut completed_executions = self.collect_executions();
        reporter.report_func(name);
        reporter.report_executions(&self.context, &completed_executions);
        reporter.report_checks(&context, &inputs, &mut completed_executions);
    }

    pub fn push_execution(&mut self, execution: Execution<'ctx>) {
        self.executions.push_back(execution);
    }

    pub fn collect_executions(&mut self) -> Vec<Execution<'ctx>> {
        let mut completed_executions = Vec::<Execution>::new();
        while let Some(execution) = self.executions.pop_front() {
            match self.step_execution(execution) {
                Some(execution) => {
                    completed_executions.push(execution);
                }
                None => (),
            }
        }

        completed_executions
    }

    pub fn step_execution(&mut self, mut execution: Execution<'ctx>) -> Option<Execution<'ctx>> {
        let func = self.func.unwrap();
        let cur_block = func.block(execution.cur_block);

        for (instr, instr_loc) in &cur_block.instrs {
            trace!("  #{} {:?}", execution.id, instr);

            let mut execution_checks = std::mem::take(&mut execution.checks);
            for check in &mut execution_checks {
                check.check(&self.context, &execution, instr, instr_loc);
            }
            execution.checks = execution_checks;

            match instr {
                ir::Instr::Drop(_) => {
                    execution.state.value_stack.pop().unwrap();
                }
                ir::Instr::Binop(imm) => {
                    let rhs = execution.state.value_stack.pop().unwrap();
                    let lhs = execution.state.value_stack.pop().unwrap();
                    match self.bin_op(imm.op, &lhs, &rhs) {
                        Ok(result) => {
                            execution.state.value_stack.push(result);
                        }
                        Err(trap) => {
                            execution.status = Status::Trap(trap);
                            return Some(execution);
                        }
                    }
                }
                ir::Instr::Const(imm) => {
                    execution
                        .state
                        .value_stack
                        .push(Val::Conc(ConcVal(imm.value)));
                }
                ir::Instr::LocalGet(imm) => {
                    let local = execution.state.locals.get(&imm.local).unwrap();
                    execution.state.value_stack.push(local.clone());
                }
                ir::Instr::LocalSet(imm) => {
                    let value = execution.state.value_stack.pop().unwrap();
                    execution.state.locals.insert(imm.local, value.clone());
                }
                ir::Instr::Br(imm) => {
                    execution.cur_block = imm.block;
                    self.push_execution(execution);
                    return None;
                }
                ir::Instr::BrIf(imm) => {
                    let condition = execution.state.value_stack.pop().unwrap();
                    match condition {
                        Val::Conc(val) => {
                            if val.as_i32() == 0 {
                                execution.cur_block = imm.block;
                                self.push_execution(execution);
                                return None;
                            }
                        }
                        Val::Sym(val) => {
                            let mut true_execution = Execution::from(&execution);
                            true_execution
                                .constraints
                                .push(val.as_i32()._eq(&self.zero(32)).not());
                            true_execution.cur_block = imm.block;

                            trace!(
                                "Forking execution #{} on {:?} -> [true: #{}/false: #{}]",
                                execution.id,
                                val,
                                true_execution.id,
                                execution.id
                            );

                            self.push_execution(true_execution);
                            self.push_execution(execution);
                            return None;
                        }
                    }
                }
                ir::Instr::IfElse(imm) => {
                    let condition = execution.state.value_stack.pop().unwrap();
                    match condition {
                        Val::Conc(val) => {
                            if val.as_i32() != 0 {
                                execution.cur_block = imm.consequent;
                            } else {
                                execution.cur_block = imm.alternative;
                            }
                        }
                        Val::Sym(val) => {
                            let mut true_execution = Execution::from(&execution);
                            true_execution
                                .constraints
                                .push(val.as_i32()._eq(&self.zero(32)).not());
                            true_execution.cur_block = imm.consequent;

                            let mut false_execution = Execution::from(&execution);
                            false_execution
                                .constraints
                                .push(val.as_i32()._eq(&self.zero(32)));
                            false_execution.cur_block = imm.alternative;

                            trace!(
                                "Forking execution #{} on {:?} -> [true: #{}/false: #{}]",
                                execution.id,
                                val,
                                true_execution.id,
                                false_execution.id
                            );

                            self.push_execution(true_execution);
                            self.push_execution(false_execution);
                            return None;
                        }
                    }
                }
                _ => unimplemented!(),
            }

            // trace!("      -> {}", execution.state);
        }
        execution.status = Status::Complete;
        return Some(execution);
    }
}

impl<'ctx, 'm> Engine<'ctx, 'm> {
    pub fn zero(&self, size: u32) -> z3::ast::BV<'ctx> {
        self.context.zero(size)
    }
    pub fn one(&self, size: u32) -> z3::ast::BV<'ctx> {
        self.context.one(size)
    }

    pub fn bin_conc(
        &self,
        op: ir::BinaryOp,
        lhs: &ConcVal,
        rhs: &ConcVal,
    ) -> Result<ConcVal, TrapReason> {
        Ok(ConcVal(match op {
            ir::BinaryOp::I32Add => ir::Value::I32(lhs.as_i32().wrapping_add(rhs.as_i32())),
            ir::BinaryOp::I32Sub => ir::Value::I32(lhs.as_i32().wrapping_sub(rhs.as_i32())),
            ir::BinaryOp::I32Mul => ir::Value::I32(lhs.as_i32().wrapping_mul(rhs.as_i32())),
            ir::BinaryOp::I32DivS => match lhs.as_i32().checked_div(rhs.as_i32()) {
                Some(value) => ir::Value::I32(value),
                None => return Err(TrapReason::DivisionByZero),
            },
            ir::BinaryOp::I32DivU => match (lhs.as_i32() as u32).checked_div(rhs.as_i32() as u32) {
                Some(value) => ir::Value::I32(value as i32),
                None => return Err(TrapReason::DivisionByZero),
            },
            ir::BinaryOp::I32LtU => {
                ir::Value::I32(i32::from((lhs.as_i32() as u32) < (rhs.as_i32() as u32)))
            }
            ir::BinaryOp::I32LtS => ir::Value::I32(i32::from(lhs.as_i32() < rhs.as_i32())),
            ir::BinaryOp::I32GtS => ir::Value::I32(i32::from(lhs.as_i32() > rhs.as_i32())),
            ir::BinaryOp::I32LeS => ir::Value::I32(i32::from(lhs.as_i32() <= rhs.as_i32())),
            ir::BinaryOp::I32GeS => ir::Value::I32(i32::from(lhs.as_i32() >= rhs.as_i32())),
            _ => panic!(),
        }))
    }

    pub fn bin_sym(
        &self,
        op: ir::BinaryOp,
        lhs: &SymVal<'ctx>,
        rhs: &SymVal<'ctx>,
    ) -> SymVal<'ctx> {
        match op {
            ir::BinaryOp::I32Add => SymVal::I32(lhs.as_i32().bvadd(rhs.as_i32())),
            ir::BinaryOp::I32Sub => SymVal::I32(lhs.as_i32().bvsub(rhs.as_i32())),
            ir::BinaryOp::I32Mul => SymVal::I32(lhs.as_i32().bvmul(rhs.as_i32())),
            ir::BinaryOp::I32DivS => SymVal::I32(lhs.as_i32().bvsdiv(rhs.as_i32())),
            ir::BinaryOp::I32DivU => todo!(),
            ir::BinaryOp::I32LtS => SymVal::I32(
                lhs.as_i32()
                    .bvslt(rhs.as_i32())
                    .ite(&self.one(32), &self.zero(32)),
            ),
            ir::BinaryOp::I32LeS => SymVal::I32(
                lhs.as_i32()
                    .bvsle(rhs.as_i32())
                    .ite(&self.one(32), &self.zero(32)),
            ),
            ir::BinaryOp::I32GtS => SymVal::I32(
                lhs.as_i32()
                    .bvsgt(rhs.as_i32())
                    .ite(&self.one(32), &self.zero(32)),
            ),
            ir::BinaryOp::I32GeS => SymVal::I32(
                lhs.as_i32()
                    .bvsge(rhs.as_i32())
                    .ite(&self.one(32), &self.zero(32)),
            ),
            _ => todo!(),
        }
    }

    pub fn bin_op(
        &self,
        op: ir::BinaryOp,
        lhs: &Val<'ctx>,
        rhs: &Val<'ctx>,
    ) -> Result<Val<'ctx>, TrapReason> {
        Ok(match (lhs, rhs) {
            (Val::Conc(lhs_val), Val::Conc(rhs_val)) => {
                Val::Conc(self.bin_conc(op, lhs_val, rhs_val)?)
            }
            (Val::Sym(lhs_val), Val::Conc(rhs_val)) => {
                let rhs_val: SymVal = SymVal::from_concrete(&self.context.context, rhs_val);
                Val::Sym(self.bin_sym(op, lhs_val, &rhs_val))
            }
            (Val::Conc(lhs_val), Val::Sym(rhs_val)) => {
                let lhs_val: SymVal = SymVal::from_concrete(&self.context.context, lhs_val);
                Val::Sym(self.bin_sym(op, &lhs_val, rhs_val))
            }
            (Val::Sym(lhs_val), Val::Sym(rhs_val)) => Val::Sym(self.bin_sym(op, lhs_val, rhs_val)),
        })
    }
}
