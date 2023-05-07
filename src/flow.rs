use std::collections::{HashMap, HashSet};
use walrus::ir;

#[derive(Debug, Copy, Clone)]
pub struct Loc {
    pub block: ir::InstrSeqId,
    pub loc: u32,
}

#[derive(Debug)]
struct InfoVisitor<'a> {
    info: &'a mut Info,
    blocks: Vec<ir::InstrSeqId>,
    to_update_next: Vec<Vec<ir::InstrSeqId>>,
}

#[derive(Debug, Default)]
pub struct Info {
    locs: Vec<Loc>,
    seqs: Vec<ir::InstrSeqId>,
    pub types: HashMap<ir::InstrSeqId, ir::Instr>,
    pub ends: HashMap<ir::InstrSeqId, Loc>,
    pub locals: HashSet<ir::LocalId>,
}

impl<'instr, 'a> ir::Visitor<'instr> for InfoVisitor<'a> {
    fn start_instr_seq(&mut self, instr_seq: &'instr ir::InstrSeq) {
        self.info.seqs.push(instr_seq.id());
        self.to_update_next.push(Vec::new());
        self.blocks.push(instr_seq.id());
    }

    fn end_instr_seq(&mut self, instr_seq: &'instr ir::InstrSeq) {
        self.blocks.pop();
        let mut last_to_update = self.to_update_next.pop().unwrap();
        last_to_update.push(instr_seq.id());
        self.to_update_next
            .last_mut()
            .unwrap()
            .extend(last_to_update);
    }

    fn visit_instr(&mut self, instr: &'instr ir::Instr, instr_loc: &'instr ir::InstrLocId) {
        let loc = Loc {
            block: *self.blocks.last().unwrap(),
            loc: instr_loc.data(),
        };

        self.info.locs.push(loc.clone());

        let to_update = self.to_update_next.last_mut().unwrap();
        while let Some(seq) = to_update.pop() {
            self.info.ends.insert(seq, loc);
        }
    }

    fn visit_block(&mut self, imm: &ir::Block) {
        self.info
            .types
            .insert(imm.seq, ir::Instr::Block(imm.clone()));
    }

    fn visit_loop(&mut self, imm: &ir::Loop) {
        self.info
            .types
            .insert(imm.seq, ir::Instr::Loop(imm.clone()));
    }

    fn visit_local_id(&mut self, id: &ir::LocalId) {
        self.info.locals.insert(*id);
    }
}

pub fn compute_info(func: &walrus::LocalFunction) -> Info {
    let mut info = Info {
        locs: Vec::new(),
        seqs: Vec::new(),
        ends: HashMap::new(),
        types: HashMap::new(),
        locals: HashSet::new(),
    };
    let mut info_visitor = InfoVisitor {
        info: &mut info,
        blocks: vec![],
        to_update_next: vec![vec![]],
    };

    ir::dfs_in_order(&mut info_visitor, func, func.entry_block());
    info
}
