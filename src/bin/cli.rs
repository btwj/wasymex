use clap::Parser;
use wasymex::checks::{DivisionByZeroCheck, MemoryCheck};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    quiet: bool,

    #[arg(short, long)]
    max_loop_iters: Option<usize>,
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

    if let Some(max_loop_iters) = args.max_loop_iters {
        engine.set_max_loop_iters(max_loop_iters);
    }

    engine.add_check(Box::new(DivisionByZeroCheck::new()));
    engine.add_check(Box::new(MemoryCheck::new()));
    engine.analyze_module();
}
