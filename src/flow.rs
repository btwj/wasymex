use std::collections::HashMap;
use walrus::ir;

#[derive(Debug, Copy, Clone)]
pub struct Loc {
    pub block: ir::InstrSeqId,
    pub loc: u32,
}

#[derive(Debug)]
struct LocsVisitor<'a> {
    locs: &'a mut Locs,
    blocks: Vec<ir::InstrSeqId>,
    to_update_next: Vec<Vec<ir::InstrSeqId>>,
}

#[derive(Debug)]
pub struct Locs {
    locs: Vec<Loc>,
    seqs: Vec<ir::InstrSeqId>,
    pub ends: HashMap<ir::InstrSeqId, Loc>,
}

impl<'instr, 'a> ir::Visitor<'instr> for LocsVisitor<'a> {
    fn start_instr_seq(&mut self, instr_seq: &'instr ir::InstrSeq) {
        self.locs.seqs.push(instr_seq.id());
        self.to_update_next.push(Vec::new());
        self.blocks.push(instr_seq.id());
    }

    fn end_instr_seq(&mut self, instr_seq: &'instr ir::InstrSeq) {
        self.blocks.pop();
        let mut last_to_update = self.to_update_next.pop().unwrap();
        last_to_update.push(instr_seq.id());
        self.to_update_next.last_mut().unwrap().extend(last_to_update);
    }

    fn visit_instr(&mut self, instr: &'instr ir::Instr, instr_loc: &'instr ir::InstrLocId) {
        let loc = Loc {
            block: *self.blocks.last().unwrap(),
            loc: instr_loc.data(),
        };

        self.locs.locs.push(loc.clone());

        let to_update = self.to_update_next.last_mut().unwrap();
        while let Some(seq) = to_update.pop() {
            self.locs.ends.insert(seq, loc);
        }
    }
}

pub fn compute_locs(func: &walrus::LocalFunction) -> Locs {
    let mut locs = Locs { locs: Vec::new(), seqs: Vec::new(), ends: HashMap::new() };
    let mut locs_visitor = LocsVisitor {
        locs: &mut locs,
        blocks: vec![],
        to_update_next: vec![vec![]],
    };

    ir::dfs_in_order(&mut locs_visitor, func, func.entry_block());
    locs
}
