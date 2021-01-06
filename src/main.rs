// Copyright 2020-2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::env;
use std::process;

mod dep_tools;
mod install;
mod render_errors;

use dep_tools::DepTool;
use dep_tools::Git;
use dep_tools::GitCmdError;
use install::Installer;

extern crate clap;
extern crate regex;
extern crate snafu;

use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::SubCommand;
use regex::Regex;

fn main() {
    let deps_file_name = "dpnd.txt";

    let install_about: &str = &format!(
        "Install dependencies defined in '{}'",
        deps_file_name,
    );
    let install_recursive_flag = "recursive";

    let args =
        App::new("dpnd")
            .version(env!("CARGO_PKG_VERSION"))
            .author(env!("CARGO_PKG_AUTHORS"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .settings(&[
                AppSettings::SubcommandRequiredElseHelp,
                AppSettings::VersionlessSubcommands,
            ])
            .subcommands(vec![
                SubCommand::with_name("install")
                    .about(install_about)
                    .args(&[
                        Arg::with_name(install_recursive_flag)
                            .short("r")
                            .long("recursive")
                            .help(
                                "Install dependencies found in dependencies",
                            ),
                    ]),
            ])
            .get_matches();

    match args.subcommand() {
        ("install", Some(sub_args)) => {
            let cwd = match env::current_dir() {
                Ok(dir) => {
                    dir
                },
                Err(err) => {
                    eprintln!("Couldn't get the current directory: {}", err);
                    process::exit(1);
                },
            };

            let mut tools: HashMap<String, &dyn DepTool<GitCmdError>> =
                HashMap::new();
            tools.insert("git".to_string(), &Git{});

            let bad_dep_name_chars = Regex::new(r"[^a-zA-Z0-9._-]").unwrap();
            let installer = &Installer{
                deps_file_name: deps_file_name.to_string(),
                state_file_name: format!("current_{}", deps_file_name),
                bad_dep_name_chars,
                tools,
            };
            let install_result = installer.install(
                &cwd,
                sub_args.is_present(install_recursive_flag),
            );
            if let Err(err) = install_result {
                let msg = render_errors::render_install_error(
                    err,
                    &cwd,
                    &deps_file_name,
                );
                eprintln!("{}", msg);
                process::exit(1);
            }
        },
        (arg_name, sub_args) => {
            // All subcommands defined in `args_defn` should be handled here,
            // so matching an unhandled command shouldn't happen.
            panic!(
                "unexpected command '{}' (arguments: '{:?}')",
                arg_name,
                sub_args,
            );
        },
    }
}
