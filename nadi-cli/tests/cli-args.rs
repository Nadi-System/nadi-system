use assert_cmd::prelude::*;
use std::path::Path;
use std::process::Command;

type Error = Box<dyn std::error::Error>;

#[test]
/// Show help and exit.
fn help_shows() -> Result<(), Error> {
    Command::cargo_bin("nadi")?.arg("-h").assert().success();
    Command::cargo_bin("nadi")?.arg("--help").assert().success();

    Ok(())
}

// TODO do these for simple tasks; then write functions to easily save
// tasks in a file and check stdout. Maybe save the stdout in a file
// as well.
#[test]
/// Show help and exit.
fn run_tasks() -> Result<(), Error> {
    Command::cargo_bin("nadi")?
        .arg("-t")
        .arg("env.echo(\"hi\")")
        .assert()
        .stdout("hi\n");
    Command::cargo_bin("nadi")?
        .arg("-t")
        .arg("env {1 + 2 + 3}")
        .assert()
        .stdout("6\n");
    Ok(())
}

#[test]
/// Show help and exit.
fn run_task_files() -> Result<(), Error> {
    let par = Path::new("tests");
    let tasks = std::fs::read_dir(par.join("tasks")).unwrap();
    for task in tasks {
        let task = task.unwrap().path();
        let path = task.to_string_lossy();
        let out = task.with_extension("stdout");
        let stdout = std::fs::read_to_string(out).unwrap();
        Command::cargo_bin("nadi")?
            .arg(path.as_ref())
            .assert()
            .stdout(stdout);
    }
    Ok(())
}
