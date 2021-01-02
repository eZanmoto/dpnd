// Copyright 2020-2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::convert::AsRef;
use std::collections::HashMap;
use std::fs;
use std::string::ToString;

#[macro_use]
extern crate maplit;
extern crate assert_cmd;
extern crate indoc;

use assert_cmd::Command as AssertCommand;

mod fs_check;
mod test_setup;

use fs_check::Node;
use test_setup::Layout;

#[test]
// Given the dependency file is in an empty directory and the newest version of
//     its dependency is specified
// When the command is run
// Then dependencies are pulled to the correct locations with the correct
//     contents
fn new_dep_vsn_pulled_correctly() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_file_conts, ..} =
        test_setup::create(
            "new_dep_vsn_pulled_correctly",
            &test_deps,
            &hashmap!{"my_scripts" => 1},
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

// `test_deps` defines dependencies that will be created as git repositories.
// Each `Vec` element defines a Git commit, in order from from the initial
// commit to the latest commit.
fn test_deps()
    -> HashMap<&'static str, Vec<HashMap<&'static str, &'static str>>>
{
    hashmap!{
        "my_scripts" => vec![
            hashmap!{"script.sh" => "echo 'hello world'"},
            hashmap!{"script.sh" => "echo 'hello, world!'"},
        ],
        "your_scripts" => vec![
            hashmap!{"script.sh" => "echo 'hello, sun!'"},
        ],
        "their_scripts" => vec![
            hashmap!{"script.sh" => "echo 'hello, moon!'"},
        ],
        "all_scripts" => vec![
            hashmap!{
                "dpnd.txt" => indoc::indoc!{"
                    deps

                    my_scripts git git://localhost/my_scripts.git master
                    your_scripts git git://localhost/your_scripts.git master
                "},
                "script.sh" => "echo 'hello, all!'",
            }
        ],
    }
}

#[test]
// Given the dependency file is in an empty directory and the oldest version of
//     its dependency is specified
// When the command is run
// Then dependencies are pulled to the correct locations with the correct
//     contents
fn old_dep_vsn_pulled_correctly() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_file_conts, ..} =
        test_setup::create(
            "old_dep_vsn_pulled_correctly",
            &test_deps,
            &hashmap!{"my_scripts" => 0},
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello world'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the dependency file is in a parent directory of the directory the
//     command is run in
// When the command is run
// Then dependencies are pulled to the correct locations relative to the
//     dependency file
fn run_in_proj_subdir() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_file_conts, ..} =
        test_setup::create(
            "run_in_proj_subdir",
            &test_deps,
            &hashmap!{"my_scripts" => 1},
        );
    let test_subdir = test_setup::create_dir(proj_dir.clone(), "sub");
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(test_subdir);

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "sub" => Node::Dir(hashmap!{}),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was run once and there have been no changes since
// When the command is run
// The dependencies don't change
fn tool_is_idempotent() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_file_conts, ..} =
        test_setup::create(
            "tool_is_idempotent",
            &test_deps,
            &hashmap!{"my_scripts" => 1},
        );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.assert()
                .code(0)
                .stdout("")
                .stderr("");

            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was just run with 0 dependencies in the depencency file and
//     then a dependency was added
// When the command is run
// Then the new dependency is pulled to the correct location with the correct
//     contents
fn add_first_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "add_first_dep",
            &test_deps,
            hashmap!{},
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{"my_scripts" => 1},
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

fn create_test_setup_and_run_tool(
    root_test_dir_name: &str,
    deps: &HashMap<&str, Vec<HashMap<&str, &str>>>,
    deps_commit_nums: HashMap<&str, usize>,
)
    -> Layout
{
    let layout =
        test_setup::create(root_test_dir_name, &deps, &deps_commit_nums);

    run_tool(&layout, deps, deps_commit_nums);

    layout
}

fn run_tool(
    layout: &Layout,
    deps: &HashMap<&str, Vec<HashMap<&str, &str>>>,
    deps_commit_nums: HashMap<&str, usize>,
) {
    let Layout{dep_srcs_dir, proj_dir, deps_file_conts, ..} = layout;

    test_setup::with_git_server(
        dep_srcs_dir.clone(),
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());
            cmd.assert()
                .code(0)
                .stdout("")
                .stderr("");
        },
    );

    let mut deps_output_dir = hashmap!{"current_dpnd.txt" => Node::AnyFile};
    for (dep_name, dep_commit_num) in deps_commit_nums {
        let mut dir_conts = hashmap!{".git" => Node::AnyDir};
        for (fname, fconts) in &deps[dep_name][dep_commit_num] {
            dir_conts.insert(fname, Node::File(fconts));
        }
        deps_output_dir.insert(dep_name, Node::Dir(dir_conts));
    }

    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(deps_output_dir),
            }),
        }),
    );
}

#[test]
// Given the tool was just run with 1 dependency in the depencency file and
//     then a dependency is added
// When the command is run
// Then the new dependency is pulled to the correct location with the correct
//     contents
fn add_second_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "add_second_dep",
            &test_deps,
            hashmap!{"my_scripts" => 1},
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{
            "my_scripts" => 1,
            "your_scripts" => 0,
        },
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
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
    );
}

#[test]
// Given the tool was just run with 2 dependencies in the depencency file and
//     then a dependency is added
// When the command is run
// Then the new dependency is pulled to the correct location with the correct
//     contents
fn add_third_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "add_third_dep",
            &test_deps,
            hashmap!{
                "my_scripts" => 1,
                "your_scripts" => 0,
            },
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{
            "my_scripts" => 1,
            "your_scripts" => 0,
            "their_scripts" => 0,
        },
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
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
                    "their_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, moon!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was just run with 3 dependencies in the depencency file and
//     then a dependency is removed
// When the command is run
// Then the directory of the removed dependency is removed
fn rm_third_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "rm_third_dep",
            &test_deps,
            hashmap!{
                "my_scripts" => 1,
                "your_scripts" => 0,
                "their_scripts" => 0,
            },
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{
            "my_scripts" => 1,
            "your_scripts" => 0,
        },
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
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
    );
}

#[test]
// Given the tool was just run with 2 dependencies in the depencency file and
//     then a dependency is removed
// When the command is run
// Then the directory of the removed dependency is removed
fn rm_second_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "rm_second_dep",
            &test_deps,
            hashmap!{
                "my_scripts" => 1,
                "your_scripts" => 0,
            },
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{"my_scripts" => 1},
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was just run with 1 dependency in the depencency file and
//     then a dependency is removed
// When the command is run
// Then the directory of the removed dependency is removed
fn rm_first_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "rm_first_dep",
            &test_deps,
            hashmap!{"my_scripts" => 1},
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{},
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was run with 1 dependency in the depencency file and
//     then a dependency is removed and then the tool was run and then a
//     dependency was added
// When the command is run
// Then dependencies are pulled to the correct locations with the correct
//     contents
fn add_after_rm() {
    let test_deps = test_deps();
    let layout =
        create_test_setup_and_run_tool(
            "add_after_rm",
            &test_deps,
            hashmap!{"my_scripts" => 1},
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &layout.proj_dir,
        &layout.deps_commit_hashes,
        &hashmap!{},
    );
    let layout = Layout{deps_file_conts, ..layout};
    run_tool(&layout, &test_deps, hashmap!{});
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} = layout;
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{"my_scripts" => 1},
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was just run with an old version of a dependency in the
//     depencency file and then the dependency was upgraded
// When the command is run
// Then the newer version of the dependency is pulled to the correct location
//     with the correct contents
fn upgrade_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "downgrade_dep",
            &test_deps,
            hashmap!{"my_scripts" => 0},
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{"my_scripts" => 1},
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the tool was just run with a new version of a dependency in the
//     depencency file and then the dependency was downgraded
// When the command is run
// Then the older version of the dependency is pulled to the correct location
//     with the correct contents
fn downgrade_dep() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        create_test_setup_and_run_tool(
            "upgrade_dep",
            &test_deps,
            hashmap!{"my_scripts" => 1},
        );
    let deps_file_conts = test_setup::write_test_deps_file(
        &proj_dir,
        &deps_commit_hashes,
        &hashmap!{"my_scripts" => 0},
    );
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello world'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the dependency file contains two versions of the same dependency with
//     different names
// When the command is run
// Then dependencies are pulled to the correct locations with the correct
//     contents
fn same_dep_diff_vsns() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        test_setup::create("same_dep_diff_vsns", &test_deps, &hashmap!{});
    let deps_file_conts = indoc::formatdoc!{
        "
            # This is the output directory.
            target/deps

            # These are the dependencies.
            my_scripts_v1 git git://localhost/my_scripts.git {}
            my_scripts_v2 git git://localhost/my_scripts.git {}
        ",
        deps_commit_hashes["my_scripts"][0],
        deps_commit_hashes["my_scripts"][1],
    };
    let deps_file = format!("{}/dpnd.txt", proj_dir);
    fs::write(&deps_file, &deps_file_conts)
        .expect("couldn't write dependency file");
    let cmd_result = test_setup::with_git_server(
        dep_srcs_dir,
        || {
            let mut cmd = test_setup::new_test_cmd(proj_dir.clone());

            cmd.assert()
        },
    );

    cmd_result.code(0).stdout("").stderr("");
    fs_check::assert_contents(
        &proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(&deps_file_conts),
            "target" => Node::Dir(hashmap!{
                "deps" => Node::Dir(hashmap!{
                    "current_dpnd.txt" => Node::AnyFile,
                    "my_scripts_v1" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello world'"),
                    }),
                    "my_scripts_v2" => Node::Dir(hashmap!{
                        ".git" => Node::AnyDir,
                        "script.sh" => Node::File("echo 'hello, world!'"),
                    }),
                }),
            }),
        }),
    );
}

#[test]
// Given the dependency file contains nested dependencies
// When the command is run with `--recursive`
// Then the nested dependencies are pulled to the correct locations with the
//     correct contents
fn nested_deps_pulled_correctly_with_long_flag() {
    check_nested_deps_pulled_correctly(
        "nested_deps_pulled_correctly_with_long_flag",
        "--recursive",
    )
}

fn check_nested_deps_pulled_correctly(root_test_dir_name: &str, flag: &str) {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, ..} =
        test_setup::create(&root_test_dir_name, &test_deps, &hashmap!{});
    let deps_file_conts = indoc::indoc!{"
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
    )
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
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        test_setup::create(test_name, &test_deps, &hashmap!{});
    let deps_file_conts = indoc::indoc!{"
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
    let mut test_deps = test_deps();
    let nested_deps_file_conts = indoc::indoc!{"
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
    let deps_file_conts = indoc::indoc!{"
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
        indoc::indoc!{"
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
        indoc::indoc!{"
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
        indoc::indoc!{"
            target/deps

            proj git git://localhost/my_scripts.git master
        "},
    );

    let cmd_result = cmd.assert();

    cmd_result
        .code(1)
        .stdout("")
        .stderr(indoc::indoc!{"
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
    let deps_file_conts = indoc::indoc!{"
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
        .stderr(indoc::indoc!{"
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
    let deps_file_conts = indoc::indoc!{"
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
        indoc::indoc!{"
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
        indoc::indoc!{"
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
    let mut test_deps = test_deps();
    test_deps.insert(
        "bad_dep",
        vec![hashmap!{
            "dpnd.txt" => nested_deps_file_conts,
            "script.sh" => "echo 'bad!'",
        }],
    );
    let Layout{dep_srcs_dir, proj_dir, ..} =
        test_setup::create(root_test_dir_name, &test_deps, &hashmap!{});

    let deps_file_conts = indoc::indoc!{"
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
    let nested_deps_file_conts = indoc::indoc!{"
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
    let nested_deps_file_conts = indoc::indoc!{"
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
    let nested_deps_file_conts = indoc::indoc!{"
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
        .stderr(indoc::indoc!{"
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
    let nested_deps_file_conts = indoc::indoc!{"
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
    let nested_deps_file_conts = indoc::indoc!{"
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
    let nested_deps_file_conts = indoc::indoc!{"
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
