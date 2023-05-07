use crate::state::TrapReason;
use crate::value::{ConcVal, SymVal, Val};
use walrus::ir;
use z3;

#[derive(Debug)]
pub struct Context<'m> {
    pub context: z3::Context,
    pub module: &'m walrus::Module,
}

impl<'ctx, 'm> Context<'m> {
    pub fn new(module: &'m walrus::Module) -> Self {
        let config = z3::Config::new();
        let context = z3::Context::new(&config);

        Context { context, module }
    }

    pub fn zero(&'ctx self, size: u32) -> z3::ast::BV<'ctx> {
        z3::ast::BV::from_i64(&self.context, 0, size)
    }
    pub fn one(&'ctx self, size: u32) -> z3::ast::BV<'ctx> {
        z3::ast::BV::from_i64(&self.context, 1, size)
    }

    pub fn bin_conc(
        &'ctx self,
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
        &'ctx self,
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
        &'ctx self,
        op: ir::BinaryOp,
        lhs: &Val<'ctx>,
        rhs: &Val<'ctx>,
    ) -> Result<Val<'ctx>, TrapReason> {
        Ok(match (lhs, rhs) {
            (Val::Conc(lhs_val), Val::Conc(rhs_val)) => {
                Val::Conc(self.bin_conc(op, lhs_val, rhs_val)?)
            }
            (Val::Sym(lhs_val), Val::Conc(rhs_val)) => {
                let rhs_val: SymVal = SymVal::from_concrete(&self.context, rhs_val);
                Val::Sym(self.bin_sym(op, lhs_val, &rhs_val))
            }
            (Val::Conc(lhs_val), Val::Sym(rhs_val)) => {
                let lhs_val: SymVal = SymVal::from_concrete(&self.context, lhs_val);
                Val::Sym(self.bin_sym(op, &lhs_val, rhs_val))
            }
            (Val::Sym(lhs_val), Val::Sym(rhs_val)) => Val::Sym(self.bin_sym(op, lhs_val, rhs_val)),
        })
    }
}
