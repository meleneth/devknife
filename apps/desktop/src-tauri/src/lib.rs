use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use devknife_core::{
    load_environment_yaml, load_workflow_yaml, plan_workflow, ExecutionLimits, ExecutionPolicy,
    LoadError, RunPlan, RunReport, Runner, RuntimeEnvironment,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentSummary {
    name: String,
    path: String,
    service_count: usize,
    value_count: usize,
    secret_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunSummary {
    run_id: String,
    workflow_name: String,
    status: devknife_core::RunStatus,
    trace_entry_count: usize,
    modified_at_unix_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowValidation {
    valid: bool,
    kind: Option<&'static str>,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
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
fn list_environments() -> Result<Vec<EnvironmentSummary>, String> {
    let root = repo_root()?;
    let environment_dir = root.join("examples/environments");
    if !environment_dir.exists() {
        return Ok(Vec::new());
    }

    let mut environments = Vec::new();
    for entry in fs::read_dir(&environment_dir)
        .map_err(|error| format!("failed to read {}: {error}", environment_dir.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
            continue;
        }

        let environment = read_environment_file(&path)?;
        environments.push(EnvironmentSummary {
            name: environment.name.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("environment")
                    .to_string()
            }),
            path: path_to_ui(&root, &path),
            service_count: environment.services.len(),
            value_count: environment.values.len(),
            secret_count: environment.secret_refs.len(),
        });
    }

    environments.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(environments)
}

#[tauri::command]
fn plan_workflow_file(path: String) -> Result<RunPlan, String> {
    let root = repo_root()?;
    let workflow_path = resolve_workflow_path(&root, &path)?;
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
fn validate_workflow_source(source: String) -> WorkflowValidation {
    match load_workflow_yaml(&source) {
        Ok(_) => WorkflowValidation {
            valid: true,
            kind: None,
            message: "Workflow is valid.".to_string(),
            line: None,
            column: None,
        },
        Err(LoadError::Parse(error)) => {
            let location = error.location();
            WorkflowValidation {
                valid: false,
                kind: Some("syntax"),
                message: error.to_string(),
                line: location.map(|value| value.line()),
                column: location.map(|value| value.column()),
            }
        }
        Err(LoadError::Validation(message)) => WorkflowValidation {
            valid: false,
            kind: Some("semantic"),
            message,
            line: None,
            column: None,
        },
    }
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

    replace_file_safely(&workflow_path, source.as_bytes())
}

#[tauri::command]
fn run_workflow_file(
    path: String,
    environment_path: Option<String>,
    allowed_capabilities: Vec<String>,
) -> Result<RunReport, String> {
    let root = repo_root()?;
    let workflow_path = resolve_workflow_path(&root, &path)?;
    let workflow = read_workflow(&workflow_path)?;
    let environment = match environment_path {
        Some(path) => {
            let path = resolve_environment_path(&root, &path)?;
            read_environment_file(&path)?
        }
        None => RuntimeEnvironment::default(),
    };
    let report = Runner::with_environment_and_policy(
        ExecutionLimits::default(),
        environment,
        ExecutionPolicy::allow_capabilities(allowed_capabilities),
    )
    .run(workflow);

    write_run_report(&root, &report)?;
    Ok(report)
}

#[tauri::command]
fn list_run_reports() -> Result<Vec<RunSummary>, String> {
    let root = repo_root()?;
    let run_dir = root.join("runs");
    if !run_dir.exists() {
        return Ok(Vec::new());
    }

    let mut reports = fs::read_dir(&run_dir)
        .map_err(|error| {
            format!(
                "failed to read run directory {}: {error}",
                run_dir.display()
            )
        })?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.ends_with(".trace.json"))
            {
                return None;
            }

            let report: RunReport = serde_json::from_str(&fs::read_to_string(&path).ok()?).ok()?;
            let modified_at_unix_ms = entry
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis();
            Some(RunSummary {
                run_id: report.run_id,
                workflow_name: report.workflow_name,
                status: report.status,
                trace_entry_count: report.trace.len(),
                modified_at_unix_ms,
            })
        })
        .collect::<Vec<_>>();

    reports.sort_by_key(|report| std::cmp::Reverse(report.modified_at_unix_ms));
    reports.truncate(20);
    Ok(reports)
}

#[tauri::command]
fn read_run_report(run_id: String) -> Result<RunReport, String> {
    if run_id.is_empty()
        || !run_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("invalid run id".to_string());
    }

    let path = repo_root()?
        .join("runs")
        .join(format!("{run_id}.trace.json"));
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read run report {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse run report {}: {error}", path.display()))
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
            list_environments,
            plan_workflow_file,
            read_workflow_source,
            save_workflow_source,
            validate_workflow_source,
            run_workflow_file,
            list_run_reports,
            read_run_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn read_workflow(path: &Path) -> Result<devknife_core::Workflow, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read workflow: {error}"))?;
    load_workflow_yaml(&contents).map_err(|error| error.to_string())
}

fn read_environment_file(path: &Path) -> Result<RuntimeEnvironment, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read environment {}: {error}", path.display()))?;
    load_environment_yaml(&contents).map_err(|error| error.to_string())
}

fn write_run_report(root: &Path, report: &RunReport) -> Result<(), String> {
    let run_dir = root.join("runs");
    fs::create_dir_all(&run_dir).map_err(|error| {
        format!(
            "failed to create run directory {}: {error}",
            run_dir.display()
        )
    })?;
    let path = run_dir.join(format!("{}.trace.json", report.run_id));
    let contents = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize run report: {error}"))?;
    fs::write(&path, contents)
        .map_err(|error| format!("failed to write run report {}: {error}", path.display()))
}

fn replace_file_safely(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("file {} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("file {} has no valid UTF-8 name", path.display()))?;

    let mut temporary = None;
    for attempt in 0..100 {
        let candidate = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to create temporary workflow file {}: {error}",
                    candidate.display()
                ));
            }
        }
    }

    let (temporary_path, mut temporary_file) =
        temporary.ok_or_else(|| "failed to allocate a temporary workflow file".to_string())?;
    if let Err(error) = temporary_file
        .write_all(contents)
        .and_then(|_| temporary_file.sync_all())
    {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!(
            "failed to write temporary workflow file {}: {error}",
            temporary_path.display()
        ));
    }
    drop(temporary_file);

    replace_file_from_temporary(path, &temporary_path)
}

#[cfg(unix)]
fn replace_file_from_temporary(path: &Path, temporary_path: &Path) -> Result<(), String> {
    fs::rename(temporary_path, path).map_err(|error| {
        let _ = fs::remove_file(temporary_path);
        format!(
            "failed to replace workflow file {}: {error}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn replace_file_from_temporary(path: &Path, temporary_path: &Path) -> Result<(), String> {
    let backup_path = path.with_extension(format!("{}.bak", std::process::id()));
    fs::rename(path, &backup_path).map_err(|error| {
        let _ = fs::remove_file(temporary_path);
        format!(
            "failed to prepare workflow file {}: {error}",
            path.display()
        )
    })?;

    if let Err(error) = fs::rename(temporary_path, path) {
        let restore_result = fs::rename(&backup_path, path);
        let _ = fs::remove_file(temporary_path);
        return Err(match restore_result {
            Ok(()) => format!("failed to replace workflow file {}: {error}", path.display()),
            Err(restore_error) => format!(
                "failed to replace workflow file {}: {error}; backup remains at {} because restoration failed: {restore_error}",
                path.display(),
                backup_path.display()
            ),
        });
    }

    fs::remove_file(&backup_path).map_err(|error| {
        format!(
            "workflow was saved, but failed to remove backup {}: {error}",
            backup_path.display()
        )
    })
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

fn resolve_environment_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let candidate = resolve_repo_path(root, path)?;
    let environment_dir = root
        .join("examples/environments")
        .canonicalize()
        .map_err(|error| format!("failed to resolve environment directory: {error}"))?;

    if !candidate.starts_with(&environment_dir)
        || candidate.extension().and_then(|value| value.to_str()) != Some("yaml")
    {
        return Err("environment path must be a YAML file in examples/environments".to_string());
    }

    Ok(candidate)
}

fn path_to_ui(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::{replace_file_safely, validate_workflow_source};

    #[test]
    fn workflow_validation_reports_yaml_locations() {
        let result = validate_workflow_source("name: [".to_string());

        assert!(!result.valid);
        assert_eq!(result.kind, Some("syntax"));
        assert!(result.line.is_some());
        assert!(result.column.is_some());
    }

    #[test]
    fn workflow_validation_distinguishes_semantic_errors() {
        let result = validate_workflow_source("name: ''".to_string());

        assert!(!result.valid);
        assert_eq!(result.kind, Some("semantic"));
        assert!(result.message.contains("workflow name is required"));
        assert_eq!(result.line, None);
    }

    #[test]
    fn safe_file_replacement_preserves_complete_contents() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "devknife-safe-save-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create test directory");
        let path = directory.join("workflow.yaml");
        fs::write(&path, "name: before\n").expect("write original");

        replace_file_safely(&path, b"name: after\n").expect("replace file");

        assert_eq!(
            fs::read_to_string(&path).expect("read replacement"),
            "name: after\n"
        );
        assert_eq!(
            fs::read_dir(&directory)
                .expect("read test directory")
                .count(),
            1
        );
        fs::remove_dir_all(&directory).expect("remove test directory");
    }
}
