// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;
use std::path::Path;
use std::process::Command;
use std::process::Output;

extern crate snafu;

use snafu::Snafu;

pub trait DepTool<E>
where
    E: Error + 'static,
{
    // `name` returns an identifying name that should be unique across all
    // dependency tools.
    fn name(&self) -> String;

    fn fetch(
        &self,
        source: String,
        version: Version,
        out_dir: &Path,
    ) -> Result<(), FetchError<E>>;
}

#[derive(Clone, PartialEq)]
pub struct Version(pub String);

impl Display for Version {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Snafu)]
pub enum FetchError<E>
where
    E: Error + 'static,
{
    RetrieveFailed{source: E},
    VersionChangeFailed{source: E},
}

#[derive(Debug)]
pub struct Git {}

impl DepTool<GitCmdError> for Git {
    fn name(&self) -> String {
        "git".to_string()
    }

    fn fetch(&self, src: String, Version(vsn): Version, out_dir: &Path)
        -> Result<(), FetchError<GitCmdError>>
    {
        let gits_args = vec![
            vec!["clone", &src, "."],
            vec!["checkout", &vsn],
        ];

        for (i, git_args) in gits_args.into_iter().enumerate() {
            let maybe_output =
                Command::new("git")
                    .args(&git_args)
                    .current_dir(out_dir)
                    .output();

            let output = match maybe_output {
                Ok(output) => output,
                Err(err) => {
                    let source = GitCmdError::StartFailed{
                        source: err,
                        args: owned_strs_to_strings(git_args),
                    };
                    if i == 0 {
                        return Err(FetchError::RetrieveFailed{source});
                    }
                    return Err(FetchError::VersionChangeFailed{source});
                }
            };

            if !output.status.success() {
                let source = GitCmdError::NotSuccess{
                    args: owned_strs_to_strings(git_args),
                    output,
                };
                if i == 0 {
                    return Err(FetchError::RetrieveFailed{source});
                }
                return Err(FetchError::VersionChangeFailed{source});
            }
        }

        Ok(())
    }
}

#[derive(Debug, Snafu)]
pub enum GitCmdError {
    StartFailed{source: IoError, args: Vec<String>},
    NotSuccess{args: Vec<String>, output: Output},
}

fn owned_strs_to_strings(strs: Vec<&str>) -> Vec<String> {
    strs.into_iter()
        .map(String::from)
        .collect()
}
