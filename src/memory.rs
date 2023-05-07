use crate::{
    engine::Engine,
    value::{ConcVal, SymVal, Val},
};
use walrus::ir;
use z3;

pub const PAGE_SIZE: u32 = 65536;

#[derive(Debug, Clone)]
pub struct Memory<'ctx> {
    pub size: Val<'ctx>, // size in pages
    pub array: z3::ast::Array<'ctx>,
}

impl<'ctx> Memory<'ctx> {
    pub fn new(context: &'ctx z3::Context, initial: u32) -> Self {
        Memory {
            size: Val::Conc(ConcVal(ir::Value::I32(initial as i32))),
            array: z3::ast::Array::const_array(
                context,
                &z3::Sort::bitvector(context, 32),
                &z3::ast::BV::from_i64(context, 0, 8),
            ),
        }
    }
}

impl<'ctx, 'm> Engine<'ctx, 'm> {
    pub fn do_load(
        &self,
        memory: &Memory<'ctx>,
        base_index: &Val<'ctx>,
        load_size: u32,
        size: u32,
        zero_extend: bool,
    ) -> Val<'ctx> {
        let num_bytes = (load_size / 8) as usize;
        let mut bytes = vec![];
        for i in 0..num_bytes {
            let index_val = self
                .bin_op(
                    ir::BinaryOp::I32Add,
                    &base_index,
                    &Val::Conc(ConcVal(ir::Value::I32(i as i32))),
                )
                .unwrap()
                .as_sym(&self.context.context);
            let index = index_val.as_i32();
            bytes.push(memory.array.select(index).as_bv().unwrap());
        }

        let mut value = bytes[num_bytes - 1].clone();
        for i in 0..(num_bytes - 1) {
            value = bytes[num_bytes - i - 2].concat(&value);
        }

        if size == load_size {
            Val::Sym(SymVal::I32(value))
        } else {
            if zero_extend {
                Val::Sym(SymVal::I32(value.zero_ext(size - load_size)))
            } else {
                Val::Sym(SymVal::I32(value.sign_ext(size - load_size)))
            }
        }
    }

    pub fn do_store(
        &mut self,
        memory: &mut Memory<'ctx>,
        base_index: &Val<'ctx>,
        value: Val<'ctx>,
        store_size: u32,
    ) {
        let num_bytes = (store_size / 8) as usize;
        let sym_val = value.as_sym(&self.context.context);
        let value = sym_val.as_i32();

        for i in 0..num_bytes {
            let index_val = self
                .bin_op(
                    ir::BinaryOp::I32Add,
                    &base_index,
                    &Val::Conc(ConcVal(ir::Value::I32(i as i32))),
                )
                .unwrap()
                .as_sym(&self.context.context);
            let index = index_val.as_i32();
            let stored = value.extract((i * 8 + 7) as u32, (i * 8) as u32);
            memory.array = memory.array.store(index, &stored);
        }
    }
}
