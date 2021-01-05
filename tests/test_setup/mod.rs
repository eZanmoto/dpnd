// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::panic;
use std::panic::UnwindSafe;
use std::process::Command;
use std::process::Stdio;

extern crate assert_cmd;

use self::assert_cmd::Command as AssertCommand;

// `create` does the following, in order:
//
// 1. Creates test directories,
// 2. Creates bare Git repositories with the specified commit history for each
//    dependency in `deps`,
// 3. Stores the commit hashes for each of the repositories created in the
//    previous step, and
// 4. Writes a new dependency file with the commits specified in
//    `deps_commit_nums`.
pub fn create(
    root_test_dir_name: &str,
    deps: &HashMap<&str, Vec<HashMap<&str, &str>>>,
    deps_commit_nums: &HashMap<&str, usize>,
)
    -> Layout
{
    let root_dir = create_root_dir(root_test_dir_name);
    let dep_srcs_dir = create_dir(root_dir.clone(), "deps");
    let scratch_dir = create_dir(root_dir.clone(), "scratch");
    let proj_dir = create_dir(root_dir, "proj");

    create_dep_srcs(&dep_srcs_dir, &scratch_dir, &deps);

    let mut deps_commit_hashes = hashmap!{};
    for dep_src_name in deps.keys() {
        deps_commit_hashes.insert(
            (*dep_src_name).to_string(),
            get_repo_hashes(&format!("{}/{}.git", dep_srcs_dir, dep_src_name)),
        );
    }

    let deps_file_conts = write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &deps_commit_nums,
    );

    Layout{
        dep_srcs_dir,
        proj_dir,
        deps_commit_hashes,
        deps_file_conts,
    }
}

pub struct Layout {
    pub dep_srcs_dir: String,
    pub deps_commit_hashes: HashMap<String, Vec<String>>,
    pub proj_dir: String,
    pub deps_file_conts: String,
}

pub fn create_root_dir(name: &str) -> String {
    create_dir(env!("TEST_DIR").to_string(), name)
}

pub fn create_dir(dir: String, name: &str) -> String {
    let path = dir + "/" + name;

    fs::create_dir(&path)
        .expect(&format!("couldn't create directory: {}", path));

    path
}

fn create_dep_srcs(
    dep_srcs_dir: &str,
    scratch_dir: &str,
    deps: &HashMap<&str, Vec<HashMap<&str, &str>>>,
) {
    for (dep_src_name, commits) in deps {
        let dep_src_dir_name = format!("{}.git", dep_src_name);
        create_bare_git_repo(
            &create_dir(dep_srcs_dir.to_owned(), &dep_src_dir_name),
            &create_dir(scratch_dir.to_string(), dep_src_name),
            &commits,
        );
    }
}

// `scratch_dir` is expected to be an empty directory that
// `create_bare_git_repo` can use for its own purposes.
pub fn create_bare_git_repo(
    repo_dir: &str,
    scratch_dir: &str,
    fs_states: &[HashMap<&str, &str>],
) {
    let gits_args = &[
        vec!["init"],
        vec!["config", "user.name", "Test"],
        vec!["config", "user.email", "test@example.com"],
    ];
    for git_args in gits_args {
        run_cmd(scratch_dir, "git", git_args);
    }

    for fs_state in fs_states {
        for (fname, fconts) in fs_state {
            fs::write(format!("{}/{}", &scratch_dir, fname), fconts)
                .expect("couldn't write test file");
        }

        let gits_args = &[
            vec!["add", "--all"],
            vec!["commit", "--message", "Initial commit"],
        ];
        for git_args in gits_args {
            run_cmd(scratch_dir, "git", git_args);
        }
    }

    let git_args = &["clone", "--bare", &scratch_dir, &repo_dir];
    run_cmd(scratch_dir, "git", git_args);
}

pub fn run_cmd<I, S>(dir: &str, prog: &str, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(prog);
    let output = cmd
        .args(args)
        .current_dir(dir)
        .env_clear()
        .output()
        .unwrap_or_else(|_|
            panic!("couldn't run `{:?}`", cmd)
        );

    assert!(
        output.status.success(),
        "`{:?}` failed:\n{}\n{}",
        cmd,
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    );

    String::from_utf8(output.stdout)
        .unwrap_or_else(|_|
            panic!("couldn't convert `{:?}` output to `String`", cmd)
        )
}

// `get_repo_hashes` returns hashes in chronological order, i.e. the first
// entry contains the hash of the oldest commit.
fn get_repo_hashes(repo_dir: &str) -> Vec<String> {
    run_cmd(&repo_dir, "git", &["log", "--reverse", "--format=%H"])
        .split_terminator('\n')
        .map(ToString::to_string)
        .collect()
}

pub fn write_test_deps_file(
    proj_dir: &str,
    deps_commit_hashes: &HashMap<String, Vec<String>>,
    deps_commit_nums: &HashMap<&str, usize>,
)
    -> String
{
    let mut deps_file_conts = formatdoc!{
        "
            # This is the output directory.
            deps

            # These are the dependencies.
        ",
    };

    for (dep_name, dep_commit_num) in deps_commit_nums {
        deps_file_conts = formatdoc!(
            "
                {deps_file_conts}
                {dep_name} git git://localhost/{dep_name}.git {dep_vsn}
            ",
            deps_file_conts = deps_file_conts,
            dep_name = dep_name,
            dep_vsn = deps_commit_hashes[*dep_name][*dep_commit_num],
        )
    }

    let deps_file = format!("{}/dpnd.txt", proj_dir);
    fs::write(&deps_file, &deps_file_conts)
        .expect("couldn't write dependency file");

    deps_file_conts
}

pub fn with_git_server<S, F, T>(dir: S, f: F) -> T
where
    F: FnOnce() -> T + UnwindSafe,
    S: AsRef<str>,
{
    let git_exec_path = run_cmd(dir.as_ref(), "git", &["--exec-path"]);

    let git_exec_path = git_exec_path
        .strip_suffix("\n")
        .expect("`git --exec-path` output didn't end with a newline");

    let git_exec_path = git_exec_path.to_owned();

    // We run `git-daemon` directly because `git daemon` spawns `git-daemon`
    // but we lose its PID in the process.
    //
    // TODO Store the output of the standard streams for debugging purposes.
    let mut daemon = Command::new(git_exec_path + "/git-daemon")
        .args(&["--reuseaddr", "--base-path=.", "--export-all", "."])
        .current_dir(dir.as_ref())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("couldn't spawn Git server");

    let result = panic::catch_unwind(f);

    daemon.kill()
        .expect("couldn't kill Git server");

    daemon.wait()
        .expect("couldn't wait for Git server");

    match result {
        Ok(v) => v,
        Err(err) => panic::resume_unwind(err),
    }
}

pub fn new_test_cmd(root_test_dir: String) -> AssertCommand {
    let mut cmd = AssertCommand::cargo_bin(env!("CARGO_PKG_NAME"))
        .expect("couldn't create command for package binary");
    cmd.current_dir(root_test_dir);
    cmd.env_clear();
    cmd.arg("install");

    cmd
}
