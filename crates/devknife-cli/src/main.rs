use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use devknife_core::{
    load_workflow_yaml, validate_workflow, Observation, RunReport, RunStatus, Runner,
    TraceEntryKind,
};

#[derive(Debug, Parser)]
#[command(name = "devknife")]
#[command(about = "Event-native service workflow runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        workflow: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Validate {
        workflow: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { workflow, json } => {
            let workflow = read_workflow(workflow)?;
            let report = Runner::default().run(workflow);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_human_report(&report);
            }
        }
        Command::Validate { workflow } => {
            let workflow = read_workflow(workflow)?;
            validate_workflow(&workflow)?;
            println!("valid workflow: {}", workflow.name);
        }
    }

    Ok(())
}

fn read_workflow(path: PathBuf) -> Result<devknife_core::Workflow> {
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read workflow file {}", path.display()))?;
    load_workflow_yaml(&contents)
        .with_context(|| format!("failed to load workflow file {}", path.display()))
}

fn print_human_report(report: &RunReport) {
    println!("Run: {}", report.run_id);
    println!("Workflow: {}", report.workflow_name);
    println!("Status: {}", status_label(&report.status));

    if let Some(failure) = &report.failure {
        println!("Failure: {}", failure.message);
    }

    println!();
    println!("Trace:");
    let mut display_index = 1usize;
    for entry in &report.trace {
        if let Some(line) = trace_line(&entry.kind) {
            println!("{display_index}. {line}");
            display_index += 1;
        }
    }
}

fn status_label(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Succeeded => "succeeded",
        RunStatus::Failed => "failed",
    }
}

fn trace_line(kind: &TraceEntryKind) -> Option<String> {
    match kind {
        TraceEntryKind::EventSeeded { event } => Some(format!("event {} seeded", event.event_type)),
        TraceEntryKind::EventDequeued { event } => Some(format!("event {}", event.event_type)),
        TraceEntryKind::HandlerMatched { on, .. } => Some(format!("handler on {on}")),
        TraceEntryKind::EffectExecuted {
            effect,
            observation,
            ..
        } => match observation {
            Observation::EmittedEvents { events } => {
                let event_types = events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!("effect {} {event_types}", effect.name()))
            }
            Observation::RecordedMessage { message } => {
                Some(format!("effect record \"{message}\""))
            }
            Observation::AssertionPassed { path } => Some(format!("effect assert {path} passed")),
            Observation::AssertionFailed { path, .. } => {
                Some(format!("effect assert {path} failed"))
            }
        },
        TraceEntryKind::HandlerSkipped { on, .. } => Some(format!("no handler for {on}")),
        TraceEntryKind::RunStarted { .. } | TraceEntryKind::RunEnded { .. } => None,
    }
}
