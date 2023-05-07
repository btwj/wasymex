use crate::check::Check;
use crate::context::Context;
use crate::value::Val;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use walrus::ir;
use z3;

#[derive(Debug, Clone, PartialEq)]
pub enum TrapReason {
    DivisionByZero,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    None,
    Complete,
    Trap(TrapReason),
    Terminated,
}

#[derive(Debug, Clone)]
pub struct Memory<'ctx> {
    pub size: u32, // size in bytes
    pub array: z3::ast::Array<'ctx>,
}

impl<'ctx> Memory<'ctx> {
    pub fn new(context: &'ctx z3::Context, initial: u32) -> Self {
        Memory {
            size: initial,
            array: z3::ast::Array::const_array(
                context,
                &z3::Sort::bitvector(context, 8),
                &z3::ast::BV::from_i64(context, 0, 8),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct State<'ctx> {
    pub value_stack: Vec<Val<'ctx>>,
    pub locals: HashMap<ir::LocalId, Val<'ctx>>,
    pub memory: Option<Memory<'ctx>>,
}

impl<'ctx> State<'ctx> {
    pub fn new() -> Self {
        State {
            value_stack: Vec::new(),
            locals: HashMap::new(),
            memory: None,
        }
    }
}

impl<'ctx> std::fmt::Display for State<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{stack=[{}], locals=[{}]{}}}",
            self.value_stack
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            self.locals
                .iter()
                .map(|(k, v)| format!("#{}={}", k.index(), v))
                .collect::<Vec<_>>()
                .join(", "),
            match &self.memory {
                None => String::from(""),
                Some(memory) => format!(", memory={:?}", memory),
            }
        )
    }
}

pub struct Loc {
    block: ir::InstrSeqId,
    loc: ir::InstrLocId,
}

#[derive(Debug, Clone)]
pub struct Execution<'ctx> {
    pub id: usize,
    pub state: State<'ctx>,
    pub constraints: Vec<z3::ast::Bool<'ctx>>,
    pub cur_block: ir::InstrSeqId,
    pub cur_location: Option<ir::InstrLocId>, // None if start of block
    pub status: Status,
    pub checks: Vec<Box<dyn Check<'ctx> + 'ctx>>,
    pub hotness: HashMap<ir::InstrSeqId, usize>,
}

static EXECUTION_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl<'ctx> Execution<'ctx> {
    pub fn new(state: State<'ctx>, entry: ir::InstrSeqId) -> Self {
        Execution {
            id: EXECUTION_COUNTER.fetch_add(1, Ordering::SeqCst),
            constraints: Vec::new(),
            state,
            cur_block: entry,
            cur_location: None,
            status: Status::None,
            checks: Vec::new(),
            hotness: HashMap::new(),
        }
    }

    pub fn add_check(&mut self, check: Box<dyn Check<'ctx> + 'ctx>) {
        self.checks.push(check);
    }

    pub fn from(other: &Execution<'ctx>) -> Self {
        let mut new_execution = other.clone();
        new_execution.id = EXECUTION_COUNTER.fetch_add(1, Ordering::SeqCst);
        new_execution
    }

    pub fn get_solver(&self, context: &'ctx Context) -> z3::Solver<'ctx> {
        let solver = z3::Solver::new(&context.context);
        for constraint in self.constraints.iter() {
            solver.assert(constraint);
        }
        solver
    }

    pub fn solve(&self, context: &'ctx Context) -> Option<z3::Model<'ctx>> {
        let solver = self.get_solver(context);
        if solver.check() == z3::SatResult::Unsat {
            None
        } else {
            solver.get_model()
        }
    }
}

impl<'ctx> std::fmt::Display for Execution<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "#{}: state={}; constraints={:?}",
            self.id, self.state, self.constraints
        )
    }
}
