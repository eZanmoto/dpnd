// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::fs;

use crate::fs_check;
use crate::fs_check::Node;
use crate::test_setup;
use crate::test_setup::Layout;

use super::success;

#[test]
// Given the dependency file of a nested dependency is empty
// When the command is run with `--recursive`
// Then the command fails with an error
fn empty_deps_file_in_nested_dep() {
    let nested_deps_file_conts = "";
    let NestedTestSetup{dep_srcs_dir, proj_dir, deps_file_conts} =
        create_nested_test_setup(
            "empty_deps_file_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/deps/bad_dep/dpnd.txt: This nested dependency file (for \
            'bad_dep') doesn't contain an output directory\n",
            proj_dir,
        ));
    assert_nested_dep_contents(
        &proj_dir,
        &deps_file_conts,
        &nested_deps_file_conts,
    );
}

fn create_nested_test_setup(
    root_test_dir_name: &str,
    nested_deps_file_conts: &str,
)
    -> NestedTestSetup
{
    let mut test_deps = success::test_deps();
    test_deps.insert(
        "bad_dep",
        vec![hashmap!{
            "dpnd.txt" => nested_deps_file_conts,
            "script.sh" => "echo 'bad!'",
        }],
    );
    let Layout{dep_srcs_dir, proj_dir, ..} =
        test_setup::create(root_test_dir_name, &test_deps, &hashmap!{});

    let deps_file_conts = indoc!{"
        deps

        bad_dep git git://localhost/bad_dep.git master
    "};
    let deps_file = format!("{}/dpnd.txt", proj_dir);
    fs::write(&deps_file, &deps_file_conts)
        .expect("couldn't write dependency file");

    NestedTestSetup{
        dep_srcs_dir,
        proj_dir,
        deps_file_conts: deps_file_conts.to_string(),
    }
}

struct NestedTestSetup {
    dep_srcs_dir: String,
    proj_dir: String,
    deps_file_conts: String,
}

fn assert_nested_dep_contents(
    proj_dir: &str,
    deps_file_conts: &str,
    nested_deps_file_conts: &str,
) {
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "bad_dep" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "dpnd.txt" => Node::File(&nested_deps_file_conts),
                    "script.sh" => Node::File("echo 'bad!'"),
                }),
            }),
        }),
    );
}

#[test]
// Given the dependency file of a nested dependency contains an invalid
//     dependency specification
// When the command is run with `--recursive`
// Then the command fails with an error
fn deps_file_invalid_dep_in_nested_dep() {
    let nested_deps_file_conts = indoc!{"
        target/deps

        proj tool source version extra
    "};
    let NestedTestSetup{dep_srcs_dir, proj_dir, deps_file_conts} =
        create_nested_test_setup(
            "deps_file_invalid_dep_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/deps/bad_dep/dpnd.txt:3: Invalid dependency specification in \
             nested dependency 'bad_dep': 'proj tool source version extra'\n",
            proj_dir,
        ));
    assert_nested_dep_contents(
        &proj_dir,
        &deps_file_conts,
        &nested_deps_file_conts,
    );
}

#[test]
// Given the dependency file of a nested dependency contains an unknown tool
// When the command is run with `--recursive`
// Then the command fails with an error
fn deps_file_invalid_tool_in_nested_dep() {
    let nested_deps_file_conts = indoc!{"
        target/deps

        proj tool source version
    "};
    let NestedTestSetup{dep_srcs_dir, proj_dir, deps_file_conts} =
        create_nested_test_setup(
            "deps_file_invalid_tool_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/deps/bad_dep/dpnd.txt:3: The dependency 'proj' of the nested \
             dependency 'bad_dep' specifies an invalid tool name ('tool'); \
             the supported tool is 'git'\n",
            proj_dir,
        ));
    assert_nested_dep_contents(
        &proj_dir,
        &deps_file_conts,
        &nested_deps_file_conts,
    );
}

#[test]
// Given the dependency file of a nested dependency specifies a Git dependency
//     that is unavailable
// When the command is run
// Then the command fails with an error
fn unavailable_git_proj_src_in_nested_dep() {
    let nested_deps_file_conts = indoc!{"
        deps

        proj git git://localhost/no_scripts.git master
    "};
    let NestedTestSetup{dep_srcs_dir, proj_dir, ..} =
        create_nested_test_setup(
            "unavailable_git_proj_src_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(indoc!{"
            Couldn't retrieve the source for the dependency 'proj' in the \
             nested dependency 'bad_dep': `git clone \
             git://localhost/no_scripts.git .` failed with the following \
             output:

            [!] Cloning into '.'...
            [!] fatal: remote error: access denied or repository not \
             exported: /no_scripts.git

        "});
    // TODO Assert the contents of the filesystem.
}

#[test]
// Given the dependency file of a nested dependency contains two dependencies
//     with the same name
// When the command is run
// Then the command fails with an error
fn dup_dep_names_in_nested_dep() {
    let nested_deps_file_conts = indoc!{"
        deps

        my_scripts git git://localhost/my_scripts.git master
        my_scripts git git://localhost/my_scripts.git master
    "};
    let NestedTestSetup{dep_srcs_dir, proj_dir, ..} =
        create_nested_test_setup(
            "dup_dep_names_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/deps/bad_dep/dpnd.txt:4: A dependency named 'my_scripts' is \
             already defined on line 3 in the nested dependency 'bad_dep'\n",
            proj_dir,
        ));
}

#[test]
// Given the dependency file of a nested dependency contains a dependency with
//     an invalid name
// When the command is run
// Then the command fails with an error
fn invalid_dep_name_in_nested_dep() {
    let nested_deps_file_conts = indoc!{"
        deps

        my_scripts? git git://localhost/my_scripts.git master
    "};
    let NestedTestSetup{dep_srcs_dir, proj_dir, ..} =
        create_nested_test_setup(
            "invalid_dep_name_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/deps/bad_dep/dpnd.txt:3: 'my_scripts?' contains an invalid \
             character ('?') at position 11; dependency names can only \
             contain numbers, letters, hyphens, underscores and periods\n",
            proj_dir,
        ));
}

#[test]
// Given the dependency file of a nested dependency contains a dependency with
//     a reserved name
// When the command is run
// Then the command fails with an error
fn reserved_dep_name_in_nested_dep() {
    let nested_deps_file_conts = indoc!{"
        deps

        current_dpnd.txt git git://localhost/my_scripts.git master
    "};
    let NestedTestSetup{dep_srcs_dir, proj_dir, ..} =
        create_nested_test_setup(
            "reserved_dep_name_in_nested_dep",
            &nested_deps_file_conts,
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result
        .code(1)
        .stdout("")
        .stderr(format!(
            "{}/deps/bad_dep/dpnd.txt:3: 'current_dpnd.txt' is a reserved \
             name and can't be used as a dependency name\n",
            proj_dir,
        ));
}
