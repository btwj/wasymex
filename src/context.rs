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
}
