/// Solves the test/assemblyscript/checksum.ts checksum problem
/// Input is given to memory (checksum is 2495677951)
use clap::Parser;
use log::info;
use wasymex::engine::Engine;
use z3::{self, ast::Ast};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    quiet: bool,

    #[arg(short, long)]
    main: String,

    #[arg(short, long, allow_hyphen_values(true))]
    checksum: i32,
}

fn main() {
    let args = Args::parse();
    let wasm_module = walrus::Module::from_file(args.input).unwrap();

    let context = wasymex::context::Context::new(&wasm_module);
    let mut engine = wasymex::engine::Engine::new(&context);
    engine.initialize();

    let func_id = match context.module.funcs.by_name(&args.main) {
        Some(id) => id,
        None => {
            let funcs = context.module.funcs.iter().collect::<Vec<_>>();
            funcs[args.main.parse::<usize>().unwrap()].id()
        }
    };
    let func = context.module.funcs.get(func_id);
    let local_func = wasymex::engine::as_local_func(func).unwrap();

    let mut initial = engine.get_initial_execution(local_func, func_id);
    let memory = initial.state.memory.as_mut().unwrap();

    let password_len = 5;
    engine.set_max_hotness(password_len + 2);
    let mut password_values = Vec::new();

    for i in 0..password_len {
        let symbolic_char = z3::ast::BV::new_const(&context.context, format!("pwd{}", i), 8);
        memory.array = memory.array.store(
            &z3::ast::BV::from_u64(&context.context, i as u64, 32),
            &symbolic_char,
        );
        password_values.push(symbolic_char);
    }
    // null terminator
    memory.array = memory.array.store(
        &z3::ast::BV::from_u64(&context.context, password_len as u64, 32),
        &z3::ast::BV::from_u64(&context.context, 0 as u64, 8),
    );

    let executions = engine.get_func_executions(local_func, func_id, Some(initial));
    for execution in executions {
        if execution.status == wasymex::state::Status::Complete {
            let frame = execution.state.call_stack.last().unwrap();
            let return_value = frame.value_stack.last().unwrap();

            let solver = execution.get_solver(&context);
            solver.assert(&return_value.as_sym(&context.context).as_i32()._eq(
                &z3::ast::BV::from_u64(&context.context, args.checksum as u64, 32),
            ));

            for val in password_values.iter() {
                solver.assert(&val.bvsge(&z3::ast::BV::from_u64(&context.context, 'a' as u64, 8)));
                solver.assert(&val.bvsle(&z3::ast::BV::from_u64(&context.context, 'z' as u64, 8)));
            }

            match solver.check() {
                z3::SatResult::Sat => {
                    let model = solver.get_model().unwrap();
                    for value in password_values.iter() {
                        let ascii_val: u8 =
                            model.eval(value, true).unwrap().as_i64().unwrap() as u8;
                        print!("{}", ascii_val as char);
                    }
                    println!();
                }
                _ => (),
            }
        }
    }
}
