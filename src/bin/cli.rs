use clap::Parser;
use log::info;
use wasymex::{
    checks::{DivisionByZeroCheck, MemoryCheck},
    engine::Engine,
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    quiet: bool,

    #[arg(long)]
    max_hotness: Option<usize>,

    #[arg(short, long)]
    main: Option<String>,
}

fn analyze_module(engine: &mut Engine) {
    for func in engine.context.module.funcs.iter() {
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
                engine.analyze_func(local_func, func.id(), &name);
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let log_colors = fern::colors::ColoredLevelConfig::new()
        .info(fern::colors::Color::White)
        .debug(fern::colors::Color::White)
        .trace(fern::colors::Color::BrightBlack)
        .error(fern::colors::Color::Red)
        .warn(fern::colors::Color::Yellow);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{target}{color_line}] {message}",
                color_line = format_args!(
                    "\x1B[{}m",
                    log_colors.get_color(&record.level()).to_fg_str()
                ),
                target = record.target(),
                message = message
            ))
        })
        .level(log::LevelFilter::Error)
        .level_for(
            "wasymex",
            if args.quiet {
                log::LevelFilter::Error
            } else {
                log::LevelFilter::Trace
            },
        )
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    let wasm_module = walrus::Module::from_file(args.input).unwrap();

    let context = wasymex::context::Context::new(&wasm_module);
    let mut engine = wasymex::engine::Engine::new(&context);
    engine.initialize();

    if let Some(max_loop_iters) = args.max_hotness {
        engine.set_max_hotness(max_loop_iters);
    }

    engine.add_check(Box::new(DivisionByZeroCheck::new()));
    engine.add_check(Box::new(MemoryCheck::new()));

    match args.main {
        None => analyze_module(&mut engine),
        Some(main) => {
            let func_id = match context.module.funcs.by_name(&main) {
                Some(id) => id,
                None => {
                    let funcs = context.module.funcs.iter().collect::<Vec<_>>();
                    funcs[main.parse::<usize>().unwrap()].id()
                }
            };
            let func = context.module.funcs.get(func_id);
            let local_func = wasymex::engine::as_local_func(func).unwrap();
            engine.analyze_func(local_func, func_id, &main)
        }
    }
}
