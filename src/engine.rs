use crate::checks::Check;
use crate::context::Context;
use crate::flow::{compute_info, Info, Loc};
use crate::memory::Memory;
use crate::reporter::Reporter;
use crate::state::{Execution, Frame, State, Status, TrapReason};
use crate::value::{ConcVal, SymVal, Val};
use log::{info, trace};
use std::collections::{HashMap, VecDeque};
use walrus::{ir, InstrLocId};
use z3::ast::Ast;

pub struct Engine<'ctx, 'm> {
    pub context: &'ctx Context<'m>,
    info: Vec<Option<Info>>,
    executions: VecDeque<Execution<'ctx>>,
    checks: Vec<Box<dyn Check<'ctx> + 'ctx>>,
    max_hotness: usize,
}

pub fn as_local_func(func: &walrus::Function) -> Option<&walrus::LocalFunction> {
    match &func.kind {
        walrus::FunctionKind::Local(local_func) => Some(local_func),
        _ => None,
    }
}

impl<'ctx, 'm> Engine<'ctx, 'm> {
    pub fn new(context: &'ctx Context<'m>) -> Self {
        Engine {
            context,
            info: vec![None; context.module.funcs.iter().count()],
            executions: VecDeque::new(),
            checks: Vec::new(),
            max_hotness: 1,
        }
    }

    pub fn set_max_hotness(&mut self, max_hotness: usize) {
        self.max_hotness = max_hotness;
    }

    pub fn add_check(&mut self, check: Box<dyn Check<'ctx> + 'ctx>) {
        self.checks.push(check);
    }

    pub fn get_inputs(&self, func: &'m walrus::LocalFunction) -> HashMap<ir::LocalId, Val<'ctx>> {
        let mut inputs = HashMap::new();
        for param_id in func.args.iter() {
            let param = self.context.module.locals.get(*param_id);
            let param_ty = param.ty();
            let symbolic_param = Val::Sym(SymVal::from_valtype(
                &self.context.context,
                param_ty,
                format!("local{}", param_id.index()),
            ));
            inputs.insert(*param_id, symbolic_param);
        }
        inputs
    }

    pub fn initialize(&mut self) {
        for func in self.context.module.funcs.iter() {
            let info = match &func.kind {
                walrus::FunctionKind::Import(_) => None,
                walrus::FunctionKind::Uninitialized(_) => None,
                walrus::FunctionKind::Local(local_func) => Some(compute_info(local_func)),
            };
            self.info[func.id().index()] = info;
        }
    }

    pub fn get_initial_execution(
        &mut self,
        func: &'m walrus::LocalFunction,
        id: walrus::FunctionId,
    ) -> Execution<'ctx> {
        let inputs = self.get_inputs(func);
        let mut frame = Frame::new(id, None);

        frame.locals.extend(inputs.clone());

        let info = self.info[id.index()].as_ref().unwrap();
        for local in info.locals.iter() {
            if !frame.locals.contains_key(local) {
                let local_ty = self.context.module.locals.get(*local).ty();
                frame
                    .locals
                    .insert(*local, Val::Conc(ConcVal::from_valtype(local_ty)));
            }
        }

        let mut state = State::new();
        state.call_stack.push(frame);
        for memory in self.context.module.memories.iter() {
            state.memory = Some(Memory::new(&self.context.context, memory.initial));
        }

        let execution = Execution::new(state, func.entry_block());
        execution
    }

    pub fn get_func_executions(
        &mut self,
        func: &'m walrus::LocalFunction,
        id: walrus::FunctionId,
        initial: Option<Execution<'ctx>>,
    ) -> Vec<Execution<'ctx>> {
        let mut execution = match initial {
            None => self.get_initial_execution(func, id),
            Some(execution) => execution,
        };

        for check in &self.checks {
            execution.add_check(dyn_clone::clone_box(&**check));
        }

        self.push_execution(execution);
        self.collect_executions()
    }

    pub fn analyze_func(
        &mut self,
        func: &'m walrus::LocalFunction,
        id: walrus::FunctionId,
        name: &str,
    ) {
        info!("Analyzing function #{}", name);

        let mut executions = self.get_func_executions(func, id, None);
        let inputs = self.get_inputs(func);
        executions
            .iter_mut()
            .for_each(|execution| execution.state.simplify());

        let reporter = Reporter::new();
        reporter.report_func(name);
        reporter.report_executions(self.context, &executions);

        let mut completed_executions = executions
            .into_iter()
            .filter(|execution| matches!(execution.status, Status::Complete | Status::Trap(_)))
            .collect();

        reporter.report_checks(self.context, &inputs, &mut completed_executions);
    }

    pub fn push_execution(&mut self, execution: Execution<'ctx>) {
        self.executions.push_back(execution);
    }

    fn collect_executions(&mut self) -> Vec<Execution<'ctx>> {
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

    fn do_jump_to_seq(&self, execution: &mut Execution<'ctx>, seq_id: &ir::InstrSeqId) {
        execution.cur_block = *seq_id;
        execution.cur_location = None;
    }

    fn do_branch(&self, execution: &mut Execution<'ctx>, block: &ir::InstrSeqId) -> bool {
        let info = self.info[execution.state.call_stack.last().unwrap().func.index()]
            .as_ref()
            .unwrap();

        let block_loc = match info.ends.get(block) {
            None => return true,
            Some(end) => end,
        };
        let block_instr = info.types.get(block).unwrap();

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
        return false;
    }

    pub fn step_execution(&mut self, mut execution: Execution<'ctx>) -> Option<Execution<'ctx>> {
        let frame = execution.state.call_stack.last().unwrap();
        let func_id = frame.func;
        let func = as_local_func(self.context.module.funcs.get(func_id)).unwrap();
        let cur_block = func.block(execution.cur_block);

        let mut skipped = execution.cur_location.is_none();
        if execution.cur_location.is_none() {
            *execution.hotness.entry(cur_block.id()).or_insert(0) += 1;
        }

        if *execution.hotness.get(&cur_block.id()).unwrap() > self.max_hotness {
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

            if execution.advance {
                execution.advance = false;
                continue;
            }

            trace!("  #{} {:?}", execution.id, instr);

            let mut execution_checks = std::mem::take(&mut execution.checks);
            for check in &mut execution_checks {
                check.check(self.context, &execution, instr, instr_loc);
            }
            execution.checks = execution_checks;

            let frame = execution.state.call_stack.last_mut().unwrap();
            match instr {
                ir::Instr::Drop(_) => {
                    frame.value_stack.pop().unwrap();
                }
                ir::Instr::Unop(imm) => {
                    let op = frame.value_stack.pop().unwrap();
                    match self.un_op(imm.op, &op) {
                        Ok(result) => {
                            frame.value_stack.push(result);
                        }
                        Err(trap) => {
                            execution.status = Status::Trap(trap);
                            return Some(execution);
                        }
                    }
                }
                ir::Instr::Binop(imm) => {
                    let rhs = frame.value_stack.pop().unwrap();
                    let lhs = frame.value_stack.pop().unwrap();
                    match self.bin_op(imm.op, &lhs, &rhs) {
                        Ok(result) => {
                            frame.value_stack.push(result);
                        }
                        Err(trap) => {
                            execution.status = Status::Trap(trap);
                            return Some(execution);
                        }
                    }
                }
                ir::Instr::Const(imm) => {
                    frame.value_stack.push(Val::Conc(ConcVal(imm.value)));
                }
                ir::Instr::LocalGet(imm) => {
                    let local = frame.locals.get(&imm.local).unwrap();
                    frame.value_stack.push(local.clone());
                }
                ir::Instr::LocalSet(imm) => {
                    let value = frame.value_stack.pop().unwrap();
                    frame.locals.insert(imm.local, value.clone());
                }
                ir::Instr::LocalTee(imm) => {
                    let value = frame.value_stack.last().unwrap();
                    frame.locals.insert(imm.local, value.clone());
                }
                ir::Instr::Select(imm) => {
                    let cond = frame.value_stack.pop().unwrap();
                    let rhs = frame
                        .value_stack
                        .pop()
                        .unwrap()
                        .as_sym(&self.context.context);
                    let lhs = frame
                        .value_stack
                        .pop()
                        .unwrap()
                        .as_sym(&self.context.context);

                    let sym_cond = cond.as_sym(&self.context.context);
                    let sym_val = sym_cond
                        .as_i32()
                        ._eq(&self.zero(32))
                        .ite(lhs.as_i32(), rhs.as_i32());
                    frame.value_stack.push(Val::Sym(SymVal::I32(sym_val)));
                }
                // Globals (TODO)
                ir::Instr::GlobalGet(imm) => {
                    frame
                        .value_stack
                        .push(Val::Conc(ConcVal(ir::Value::I32(0))));
                }
                ir::Instr::GlobalSet(imm) => {
                    frame.value_stack.pop();
                }
                // Control flow
                ir::Instr::Block(ir::Block { seq }) | ir::Instr::Loop(ir::Loop { seq }) => {
                    self.do_jump_to_seq(&mut execution, seq);
                    self.push_execution(execution);
                    return None;
                }
                ir::Instr::Br(imm) => {
                    if self.do_branch(&mut execution, &imm.block) {
                        break;
                    }
                    self.push_execution(execution);
                    return None;
                }
                ir::Instr::BrIf(imm) => {
                    let condition = frame.value_stack.pop().unwrap();
                    match condition {
                        Val::Conc(val) => {
                            if val.as_i32() != 0 {
                                if self.do_branch(&mut execution, &imm.block) {
                                    break;
                                }
                                self.push_execution(execution);
                                return None;
                            }
                        }
                        Val::Sym(val) => {
                            let mut true_execution = Execution::from(&execution);
                            true_execution
                                .constraints
                                .push(val.as_i32()._eq(&self.zero(32)).not());
                            if self.do_branch(&mut true_execution, &imm.block) {
                                break;
                            }

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
                    let condition = frame.value_stack.pop().unwrap();
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
                ir::Instr::Call(imm) => {
                    let func_id = imm.func;
                    let func = self.context.module.funcs.get(func_id);
                    let local_func = as_local_func(func).unwrap();
                    let mut inputs = Vec::new();
                    for _ in local_func.args.iter() {
                        inputs.push(frame.value_stack.pop().unwrap());
                    }
                    inputs.reverse();

                    let mut frame = Frame::new(
                        func_id,
                        Some(Loc {
                            block: execution.cur_block,
                            loc: instr_loc.data(),
                        }),
                    );
                    for (param, value) in std::iter::zip(local_func.args.iter(), inputs.into_iter())
                    {
                        frame.locals.insert(*param, value);
                    }

                    let info = self.info[func_id.index()].as_ref().unwrap();
                    for local in info.locals.iter() {
                        if !frame.locals.contains_key(local) {
                            let local_ty = self.context.module.locals.get(*local).ty();
                            frame
                                .locals
                                .insert(*local, Val::Conc(ConcVal::from_valtype(local_ty)));
                        }
                    }

                    execution.state.call_stack.push(frame);
                    execution.cur_block = local_func.entry_block();
                    execution.cur_location = None;
                    trace!("      -> {}", execution.state);
                    self.push_execution(execution);
                    return None;
                }
                ir::Instr::Return(_) => {
                    execution.status = Status::Complete;
                    return Some(execution);
                }
                // Memory Instructions
                ir::Instr::MemorySize(_) => {
                    let size = execution.state.memory.as_ref().unwrap().size.clone();
                    frame.value_stack.push(size);
                }
                ir::Instr::MemoryGrow(_) => {
                    let size = execution.state.memory.as_ref().unwrap().size.clone();
                    let num_pages = frame.value_stack.pop().unwrap();
                    let new_size = self
                        .bin_op(ir::BinaryOp::I32Add, &size, &num_pages)
                        .unwrap();
                    execution.state.memory.as_mut().unwrap().size = new_size;
                    frame.value_stack.push(num_pages);
                }
                ir::Instr::Load(imm) => {
                    let memory = execution.state.memory.as_ref().unwrap();
                    let offset = imm.arg.offset as i32;
                    let index = frame.value_stack.pop().unwrap();
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
                        } => self.do_load(&memory, &access_index, 8, 32, true),
                        ir::LoadKind::I32_8 { .. } => {
                            self.do_load(&memory, &access_index, 8, 32, false)
                        }
                        ir::LoadKind::I32_16 {
                            kind: ir::ExtendedLoad::ZeroExtend,
                        } => self.do_load(&memory, &access_index, 16, 32, true),
                        ir::LoadKind::I32_16 { .. } => {
                            self.do_load(&memory, &access_index, 16, 32, false)
                        }
                        _ => unimplemented!(),
                    };
                    frame.value_stack.push(value);
                }
                ir::Instr::Store(imm) => {
                    let memory = execution.state.memory.as_mut().unwrap();
                    let offset = imm.arg.offset as i32;
                    let value = frame.value_stack.pop().unwrap();
                    let index = frame.value_stack.pop().unwrap();
                    let access_index = self
                        .bin_op(
                            ir::BinaryOp::I32Add,
                            &Val::Conc(ConcVal(ir::Value::I32(offset))),
                            &index,
                        )
                        .unwrap();

                    match imm.kind {
                        ir::StoreKind::I32 { .. } => {
                            self.do_store(memory, &access_index, value, 32)
                        }
                        ir::StoreKind::I32_8 { .. } => {
                            self.do_store(memory, &access_index, value, 8)
                        }
                        ir::StoreKind::I32_16 { .. } => {
                            self.do_store(memory, &access_index, value, 16)
                        }
                        _ => unimplemented!(),
                    }
                }
                _ => unimplemented!(),
            }

            execution.state.simplify();
            trace!("      -> {}", execution.state);
        }

        execution.advance = false;
        let info = self.info[func_id.index()].as_ref().unwrap();
        match info.ends.get(&cur_block.id()) {
            None => {
                let old_frame = execution.state.call_stack.pop().unwrap();
                match execution.state.call_stack.last_mut() {
                    None => {
                        execution.status = Status::Complete;
                        execution.state.call_stack.push(old_frame);
                        Some(execution)
                    }
                    Some(prev_frame) => {
                        let ret = old_frame.ret.unwrap();
                        execution.cur_block = ret.block;
                        execution.cur_location = Some(InstrLocId::new(ret.loc));
                        execution.advance = true;
                        prev_frame.value_stack.extend(old_frame.value_stack);
                        self.push_execution(execution);
                        return None;
                    }
                }
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

    pub fn un_op(&self, op: ir::UnaryOp, operand: &Val<'ctx>) -> Result<Val<'ctx>, TrapReason> {
        self.context.un_op(op, operand)
    }
}
