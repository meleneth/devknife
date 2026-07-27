use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use devknife_core::{
    load_environment_yaml, load_workflow_yaml, plan_workflow, validate_workflow_environment,
    ExecutionLimits, ExecutionPolicy, LoadError, RunPlan, RunReport, Runner, RuntimeEnvironment,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowSummary {
    name: String,
    version: String,
    path: String,
    valid: bool,
    validation_error: Option<String>,
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
    valid: bool,
    validation_error: Option<String>,
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
        if !is_yaml_path(&path) {
            continue;
        }

        workflows.push(summarize_workflow(&root, &workflow_dir, &path));
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
        if !is_yaml_path(&path) {
            continue;
        }

        environments.push(summarize_environment(&root, &environment_dir, &path));
    }

    environments.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(environments)
}

#[tauri::command]
fn plan_workflow_file(path: String, environment_path: Option<String>) -> Result<RunPlan, String> {
    let root = repo_root()?;
    let workflow_path = resolve_workflow_path(&root, &path)?;
    let workflow = read_workflow(&workflow_path)?;
    if let Some(path) = environment_path {
        let environment_path = resolve_environment_path(&root, &path)?;
        let environment = read_environment_file(&environment_path)?;
        validate_workflow_environment(&workflow, &environment)
            .map_err(|error| error.to_string())?;
    }
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
    validate_workflow_environment(&workflow, &environment).map_err(|error| error.to_string())?;
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

fn summarize_workflow(root: &Path, workflow_dir: &Path, path: &Path) -> WorkflowSummary {
    let ui_path = path_to_ui(root, path);
    let confined_path = match confine_discovered_path(workflow_dir, path) {
        Ok(path) => path,
        Err(error) => {
            return WorkflowSummary {
                name: artifact_name(path, "workflow"),
                version: "invalid".to_string(),
                path: ui_path,
                valid: false,
                validation_error: Some(error),
                seed_event_count: 0,
                handler_count: 0,
                effect_count: 0,
                capability_count: 0,
            };
        }
    };
    match read_workflow(&confined_path) {
        Ok(workflow) => {
            let plan = plan_workflow(&workflow);
            WorkflowSummary {
                name: workflow.name,
                version: workflow.version,
                path: ui_path,
                valid: true,
                validation_error: None,
                seed_event_count: workflow.seed_events.len(),
                handler_count: workflow.handlers.len(),
                effect_count: plan.effects.len(),
                capability_count: plan.required_capabilities.len(),
            }
        }
        Err(error) => WorkflowSummary {
            name: artifact_name(path, "workflow"),
            version: "invalid".to_string(),
            path: ui_path,
            valid: false,
            validation_error: Some(error),
            seed_event_count: 0,
            handler_count: 0,
            effect_count: 0,
            capability_count: 0,
        },
    }
}

fn summarize_environment(root: &Path, environment_dir: &Path, path: &Path) -> EnvironmentSummary {
    let ui_path = path_to_ui(root, path);
    let confined_path = match confine_discovered_path(environment_dir, path) {
        Ok(path) => path,
        Err(error) => {
            return EnvironmentSummary {
                name: artifact_name(path, "environment"),
                path: ui_path,
                valid: false,
                validation_error: Some(error),
                service_count: 0,
                value_count: 0,
                secret_count: 0,
            };
        }
    };
    match read_environment_file(&confined_path) {
        Ok(environment) => EnvironmentSummary {
            name: environment
                .name
                .unwrap_or_else(|| artifact_name(path, "environment")),
            path: ui_path,
            valid: true,
            validation_error: None,
            service_count: environment.services.len(),
            value_count: environment.values.len(),
            secret_count: environment.secret_refs.len(),
        },
        Err(error) => EnvironmentSummary {
            name: artifact_name(path, "environment"),
            path: ui_path,
            valid: false,
            validation_error: Some(error),
            service_count: 0,
            value_count: 0,
            secret_count: 0,
        },
    }
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

    if !candidate.starts_with(&workflow_dir) || !is_yaml_path(&candidate) {
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

    if !candidate.starts_with(&environment_dir) || !is_yaml_path(&candidate) {
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

fn is_yaml_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("yaml" | "yml")
    )
}

fn confine_discovered_path(directory: &Path, path: &Path) -> Result<PathBuf, String> {
    let directory = directory
        .canonicalize()
        .map_err(|error| format!("failed to resolve artifact directory: {error}"))?;
    let candidate = path
        .canonicalize()
        .map_err(|error| format!("failed to resolve artifact {}: {error}", path.display()))?;
    if !candidate.starts_with(&directory) {
        return Err(format!(
            "artifact {} resolves outside {}",
            path.display(),
            directory.display()
        ));
    }
    Ok(candidate)
}

fn artifact_name(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback)
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::{
        confine_discovered_path, replace_file_safely, resolve_environment_path,
        resolve_workflow_path, summarize_environment, summarize_workflow, validate_workflow_source,
    };

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
    fn invalid_artifacts_produce_error_summaries() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "devknife-invalid-artifacts-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create test directory");
        let workflow_path = directory.join("broken.workflow.yaml");
        let environment_path = directory.join("broken.environment.yaml");
        fs::write(&workflow_path, "name: [").expect("write invalid workflow");
        fs::write(&environment_path, "services: [").expect("write invalid environment");

        let workflow = summarize_workflow(&directory, &directory, &workflow_path);
        let environment = summarize_environment(&directory, &directory, &environment_path);

        assert!(!workflow.valid);
        assert_eq!(workflow.name, "broken.workflow");
        assert!(workflow.validation_error.is_some());
        assert!(!environment.valid);
        assert_eq!(environment.name, "broken.environment");
        assert!(environment.validation_error.is_some());

        fs::remove_dir_all(&directory).expect("remove test directory");
    }

    #[test]
    fn repository_path_guards_accept_yaml_extensions() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "devknife-yaml-paths-{}-{unique}",
            std::process::id()
        ));
        let workflow_dir = root.join("examples/workflows");
        let environment_dir = root.join("examples/environments");
        fs::create_dir_all(&workflow_dir).expect("create workflow directory");
        fs::create_dir_all(&environment_dir).expect("create environment directory");
        let workflow = workflow_dir.join("workflow.yml");
        let environment = environment_dir.join("environment.yaml");
        let rejected = workflow_dir.join("workflow.json");
        fs::write(&workflow, "name: workflow\n").expect("write workflow");
        fs::write(&environment, "name: environment\n").expect("write environment");
        fs::write(&rejected, "{}").expect("write rejected artifact");

        assert_eq!(
            resolve_workflow_path(&root, "examples/workflows/workflow.yml")
                .expect("resolve yml workflow"),
            workflow
        );
        assert_eq!(
            resolve_environment_path(&root, "examples/environments/environment.yaml")
                .expect("resolve yaml environment"),
            environment
        );
        assert!(resolve_workflow_path(&root, "examples/workflows/workflow.json").is_err());

        fs::remove_dir_all(&root).expect("remove test directory");
    }

    #[test]
    fn discovery_rejects_artifacts_outside_its_directory() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "devknife-confined-artifacts-{}-{unique}",
            std::process::id()
        ));
        let allowed = root.join("allowed");
        fs::create_dir_all(&allowed).expect("create allowed directory");
        let inside = allowed.join("inside.yaml");
        let outside = root.join("outside.yaml");
        fs::write(&inside, "name: inside\n").expect("write inside artifact");
        fs::write(&outside, "name: outside\n").expect("write outside artifact");

        assert_eq!(
            confine_discovered_path(&allowed, &inside).expect("inside path is accepted"),
            inside
        );
        assert!(confine_discovered_path(&allowed, &outside).is_err());

        fs::remove_dir_all(&root).expect("remove test directory");
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
