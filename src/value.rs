use walrus::ir;

#[derive(Debug, Clone)]
pub enum Val<'ctx> {
    Sym(SymVal<'ctx>),
    Conc(ConcVal),
}

impl<'ctx> std::fmt::Display for Val<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Val::Sym(val) => write!(f, "{val}"),
            Val::Conc(val) => write!(f, "{val}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConcVal(pub ir::Value);

impl std::fmt::Display for ConcVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ir::Value::I32(value) => write!(f, "{value}: i32"),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SymVal<'ctx> {
    I32(z3::ast::BV<'ctx>),
}

impl<'ctx> std::fmt::Display for SymVal<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymVal::I32(value) => write!(f, "{:?}: i32", value),
        }
    }
}

impl<'ctx> SymVal<'ctx> {
    pub fn from_concrete(context: &'ctx z3::Context, value: &ConcVal) -> SymVal<'ctx> {
        match value.0 {
            ir::Value::I32(value) => SymVal::I32(z3::ast::BV::from_i64(context, value as i64, 32)),
            _ => unreachable!(),
        }
    }

    pub fn from_valtype(
        context: &'ctx z3::Context,
        val_type: walrus::ValType,
        name: String,
    ) -> SymVal<'ctx> {
        match val_type {
            walrus::ValType::I32 => SymVal::I32(z3::ast::BV::new_const(&context, name, 32)),
            _ => todo!(),
        }
    }

    pub fn as_i32(&self) -> &z3::ast::BV<'ctx> {
        match self {
            SymVal::I32(z3_val) => z3_val,
        }
    }
}

impl ConcVal {
    pub fn as_i32(&self) -> i32 {
        if let ir::Value::I32(value) = self.0 {
            value
        } else {
            panic!("not i32")
        }
    }
}
