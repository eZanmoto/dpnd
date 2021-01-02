// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::fs;

extern crate assert_cmd;

use self::assert_cmd::Command as AssertCommand;

use crate::test_setup;

#[test]
// Given the dependency file doesn't exist
// When the command is run
// Then the command fails with an error
fn missing_deps_file() {
    let root_test_dir = test_setup::create_root_dir("missing_deps_file");
    let test_proj_dir = test_setup::create_dir(root_test_dir, "proj");
    let mut cmd = test_setup::new_test_cmd(test_proj_dir);

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(
            "Couldn't find the dependency file 'dpnd.txt' in the current \
             directory or parent directories\n",
        );
}

fn setup_test_with_deps_file<C: AsRef<[u8]>>(
    root_test_dir_name: &str,
    conts: C,
)
    -> (String, AssertCommand)
{
    let root_test_dir = test_setup::create_root_dir(root_test_dir_name);
    let test_proj_dir = test_setup::create_dir(root_test_dir, "proj");
    fs::write(format!("{}/dpnd.txt", test_proj_dir), conts)
        .expect("couldn't write dependency file");

    (test_proj_dir.clone(), test_setup::new_test_cmd(test_proj_dir))
}

#[test]
// Given the dependency file is empty
// When the command is run
// Then the command fails with an error
fn empty_deps_file() {
    let (test_proj_dir, mut cmd) =
        setup_test_with_deps_file("empty_deps_file", "");

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/dpnd.txt: This dependency file doesn't contain an output \
             directory\n",
            test_proj_dir,
        ));
}

#[test]
// Given the dependency file contains an invalid UTF-8 sequence
// When the command is run
// Then the command fails with an error
fn deps_file_invalid_utf8() {
    let (test_proj_dir, mut cmd) = setup_test_with_deps_file(
        "deps_file_invalid_utf8",
        [0x00, 0x00, 0x00, 0x00, 0xa0, 0x00, 0x00, 0x00],
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/dpnd.txt: This dependency file contains an invalid UTF-8 \
             sequence after byte 4\n",
            test_proj_dir,
        ));
}

#[test]
// Given the dependency file contains an invalid dependency specification
// When the command is run
// Then the command fails with an error
fn deps_file_invalid_dep() {
    let (test_proj_dir, mut cmd) = setup_test_with_deps_file(
        "deps_file_invalid_dep",
        indoc!{"
            target/deps

            proj tool source version extra
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/dpnd.txt:3: Invalid dependency specification: 'proj tool \
             source version extra'\n",
            test_proj_dir,
        ));
}

#[test]
// Given the dependency file contains an unknown tool
// When the command is run
// Then the command fails with an error
fn deps_file_invalid_tool() {
    let (test_proj_dir, mut cmd) = setup_test_with_deps_file(
        "deps_file_invalid_tool",
        indoc!{"
            target/deps

            proj tool source version
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/dpnd.txt:3: The dependency 'proj' specifies an invalid tool \
             name ('tool'); the supported tool is 'git'\n",
            test_proj_dir,
        ));
}

#[test]
// Given the dependency file specifies a Git dependency that is unavailable
// When the command is run
// Then the command fails with an error
fn unavailable_git_proj_src() {
    let (_, mut cmd) = setup_test_with_deps_file(
        "unavailable_git_proj_src",
        indoc!{"
            target/deps

            proj git git://localhost/my_scripts.git master
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(indoc!{"
            Couldn't retrieve the source for the dependency 'proj': `git \
             clone git://localhost/my_scripts.git .` failed with the \
             following output:

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
    let root_test_dir =
        test_setup::create_root_dir("unavailable_git_proj_vsn");
    let dep_dir =
        test_setup::create_dir(root_test_dir.clone(), "my_scripts.git");
    let scratch_dir = test_setup::create_dir(root_test_dir.clone(), "scratch");
    test_setup::create_bare_git_repo(
        &dep_dir,
        &scratch_dir,
        &[
            hashmap!{"script.sh" => "echo 'hello, world!'"},
        ],
    );
    let test_proj_dir = test_setup::create_dir(root_test_dir.clone(), "proj");
    let deps_file_conts = indoc!{"
        target/deps

        my_scripts git git://localhost/my_scripts.git bad_commit
    "};
    let cmd_result = test_setup::with_git_server(
        root_test_dir,
        || {
            fs::write(
                test_proj_dir.to_string() + "/dpnd.txt",
                &deps_file_conts,
            )
                .expect("couldn't write dependency file");
            let mut cmd = test_setup::new_test_cmd(test_proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(indoc!{"
            Couldn't change the version for the 'my_scripts' dependency: `git \
             checkout bad_commit` failed with the following output:

            [!] error: pathspec 'bad_commit' did not match any file(s) known \
             to git.

        "});
}

#[test]
// Given the main output directory is a file
// When the command is run
// Then the command fails with an error
fn main_output_dir_is_file() {
    let root_test_dir = test_setup::create_root_dir("main_output_dir_is_file");
    let test_proj_dir = test_setup::create_dir(root_test_dir, "proj");
    fs::write(test_proj_dir.to_string() + "/target", "")
        .expect("couldn't write dummy target file");
    let deps_file_conts = "target/deps\n";
    fs::write(test_proj_dir.to_string() + "/dpnd.txt", &deps_file_conts)
        .expect("couldn't write dependency file");
    let mut cmd = test_setup::new_test_cmd(test_proj_dir.clone());

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "Couldn't read the state file \
             ('{}/target/deps/current_dpnd.txt'): Not a directory (os error \
             20)\n",
            test_proj_dir,
        ));
}

#[test]
// Given the output directory for a dependency is a file
// When the command is run
// Then the command fails with an error
fn dep_output_dir_is_file() {
    let root_test_dir = test_setup::create_root_dir("dep_output_dir_is_file");
    let test_proj_dir = test_setup::create_dir(root_test_dir, "proj");
    let test_proj_deps_dir =
        test_setup::create_dir(test_proj_dir.clone(), "deps");
    fs::write(test_proj_deps_dir + "/my_scripts", "")
        .expect("couldn't write dummy target file");
    let deps_file_conts = indoc!{"
        deps

        my_scripts git git://localhost/my_scripts.git master
    "};
    fs::write(test_proj_dir.to_string() + "/dpnd.txt", &deps_file_conts)
        .expect("couldn't write dependency file");
    let mut cmd = test_setup::new_test_cmd(test_proj_dir.clone());

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "Couldn't remove '{}/deps/my_scripts', the output directory for \
             the 'my_scripts' dependency: Not a directory (os error 20)\n",
            test_proj_dir,
        ));
}

#[test]
// Given the dependency file contains two dependencies with the same name
// When the command is run
// Then the command fails with an error
fn dup_dep_names() {
    let (test_proj_dir, mut cmd) = setup_test_with_deps_file(
        "dup_dep_names",
        indoc!{"
            target/deps

            my_scripts git git://localhost/my_scripts.git master
            my_scripts git git://localhost/my_scripts.git master
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/dpnd.txt:4: A dependency named 'my_scripts' is already \
             defined on line 3\n",
            test_proj_dir,
        ));
}

#[test]
// Given the dependency file contains a dependency with an invalid name
// When the command is run
// Then the command fails with an error
fn invalid_dep_name() {
    let (test_proj_dir, mut cmd) = setup_test_with_deps_file(
        "invalid_dep_name",
        indoc!{"
            target/deps

            my_scripts? git git://localhost/my_scripts.git master
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/dpnd.txt:3: 'my_scripts?' contains an invalid character ('?') \
             at position 11; dependency names can only contain numbers, \
             letters, hyphens, underscores and periods\n",
            test_proj_dir,
        ));
}
