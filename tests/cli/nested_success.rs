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
// Given the dependency file contains nested dependencies
// When the command is run with `--recursive`
// Then the nested dependencies are pulled to the correct locations with the
//     correct contents
fn nested_deps_pulled_correctly_with_long_flag() {
    check_nested_deps_pulled_correctly(
        "nested_deps_pulled_correctly_with_long_flag",
        "--recursive",
    );
}

fn check_nested_deps_pulled_correctly(root_test_dir_name: &str, flag: &str) {
    let test_deps = success::test_deps();
    let Layout{dep_srcs_dir, proj_dir, ..} =
        test_setup::create(root_test_dir_name, &test_deps, &hashmap!{});
    let deps_file_conts = indoc!{"
        deps

        all_scripts git git://localhost/all_scripts.git master
    "};
    let deps_file = format!("{}/dpnd.txt", proj_dir);
    fs::write(&deps_file, &deps_file_conts)
        .expect("couldn't write dependency file");
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg(flag);

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(deps_file_conts),
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "all_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "dpnd.txt" => Node::AnyFile,
                    "script.sh" => Node::File("echo 'hello, all!'"),
                    "deps" => Node::Dir(hashmap!{
                        "current_dpnd.txt" => Node::AnyFile,
                        "my_scripts" => Node::Dir(hashmap!{
                            ".git" => Node::AnyDir,
                            "script.sh" => Node::File("echo 'hello, world!'"),
                        }),
                        "your_scripts" => Node::Dir(hashmap!{
                            ".git" => Node::AnyDir,
                            "script.sh" => Node::File("echo 'hello, sun!'"),
                        }),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the dependency file contains nested dependencies
// When the command is run with `-r`
// Then the nested dependencies are pulled to the correct locations with the
//     correct contents
fn nested_deps_pulled_correctly_with_short_flag() {
    check_nested_deps_pulled_correctly(
        "nested_deps_pulled_correctly_with_short_flag",
        "-r",
    );
}

#[test]
// Given the dependency file contains nested dependencies
// When the command is run without recursion
// Then the nested dependencies are not pulled
fn nested_deps_not_pulled_without_recursion() {
    let test_name = "nested_deps_not_pulled_without_recursion";
    check_nested_deps_not_pulled_without_recursion(test_name);
}

fn check_nested_deps_not_pulled_without_recursion(test_name: &str)
    -> Layout
{
    let test_deps = success::test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        test_setup::create(test_name, &test_deps, &hashmap!{});
    let deps_file_conts = indoc!{"
        deps

        all_scripts git git://localhost/all_scripts.git master
    "};
    let deps_file = format!("{}/dpnd.txt", proj_dir);
    fs::write(&deps_file, &deps_file_conts)
        .expect("couldn't write dependency file");
    let cmd_result = test_setup::with_git_server(
        &dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(deps_file_conts),
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "all_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "dpnd.txt" => Node::AnyFile,
                    "script.sh" => Node::File("echo 'hello, all!'"),
                }),
            }),
        }),
    );

    Layout{
        dep_srcs_dir,
        proj_dir,
        deps_commit_hashes,
        deps_file,
        deps_file_conts: deps_file_conts.to_string(),
    }
}

#[test]
// Given the dependency file contains nested dependencies and the command was
//     run without recursion
// When the command is run with recursion
// Then the nested dependencies are pulled to the correct locations with the
//     correct contents
fn run_with_recursion_after_run_without_recursion() {
    let test_name = "run_with_recursion_after_run_without_recursion";
    let Layout{deps_file_conts, dep_srcs_dir, proj_dir, ..} =
        check_nested_deps_not_pulled_without_recursion(test_name);
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "all_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "dpnd.txt" => Node::AnyFile,
                    "script.sh" => Node::File("echo 'hello, all!'"),
                    "deps" => Node::Dir(hashmap!{
                        "current_dpnd.txt" => Node::AnyFile,
                        "my_scripts" => Node::Dir(hashmap!{
                            ".git" => Node::AnyDir,
                            "script.sh" => Node::File("echo 'hello, world!'"),
                        }),
                        "your_scripts" => Node::Dir(hashmap!{
                            ".git" => Node::AnyDir,
                            "script.sh" => Node::File("echo 'hello, sun!'"),
                        }),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the dependency file contains nested dependencies that contain nested
//     dependencies
// When the command is run with `--recursive`
// Then the nested dependencies are pulled to the correct locations with the
//     correct contents
fn double_nested_deps_pulled_correctly() {
    let mut test_deps = success::test_deps();
    let nested_deps_file_conts = indoc!{"
        deps

        all_scripts git git://localhost/all_scripts.git master
    "};
    test_deps.insert(
        "nested_scripts",
        vec![hashmap!{
            "dpnd.txt" => nested_deps_file_conts,
            "script.sh" => "echo 'hello!'",
        }],
    );
    let Layout{dep_srcs_dir, proj_dir, ..} = test_setup::create(
        "double_nested_deps_pulled_correctly",
        &test_deps,
        &hashmap!{},
    );
    let deps_file_conts = indoc!{"
        deps

        nested_scripts git git://localhost/nested_scripts.git master
    "};
    let deps_file = format!("{}/dpnd.txt", proj_dir);
    fs::write(&deps_file, &deps_file_conts)
        .expect("couldn't write dependency file");
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.arg("--recursive");

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(deps_file_conts),
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "nested_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "dpnd.txt" => Node::File(nested_deps_file_conts),
                    "script.sh" => Node::File("echo 'hello!'"),
                    "deps" => Node::Dir(hashmap!{
                        "current_dpnd.txt" => Node::AnyFile,
                        "all_scripts" => Node::Dir(hashmap!{
                            ".git" => Node::AnyDir,
                            "dpnd.txt" => Node::AnyFile,
                            "script.sh" => Node::File("echo 'hello, all!'"),
                            "deps" => Node::Dir(hashmap!{
                                "current_dpnd.txt" => Node::AnyFile,
                                "my_scripts" => Node::Dir(hashmap!{
                                    ".git" => Node::AnyDir,
                                    "script.sh" =>
                                        Node::File("echo 'hello, world!'"),
                                }),
                                "your_scripts" => Node::Dir(hashmap!{
                                    ".git" => Node::AnyDir,
                                    "script.sh" =>
                                        Node::File("echo 'hello, sun!'"),
                                }),
                            }),
                        }),
                    }),
                }),
            }),
        }),
    );
}
