// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::fs;

use crate::fs_check;
use crate::fs_check::Node;
use crate::test_setup;
use crate::test_setup::Layout;

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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
                }),
            }),
        }),
    );
}

// `test_deps` defines dependencies that will be created as git repositories.
// Each `Vec` element defines a Git commit, in order from from the initial
// commit to the latest commit.
pub fn test_deps()
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
                "dpnd.txt" => indoc!{"
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello world'"),
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
                }),
            }),
        }),
    );
}

#[test]
// Given the output directory is a subdirectory of another directory
// When the command is run
// Then dependencies are pulled to the correct locations
fn output_dir_is_subdir() {
    let test_deps = test_deps();
    let Layout{dep_srcs_dir, proj_dir, deps_commit_hashes, ..} =
        test_setup::create("output_dir_is_subdir", &test_deps, &hashmap!{});
    let deps_file_conts = formatdoc!{
        "
            # This is the output directory.
            target/deps

            # These are the dependencies.
            my_scripts git git://localhost/my_scripts.git {}
        ",
        deps_commit_hashes["my_scripts"][0],
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
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
        test_setup::create(root_test_dir_name, deps, &deps_commit_nums);

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
        proj_dir,
        &Node::Dir(hashmap!{
            "dpnd.txt" => Node::File(deps_file_conts),
            "deps" => Node::Dir(deps_output_dir),
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello, world!'"),
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
            "deps" => Node::Dir(hashmap!{
                "current_dpnd.txt" => Node::AnyFile,
                "my_scripts" => Node::Dir(hashmap!{
                    ".git" => Node::AnyDir,
                    "script.sh" => Node::File("echo 'hello world'"),
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
    let deps_file_conts = formatdoc!{
        "
            # This is the output directory.
            deps

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
    );
}
