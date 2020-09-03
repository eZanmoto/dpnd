// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::panic;
use std::process::Command;

#[macro_use]
extern crate maplit;
extern crate assert_cmd;
extern crate indoc;

use assert_cmd::Command as AssertCommand;

#[test]
// Given the dependency file is in an empty directory and its dependencies are
//     available
// When the command is run
// Then dependencies are pulled to the correct locations with the correct
//     contents
fn dependencies_pulled_correctly() {
    let root_test_dir = create_root_test_dir("dependencies_pulled_correctly");
    let dep_dir = create_test_dir(root_test_dir.clone(), "my_scripts.git");
    let scratch_dir = create_test_dir(root_test_dir.clone(), "scratch");
    create_bare_git_repo(
        &dep_dir,
        &scratch_dir,
        "script.sh",
        "echo 'hello, world!'",
    );
    let test_proj_dir = create_test_dir(root_test_dir.clone(), "proj");
    let deps_file_conts = indoc::indoc! {"
        # This is the output directory.
        target/deps

        # These are the dependencies.
        my_scripts git git://localhost/my_scripts.git master
    "};
    let cmd_result = with_git_server(
        root_test_dir,
        || {
            fs::write(format!("{}/dpnd.txt", test_proj_dir), &deps_file_conts)
                .expect("couldn't write dependency file");
            let mut cmd = new_test_cmd(test_proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    assert_fs_contents(
        &test_proj_dir,
        &Node::Dir(hashmap! {
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

fn create_root_test_dir(name: &str) -> String {
    create_test_dir(env!("TEST_DIR").to_string(), name)
}

fn create_test_dir(dir: String, name: &str) -> String {
    let path = dir + "/" + name;

    fs::create_dir(&path)
        .expect("couldn't create directory");

    path
}

// `scratch_dir` is expected to be an empty directory that
// `create_bare_git_repo` can use for its own purposes.
fn create_bare_git_repo(
    repo_dir: &str,
    scratch_dir: &str,
    fname: &str,
    fconts: &str,
) {
    fs::write(format!("{}/{}", &scratch_dir, fname), fconts)
        .expect("couldn't write test file");

    let gits_args = &[
        vec!["init"],
        vec!["config", "user.name", "Test"],
        vec!["config", "user.email", "test@example.com"],
        vec!["add", "--all"],
        vec!["commit", "--message", "Initial commit"],
        vec!["clone", "--bare", &scratch_dir, &repo_dir],
    ];
    for git_args in gits_args {
        run_cmd(scratch_dir.to_string(), "git", git_args);
    }
}

fn run_cmd<I, S>(dir: String, cmd: &str, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    AssertCommand::new(cmd)
        .args(args)
        .current_dir(dir)
        .env_clear()
        .assert()
        .code(0);
}

fn with_git_server<F, T>(dir: String, f: F) -> T
where
    F: FnOnce() -> T,
{
    let git_exec_path_output = Command::new("git")
        .args(&["--exec-path"])
        .output()
        .expect("couldn't get Git execution path");

    assert!(git_exec_path_output.status.success());

    let git_exec_path = String::from_utf8(git_exec_path_output.stdout)
        .expect("couldn't convert `git --exec-path` output to `String`");

    let git_exec_path = git_exec_path
        .strip_suffix("\n")
        .expect("`git --exec-path` output didn't end with a newline");

    let git_exec_path = git_exec_path.to_owned();

    // We run `git-daemon` directly because `git daemon` spawns `git-daemon`
    // but we lose its PID in the process.
    let mut daemon = Command::new(git_exec_path + "/git-daemon")
        .args(&["--reuseaddr", "--base-path=.", "--export-all", "."])
        .current_dir(dir)
        .spawn()
        .expect("couldn't spawn Git server");

    let v = f();

    daemon.kill()
        .expect("couldn't kill Git server");

    daemon.wait()
        .expect("couldn't wait for Git server");

    v
}

fn new_test_cmd(root_test_dir: String) -> AssertCommand {
    let mut cmd = AssertCommand::cargo_bin(env!("CARGO_PKG_NAME"))
        .expect("couldn't create command for package binary");
    cmd.current_dir(root_test_dir);
    cmd.env_clear();

    cmd
}

enum Node<'a> {
    AnyDir,
    Dir(HashMap<&'a str, Node<'a>>),
    File(&'a str),
}

fn assert_fs_contents<'a>(path: &str, exp: &Node<'a>) {
    match exp {
        Node::File(exp_conts) => {
            let act_conts =
                fs::read(&path)
                    .unwrap_or_else(|_| panic!(
                        "couldn't open '{}' as a file",
                        &path,
                    ));

            assert!(
                exp_conts.as_bytes().to_vec() == act_conts,
                format!("'{}' contained unexpected data", &path),
            );
        }
        Node::AnyDir => {
            fs::read_dir(&path)
                .unwrap_or_else(|_| panic!(
                    "couldn't open '{}' as a directory",
                    path,
                ));
        }
        Node::Dir(exp_entries) => {
            let act_entries =
                fs::read_dir(&path)
                    .unwrap_or_else(|_| panic!(
                        "couldn't open '{}' as a directory",
                        &path,
                    ));

            let mut act_entry_names: HashSet<String> = HashSet::new();
            for act_entry in act_entries {
                let entry =
                    act_entry
                        .unwrap_or_else(|_| panic!(
                            "couldn't get entry from '{}'",
                            &path,
                        ));

                let entry_name: String =
                    entry
                        .file_name()
                        .into_string()
                        .expect("couldn't read entry's file name as Unicode");

                let exp_entry =
                    exp_entries
                        .get::<str>(&entry_name)
                        .unwrap_or_else(|| panic!(
                            "found unexpected entry '{}/{}' in filesystem",
                            path,
                            entry_name,
                        ));

                assert_fs_contents(
                    &format!("{}/{}", path, entry_name),
                    exp_entry,
                );

                act_entry_names.insert(entry_name);
            }

            for exp_entry_name in exp_entries.keys() {
                assert!(
                    act_entry_names.contains::<str>(&exp_entry_name),
                    format!(
                        "couldn't find expected entry '{}/{}' in filesystem",
                        path,
                        exp_entry_name,
                    ),
                );
            }
        }
    }
}

#[test]
// Given the dependency file doesn't exist
// When the command is run
// Then the command fails with an error
fn missing_deps_file() {
    let root_test_dir = create_root_test_dir("missing_deps_file");
    let test_proj_dir = create_test_dir(root_test_dir, "proj");
    let mut cmd = new_test_cmd(test_proj_dir);

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("Couldn't find the dependency file 'dpnd.txt' in the current directory or parent directories\n");
}

fn setup_test_with_deps_file<C: AsRef<[u8]>>(
    root_test_dir_name: &str,
    conts: C,
)
    -> AssertCommand
{
    let root_test_dir = create_root_test_dir(root_test_dir_name);
    let test_proj_dir = create_test_dir(root_test_dir, "proj");
    fs::write(format!("{}/dpnd.txt", test_proj_dir), conts)
        .expect("couldn't write dependency file");

    new_test_cmd(test_proj_dir)
}

#[test]
// Given the dependency file is empty
// When the command is run
// Then the command fails with an error
fn empty_deps_file() {
    let mut cmd = setup_test_with_deps_file("empty_deps_file", "");

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("The dependency file doesn't contain an output directory\n");
}

#[test]
// Given the dependency file contains an invalid UTF-8 sequence
// When the command is run
// Then the command fails with an error
fn deps_file_invalid_utf8() {
    let mut cmd = setup_test_with_deps_file(
        "deps_file_invalid_utf8",
        [0x00, 0x00, 0x00, 0x00, 0xa0, 0x00, 0x00, 0x00],
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("The dependency file contains an invalid UTF-8 sequence after byte 4\n");
}

#[test]
// Given the dependency file contains an invalid dependency specification
// When the command is run
// Then the command fails with an error
fn deps_file_invalid_dep() {
    let mut cmd = setup_test_with_deps_file(
        "deps_file_invalid_dep",
        indoc::indoc! {"
            target/deps

            proj tool source version extra
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("Line 3: Invalid dependency specification: 'proj tool source version extra'\n");
}

#[test]
// Given the dependency file contains an unknown tool
// When the command is run
// Then the command fails with an error
fn deps_file_invalid_tool() {
    let mut cmd = setup_test_with_deps_file(
        "deps_file_invalid_tool",
        indoc::indoc! {"
            target/deps

            proj tool source version
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("Line 3: The 'proj' dependency specifies an invalid tool name ('tool'); the supported tool is 'git'\n");
}

#[test]
// Given the dependency file specifies a Git dependency that is unavailable
// When the command is run
// Then the command fails with an error
fn unavailable_git_proj_src() {
    let mut cmd = setup_test_with_deps_file(
        "unavailable_git_proj_src",
        indoc::indoc! {"
            target/deps

            proj git git://localhost/my_scripts.git master
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(indoc::indoc!{"
            Couldn't retrieve the source for the 'proj' dependency: `git clone git://localhost/my_scripts.git .` failed with the following output:

            [!] Cloning into '.'...
            [!] fatal: unable to connect to localhost:
            [!] localhost[0: 127.0.0.1]: errno=Connection refused
            [!] localhost[1: ::1]: errno=Cannot assign requested address
            [!] 

        "});
}

#[test]
// Given the dependency file specifies a Git version that is unavailable
// When the command is run
// Then the command fails with the output of the Git command
fn unavailable_git_proj_vsn() {
    let root_test_dir = create_root_test_dir("unavailable_git_proj_vsn");
    let dep_dir = create_test_dir(root_test_dir.clone(), "my_scripts.git");
    let scratch_dir = create_test_dir(root_test_dir.clone(), "scratch");
    create_bare_git_repo(
        &dep_dir,
        &scratch_dir,
        "script.sh",
        "echo 'hello, world!'",
    );
    let test_proj_dir = create_test_dir(root_test_dir.clone(), "proj");
    let deps_file_conts = indoc::indoc! {"
        target/deps

        my_scripts git git://localhost/my_scripts.git bad_commit
    "};
    let cmd_result = with_git_server(root_test_dir, || {
        fs::write(test_proj_dir.to_string() + "/dpnd.txt", &deps_file_conts)
            .expect("couldn't write dependency file");
        let mut cmd = new_test_cmd(test_proj_dir.clone());

        cmd.assert()
    });

    cmd_result
        .code(1)
        .stdout("")
        .stderr(indoc::indoc!{"
            Couldn't change the version for the 'my_scripts' dependency: `git checkout bad_commit` failed with the following output:

            [!] error: pathspec 'bad_commit' did not match any file(s) known to git.

        "});
}

#[test]
// Given the main output directory is a file
// When the command is run
// Then the command fails with an error
fn main_output_dir_is_file() {
    let root_test_dir = create_root_test_dir("main_output_dir_is_file");
    let test_proj_dir = create_test_dir(root_test_dir, "proj");
    fs::write(test_proj_dir.to_string() + "/target", "")
        .expect("couldn't write dummy target file");
    let deps_file_conts = "target/deps\n";
    fs::write(test_proj_dir.to_string() + "/dpnd.txt", &deps_file_conts)
        .expect("couldn't write dependency file");
    let mut cmd = new_test_cmd(test_proj_dir);

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("Couldn't create 'target/deps', the main output directory: Not a directory (os error 20)\n");
}

#[test]
// Given the output directory for a dependency is a file
// When the command is run
// Then the command fails with an error
fn dep_output_dir_is_file() {
    let root_test_dir = create_root_test_dir("dep_output_dir_is_file");
    let test_proj_dir = create_test_dir(root_test_dir, "proj");
    let test_proj_deps_dir = create_test_dir(test_proj_dir.clone(), "deps");
    fs::write(test_proj_deps_dir + "/my_scripts", "")
        .expect("couldn't write dummy target file");
    let deps_file_conts = indoc::indoc! {"
        deps

        my_scripts git git://localhost/my_scripts.git master
    "};
    fs::write(test_proj_dir.to_string() + "/dpnd.txt", &deps_file_conts)
        .expect("couldn't write dependency file");
    let mut cmd = new_test_cmd(test_proj_dir);

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr("Couldn't create 'deps/my_scripts', the output directory for the 'my_scripts' dependency: File exists (os error 17)\n");
}
