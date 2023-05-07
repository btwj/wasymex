use crate::check::Check;
use crate::context::Context;
use crate::flow::{compute_info, Info};
use crate::memory::Memory;
use crate::reporter::Reporter;
use crate::state::{Execution, State, Status, TrapReason};
use crate::value::{ConcVal, SymVal, Val};
use log::{info, trace};
use std::collections::{HashMap, VecDeque};
use walrus::{ir, InstrLocId};
use z3::ast::Ast;

pub struct Engine<'ctx, 'm> {
    pub context: &'ctx Context<'m>,
    func: Option<&'m walrus::LocalFunction>,
    info: Info,
    executions: VecDeque<Execution<'ctx>>,
    checks: Vec<Box<dyn Check<'ctx> + 'ctx>>,
    max_loop_iters: usize,
}

impl<'ctx, 'm> Engine<'ctx, 'm> {
    pub fn new(context: &'ctx Context<'m>) -> Self {
        Engine {
            context,
            func: None,
            info: Info::default(),
            executions: VecDeque::new(),
            checks: Vec::new(),
            max_loop_iters: 1,
        }
    }

    pub fn set_max_loop_iters(&mut self, max_loop_iters: usize) {
        self.max_loop_iters = max_loop_iters;
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
                    let info = compute_info(local_func);
                    self.info = info;
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

        for local in self.info.locals.iter() {
            if !state.locals.contains_key(local) {
                let local_ty = self.context.module.locals.get(*local).ty();
                state
                    .locals
                    .insert(*local, Val::Conc(ConcVal::from_valtype(local_ty)));
            }
        }

        for memory in self.context.module.memories.iter() {
            state.memory = Some(Memory::new(&self.context.context, memory.initial));
        }

        let mut execution = Execution::new(state, func.entry_block());
        for check in &self.checks {
            execution.add_check(dyn_clone::clone_box(&**check));
        }

        self.push_execution(execution);

        let context = self.context;
        let reporter = Reporter::new();

        let mut executions = self.collect_executions();
        executions
            .iter_mut()
            .for_each(|execution| execution.state.simplify());

        reporter.report_func(name);
        reporter.report_executions(self.context, &executions);

        let mut completed_executions = executions
            .into_iter()
            .filter(|execution| matches!(execution.status, Status::Complete | Status::Trap(_)))
            .collect();

        reporter.report_checks(context, &inputs, &mut completed_executions);
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

    fn do_jump_to_seq(&mut self, execution: &mut Execution<'ctx>, seq_id: &ir::InstrSeqId) {
        execution.cur_block = *seq_id;
        execution.cur_location = None;
    }

    fn do_branch(&mut self, execution: &mut Execution<'ctx>, block: &ir::InstrSeqId) {
        let block_loc = self.info.ends.get(block).unwrap();
        let block_instr = self.info.types.get(block).unwrap();

        match block_instr {
            ir::Instr::Block(_) => {
                execution.cur_block = block_loc.block;
                execution.cur_location = Some(InstrLocId::new(block_loc.loc));
            }
            ir::Instr::Loop(_) => {
                execution.cur_block = *block;
                execution.cur_location = None;
            }
            _ => unreachable!(),
        }
    }

    pub fn step_execution(&mut self, mut execution: Execution<'ctx>) -> Option<Execution<'ctx>> {
        let func = self.func.unwrap();
        let cur_block = func.block(execution.cur_block);

        let mut skipped = execution.cur_location.is_none();
        if execution.cur_location.is_none() {
            *execution.hotness.entry(cur_block.id()).or_insert(0) += 1;
        }

        if *execution.hotness.get(&cur_block.id()).unwrap() > self.max_loop_iters {
            execution.status = Status::Terminated;
            return Some(execution);
        }

        for (instr, instr_loc) in &cur_block.instrs {
            // Skip execution to current location
            if let Some(cur_location) = execution.cur_location {
                if !skipped && cur_location.data() != instr_loc.data() {
                    continue;
                }
            }
            skipped = true;
            execution.cur_location = Some(*instr_loc);

            trace!("  #{} {:?}", execution.id, instr);

            let mut execution_checks = std::mem::take(&mut execution.checks);
            for check in &mut execution_checks {
                check.check(self.context, &execution, instr, instr_loc);
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
                // Control flow
                ir::Instr::Block(ir::Block { seq }) | ir::Instr::Loop(ir::Loop { seq }) => {
                    self.do_jump_to_seq(&mut execution, seq);
                    self.push_execution(execution);
                    return None;
                }
                ir::Instr::Br(imm) => {
                    self.do_branch(&mut execution, &imm.block);
                    self.push_execution(execution);
                    return None;
                }
                ir::Instr::BrIf(imm) => {
                    let condition = execution.state.value_stack.pop().unwrap();
                    match condition {
                        Val::Conc(val) => {
                            if val.as_i32() != 0 {
                                self.do_branch(&mut execution, &imm.block);
                                self.push_execution(execution);
                                return None;
                            }
                        }
                        Val::Sym(val) => {
                            let mut true_execution = Execution::from(&execution);
                            true_execution
                                .constraints
                                .push(val.as_i32()._eq(&self.zero(32)).not());
                            self.do_branch(&mut true_execution, &imm.block);

                            execution.constraints.push(val.as_i32()._eq(&self.zero(32)));

                            trace!(
                                "Forking execution #{} on {:?} -> [true: #{}/false: #{}]",
                                execution.id,
                                val,
                                true_execution.id,
                                execution.id
                            );

                            self.push_execution(true_execution);
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
                            true_execution.cur_location = None;

                            let mut false_execution = Execution::from(&execution);
                            false_execution
                                .constraints
                                .push(val.as_i32()._eq(&self.zero(32)));
                            false_execution.cur_block = imm.alternative;
                            false_execution.cur_location = None;

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
                ir::Instr::Return(_) => {
                    execution.status = Status::Complete;
                    return Some(execution);
                }
                // Memory Instructions
                ir::Instr::MemorySize(_) => {
                    let size = execution.state.memory.as_ref().unwrap().size.clone();
                    execution.state.value_stack.push(size);
                }
                ir::Instr::MemoryGrow(_) => {
                    let size = execution.state.memory.as_ref().unwrap().size.clone();
                    let num_pages = execution.state.value_stack.pop().unwrap();
                    let new_size = self
                        .bin_op(ir::BinaryOp::I32Add, &size, &num_pages)
                        .unwrap();
                    execution.state.memory.as_mut().unwrap().size = new_size;
                    execution.state.value_stack.push(num_pages);
                }
                ir::Instr::Load(imm) => {
                    let memory = execution.state.memory.as_ref().unwrap();
                    let offset = imm.arg.offset as i32;
                    let index = execution.state.value_stack.pop().unwrap();
                    let access_index = self
                        .bin_op(
                            ir::BinaryOp::I32Add,
                            &Val::Conc(ConcVal(ir::Value::I32(offset))),
                            &index,
                        )
                        .unwrap();

                    let value = match imm.kind {
                        ir::LoadKind::I32 { .. } => {
                            self.do_load(&memory, &access_index, 32, 32, false)
                        }
                        ir::LoadKind::I32_8 {
                            kind: ir::ExtendedLoad::ZeroExtend,
                        } => self.do_load(&memory, &access_index, 32, 8, true),
                        ir::LoadKind::I32_8 { .. } => {
                            self.do_load(&memory, &access_index, 32, 8, false)
                        }
                        ir::LoadKind::I32_16 {
                            kind: ir::ExtendedLoad::ZeroExtend,
                        } => self.do_load(&memory, &access_index, 32, 16, true),
                        ir::LoadKind::I32_16 { .. } => {
                            self.do_load(&memory, &access_index, 32, 16, false)
                        }
                        _ => unimplemented!(),
                    };
                    execution.state.value_stack.push(value);
                }
                ir::Instr::Store(imm) => {
                    let memory = execution.state.memory.as_mut().unwrap();
                    let offset = imm.arg.offset as i32;
                    let index = execution.state.value_stack.pop().unwrap();
                    let access_index = self
                        .bin_op(
                            ir::BinaryOp::I32Add,
                            &Val::Conc(ConcVal(ir::Value::I32(offset))),
                            &index,
                        )
                        .unwrap();

                    let value = execution.state.value_stack.pop().unwrap();
                    match imm.kind {
                        ir::StoreKind::I32 { .. } => {
                            self.do_store(memory, &access_index, value, 32)
                        }
                        _ => unimplemented!(),
                    }
                }
                _ => unimplemented!(),
            }

            execution.state.simplify();
            trace!("      -> {}", execution.state);
        }

        match self.info.ends.get(&cur_block.id()) {
            None => {
                execution.status = Status::Complete;
                Some(execution)
            }
            Some(end) => {
                execution.cur_block = end.block;
                execution.cur_location = Some(ir::InstrLocId::new(end.loc));
                self.push_execution(execution);
                None
            }
        }
    }
}

impl<'ctx, 'm> Engine<'ctx, 'm> {
    pub fn zero(&self, size: u32) -> z3::ast::BV<'ctx> {
        self.context.zero(size)
    }

    pub fn bin_op(
        &self,
        op: ir::BinaryOp,
        lhs: &Val<'ctx>,
        rhs: &Val<'ctx>,
    ) -> Result<Val<'ctx>, TrapReason> {
        self.context.bin_op(op, lhs, rhs)
    }
}
