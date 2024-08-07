use std::{iter, path::PathBuf};

use anyhow::{anyhow, Context};

use crate::{
    job_type::JobType,
    jobs_runner::JobDefinition,
    spawner::{spawn, spawn_blocking, SpawnResult},
};

use super::{ProcessCommand, Target};

const TEST_SPECS: [&str; 14] = [
    // TODO:
    // Running "jobs" here causes the program to receive SIGTRAP from OS because of an
    // out-of-memory error in electron app, even if only this job was running (by
    // commenting out the other specs).
    //
    // This error happens while executing function `jobs.getShellProfiles()` in file `session.jobs.spec.ts`
    // which will call rust function `get_valid_profiles()` in `indexer/session/src/unbound/commands/shells.rs`
    // using the crate `envvars` which panics with piped shells.
    //
    // The current work-around is blocking the progress bars temporally and running the tests
    // sequentially using `Stdio::inherit` to keep using the main shell, printing the results
    // of the test directly to standard out, then the progress bars will be shown again.
    "jobs",
    "search",
    "values",
    "extract",
    "ranges",
    "exporting",
    "map",
    "observe",
    "indexes",
    "concat",
    "cancel",
    "errors",
    "stream",
    "promises",
];

pub async fn run_test(production: bool) -> Result<SpawnResult, anyhow::Error> {
    let job_def = JobDefinition::new(Target::Binding, JobType::Test { production });
    let mut final_result: Option<SpawnResult> = None;

    let cwd = Target::Wrapper.cwd();

    let build_spec_path = cwd.join("spec");
    //TODO: This check exists in rake implementation but it need to be improved.
    // The check should cover if the test themselves or the code under the tests has been changed.
    if !build_spec_path.join("build").exists() {
        // Finding tsc path on differnet platforms
        let mut test_runner_path = cwd.join("node_modules").join(".bin");
        if cfg!(windows) {
            let tsc_path = which::which_in("tsc", Some(&test_runner_path), &test_runner_path)
                .context("Error while resolving tsc bin path on Windows")?;
            test_runner_path = tsc_path;
        } else {
            test_runner_path.push("tsc");
        }

        let build_spec_cmd = ProcessCommand::new(
            test_runner_path.to_string_lossy().to_string(),
            vec![String::from("-p"), String::from("tsconfig.json")],
        );

        let spec_res = spawn(
            job_def,
            build_spec_cmd,
            Some(build_spec_path),
            iter::empty(),
            None,
        )
        .await?;

        final_result = Some(spec_res);
    }

    let mut electron_path: PathBuf = cwd.join("node_modules").join(".bin");
    if cfg!(windows) {
        electron_path = which::which_in("electron", Some(&electron_path), &electron_path)
            .context("Error while resolving electron bin path on Windows")?;
    } else {
        electron_path.push("electron");
    }
    // "electron"].iter().collect();
    let electron_path = electron_path.to_string_lossy();

    let jasmine_path: PathBuf = [".", "node_modules", "jasmine", "bin", "jasmine.js"]
        .iter()
        .collect();
    let jasmine_path = jasmine_path.to_string_lossy();

    let specs_dir_path: PathBuf = ["spec", "build", "spec"].iter().collect();

    for spec in TEST_SPECS {
        let spec_file_name = format!("session.{spec}.spec.js");
        let spec_file_path = specs_dir_path.join(spec_file_name);
        let command = ProcessCommand::new(
            electron_path.to_string(),
            vec![
                jasmine_path.to_string(),
                spec_file_path.to_string_lossy().to_string(),
            ],
        );
        let res = spawn_blocking(
            job_def,
            command,
            Some(cwd.clone()),
            vec![(String::from("ELECTRON_RUN_AS_NODE"), String::from("1"))],
        )
        .await?;

        match final_result.as_mut() {
            Some(acc) => acc.append(res),
            None => final_result = Some(res),
        };
    }

    final_result.ok_or_else(|| anyhow!("Wrapper doesn't have test specs"))
}
