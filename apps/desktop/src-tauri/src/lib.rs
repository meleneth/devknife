use std::{
    fs,
    path::{Path, PathBuf},
};

use devknife_core::{
    load_environment_yaml, load_workflow_yaml, plan_workflow, ExecutionLimits, RunPlan, RunReport,
    Runner, RuntimeEnvironment,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowSummary {
    name: String,
    version: String,
    path: String,
    seed_event_count: usize,
    handler_count: usize,
    effect_count: usize,
    capability_count: usize,
}

#[tauri::command]
fn list_workflows() -> Result<Vec<WorkflowSummary>, String> {
    let root = repo_root()?;
    let workflow_dir = root.join("examples/workflows");
    let mut workflows = Vec::new();

    for entry in fs::read_dir(&workflow_dir)
        .map_err(|error| format!("failed to read {}: {error}", workflow_dir.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
            continue;
        }

        let workflow = read_workflow(&path)?;
        let plan = plan_workflow(&workflow);
        workflows.push(WorkflowSummary {
            name: workflow.name,
            version: workflow.version,
            path: path_to_ui(&root, &path),
            seed_event_count: workflow.seed_events.len(),
            handler_count: workflow.handlers.len(),
            effect_count: plan.effects.len(),
            capability_count: plan.required_capabilities.len(),
        });
    }

    workflows.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(workflows)
}

#[tauri::command]
fn plan_workflow_file(path: String) -> Result<RunPlan, String> {
    let root = repo_root()?;
    let workflow_path = resolve_repo_path(&root, &path)?;
    let workflow = read_workflow(&workflow_path)?;
    Ok(plan_workflow(&workflow))
}

#[tauri::command]
fn read_workflow_source(path: String) -> Result<String, String> {
    let root = repo_root()?;
    let workflow_path = resolve_workflow_path(&root, &path)?;
    fs::read_to_string(&workflow_path).map_err(|error| {
        format!(
            "failed to read workflow source {}: {error}",
            workflow_path.display()
        )
    })
}

#[tauri::command]
fn validate_workflow_source(source: String) -> Result<(), String> {
    load_workflow_yaml(&source)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_workflow_source(
    path: String,
    source: String,
    expected_source: String,
) -> Result<(), String> {
    load_workflow_yaml(&source).map_err(|error| error.to_string())?;

    let root = repo_root()?;
    let workflow_path = resolve_workflow_path(&root, &path)?;
    let current_source = fs::read_to_string(&workflow_path).map_err(|error| {
        format!(
            "failed to read workflow source {} before saving: {error}",
            workflow_path.display()
        )
    })?;

    if current_source != expected_source {
        return Err(
            "workflow changed on disk since it was loaded; reload before saving".to_string(),
        );
    }

    fs::write(&workflow_path, source).map_err(|error| {
        format!(
            "failed to save workflow source {}: {error}",
            workflow_path.display()
        )
    })
}

#[tauri::command]
fn run_workflow_file(path: String) -> Result<RunReport, String> {
    let root = repo_root()?;
    let workflow_path = resolve_repo_path(&root, &path)?;
    let workflow = read_workflow(&workflow_path)?;
    let environment = read_environment(&root)?;

    Ok(Runner::with_environment(ExecutionLimits::default(), environment).run(workflow))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_workflows,
            plan_workflow_file,
            read_workflow_source,
            save_workflow_source,
            validate_workflow_source,
            run_workflow_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn read_workflow(path: &Path) -> Result<devknife_core::Workflow, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read workflow: {error}"))?;
    load_workflow_yaml(&contents).map_err(|error| error.to_string())
}

fn read_environment(root: &Path) -> Result<RuntimeEnvironment, String> {
    let path = root.join("examples/environments/local.yaml");
    if !path.exists() {
        return Ok(RuntimeEnvironment::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read environment {}: {error}", path.display()))?;
    load_environment_yaml(&contents).map_err(|error| error.to_string())
}

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve repository root".to_string())
}

fn resolve_repo_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate
        .canonicalize()
        .map_err(|error| format!("failed to resolve path {}: {error}", candidate.display()))?;
    let root = root
        .canonicalize()
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;

    if !candidate.starts_with(&root) {
        return Err("workflow path must stay inside the repository".to_string());
    }

    Ok(candidate)
}

fn resolve_workflow_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let candidate = resolve_repo_path(root, path)?;
    let workflow_dir = root
        .join("examples/workflows")
        .canonicalize()
        .map_err(|error| format!("failed to resolve workflow directory: {error}"))?;

    if !candidate.starts_with(&workflow_dir)
        || candidate.extension().and_then(|value| value.to_str()) != Some("yaml")
    {
        return Err("workflow source path must be a YAML file in examples/workflows".to_string());
    }

    Ok(candidate)
}

fn path_to_ui(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
