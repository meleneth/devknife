use std::{fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use devknife_core::{
    load_environment_yaml, load_workflow_yaml, validate_workflow, ExecutionLimits,
    GraphqlAssertionObservation, Observation, RestAssertionObservation, RunReport, RunStatus,
    Runner, TraceEntryKind,
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
        #[arg(long, value_name = "PATH")]
        environment: Option<PathBuf>,
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
        Command::Run {
            workflow,
            environment,
            json,
        } => {
            let workflow = read_workflow(workflow)?;
            let environment = read_environment(environment)?;
            let report =
                Runner::with_environment(ExecutionLimits::default(), environment).run(workflow);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_human_report(&report);
            }
            if report.status == RunStatus::Failed {
                bail!("workflow run failed");
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

fn read_environment(path: Option<PathBuf>) -> Result<devknife_core::RuntimeEnvironment> {
    let path = path.unwrap_or_else(|| PathBuf::from("examples/environments/local.yaml"));
    if !path.exists() {
        return Ok(devknife_core::RuntimeEnvironment::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read environment file {}", path.display()))?;
    load_environment_yaml(&contents)
        .with_context(|| format!("failed to load environment file {}", path.display()))
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
            Observation::RestResponse {
                operation,
                response,
                assertions,
                emitted_events,
            } => {
                let status_assertion = assertions.iter().next().map(|assertion| match assertion {
                    RestAssertionObservation::StatusPassed { expected, .. } => {
                        format!("status {expected} passed")
                    }
                    RestAssertionObservation::StatusFailed { expected, actual } => {
                        format!("status expected {expected} failed with {actual}")
                    }
                });
                let emitted = emitted_events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!(
                    "effect {} {} {} -> {}{}{}",
                    effect.name(),
                    operation.method,
                    operation.url,
                    response.status,
                    status_assertion
                        .map(|assertion| format!(" ({assertion})"))
                        .unwrap_or_default(),
                    if emitted.is_empty() {
                        String::new()
                    } else {
                        format!(" emitted {emitted}")
                    }
                ))
            }
            Observation::RestFailed {
                operation, message, ..
            } => Some(format!(
                "effect {} {} {} failed: {}",
                effect.name(),
                operation.method,
                operation.url,
                message
            )),
            Observation::GraphqlResponse {
                operation,
                response,
                assertions,
                emitted_events,
            } => {
                let graphql_assertion = assertions.iter().next().map(|assertion| match assertion {
                    GraphqlAssertionObservation::StatusPassed { expected, .. } => {
                        format!("status {expected} passed")
                    }
                    GraphqlAssertionObservation::StatusFailed { expected, actual } => {
                        format!("status expected {expected} failed with {actual}")
                    }
                    GraphqlAssertionObservation::NoErrorsPassed => "no GraphQL errors".to_string(),
                    GraphqlAssertionObservation::NoErrorsFailed { errors } => {
                        format!("{} GraphQL error(s)", errors.len())
                    }
                });
                let emitted = emitted_events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!(
                    "effect {} {} {} -> {}{}{}",
                    effect.name(),
                    operation.operation_name.as_deref().unwrap_or("anonymous"),
                    operation.url,
                    response.status,
                    graphql_assertion
                        .map(|assertion| format!(" ({assertion})"))
                        .unwrap_or_default(),
                    if emitted.is_empty() {
                        String::new()
                    } else {
                        format!(" emitted {emitted}")
                    }
                ))
            }
            Observation::GraphqlFailed {
                operation, message, ..
            } => Some(format!(
                "effect {} {} failed: {}",
                effect.name(),
                operation.url,
                message
            )),
            Observation::SnsPublish {
                operation,
                message_id,
                emitted_events,
            } => Some(format!(
                "effect {} {} -> {}{}",
                effect.name(),
                operation.url,
                message_id,
                emitted_suffix(emitted_events)
            )),
            Observation::SqsSend {
                operation,
                message_id,
                emitted_events,
            } => Some(format!(
                "effect {} {} -> {}{}",
                effect.name(),
                operation.url,
                message_id,
                emitted_suffix(emitted_events)
            )),
            Observation::SqsReceive {
                operation,
                messages,
                deleted_receipt_handles,
                emitted_events,
            } => Some(format!(
                "effect {} {} -> {} message(s), {} deleted{}",
                effect.name(),
                operation.url,
                messages.len(),
                deleted_receipt_handles.len(),
                emitted_suffix(emitted_events)
            )),
            Observation::AwsFailed { operation, message } => Some(format!(
                "effect {} {} {} failed: {}",
                effect.name(),
                operation.action,
                operation.url,
                message
            )),
        },
        TraceEntryKind::HandlerSkipped { on, .. } => Some(format!("no handler for {on}")),
        TraceEntryKind::RunStarted { .. } | TraceEntryKind::RunEnded { .. } => None,
    }
}

fn emitted_suffix(events: &[devknife_core::Event]) -> String {
    let emitted = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if emitted.is_empty() {
        String::new()
    } else {
        format!(" emitted {emitted}")
    }
}
