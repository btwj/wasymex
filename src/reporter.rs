use crate::checks::CheckResult;
use crate::context::Context;
use crate::state::{Execution, Status};
use crate::value::{SymVal, Val};
use colored::Colorize;
use std::collections::HashMap;
use walrus::ir;

pub struct Reporter {
    show_traces: bool,
}

impl Reporter {
    pub fn new() -> Self {
        Reporter { show_traces: false }
    }

    pub fn report_func(&self, name: &str) {
        println!("{}", name.to_string().bold().cyan())
    }

    pub fn report_executions(&self, _: &Context, executions: &Vec<Execution>) {
        println!(
            "  {}",
            format!("Collected {} Execution Paths", executions.len()).blue()
        );
        for execution in executions.iter() {
            if execution.status == Status::Complete {
                println!("    {}", execution.to_string().white());
            } else {
                println!(
                    "    ✗ {} {}",
                    (match execution.status {
                        Status::Terminated => "Terminated",
                        _ => todo!(),
                    })
                    .yellow(),
                    execution.to_string().bright_black()
                );
            }
        }
    }

    pub fn format_model_values<'ctx, T: z3::ast::Ast<'ctx> + std::fmt::Debug>(
        variables: &[T],
        model: &z3::Model<'ctx>,
    ) -> String {
        variables
            .iter()
            .map(|var| format!("{:?}={:?}", var, model.eval(var, true).unwrap()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn format_model(inputs: &HashMap<ir::LocalId, Val>, model: &z3::Model) -> String {
        inputs
            .iter()
            .map(|(local_id, input_value)| {
                format!(
                    "local{}={}",
                    local_id.index(),
                    match input_value {
                        Val::Conc(val) => format!("{}", val),
                        Val::Sym(val) => {
                            format!(
                                "{}",
                                match val {
                                    SymVal::I32(i32_val) => model.eval(i32_val, true).unwrap(),
                                }
                            )
                        }
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn report_checks<'ctx>(
        &self,
        context: &'ctx Context,
        inputs: &HashMap<ir::LocalId, Val<'ctx>>,
        executions: &mut Vec<Execution<'ctx>>,
    ) {
        println!("  {}", "Execution Path Checks".blue());
        for execution in executions {
            let model_input = execution.solve(context);
            match model_input {
                None => {
                    println!(
                        "{}",
                        format!("    #{}: Infeasible; skipping...", execution.id).bright_black()
                    );
                }
                Some(model) => {
                    println!(
                        "{}",
                        format!(
                            "    #{}: Feasible; Input=[{}]",
                            execution.id,
                            Self::format_model(inputs, &model)
                        )
                        .white()
                    );

                    let mut execution_checks = std::mem::take(&mut execution.checks);
                    for check in &mut execution_checks {
                        match check.run(context, execution, inputs) {
                            CheckResult::Ok => {
                                println!("        {}", format!("[{}] ✓", check.name()).green())
                            }
                            CheckResult::PossibleFail(message) => {
                                println!(
                                    "        {}",
                                    format!("[{}] ? {}", check.name(), message).yellow()
                                )
                            }
                            CheckResult::Fail(message) => {
                                println!(
                                    "        {}",
                                    format!("[{}] ✗ {}", check.name(), message).red()
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
