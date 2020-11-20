// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::str;

pub trait DepTool<E> {
    // `name` returns an identifying name that should be unique across all
    // dependency tools.
    fn name(&self) -> String;

    fn fetch(
        &self,
        source: String,
        version: String,
        out_dir: &PathBuf,
    ) -> Result<(), FetchError<E>>;
}

#[derive(Debug)]
pub enum FetchError<E> {
    RetrieveFailed(E),
    VersionChangeFailed(E),
}

impl<E> Display for FetchError<E>
where
    E: Display
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let (act, err) = match self {
            FetchError::RetrieveFailed(err) =>
                ("retrieve dependency", err),
            FetchError::VersionChangeFailed(err) =>
                ("change the dependency version", err),
        };
        write!(f, "couldn't {}: {}", act, err)
    }
}

impl<E> Error for FetchError<E>
where
    E: Display + Debug
{}

#[derive(Debug)]
pub struct Git {}

impl DepTool<GitCmdError> for Git {
    fn name(&self) -> String {
        "git".to_string()
    }

    fn fetch(&self, src: String, vsn: String, out_dir: &PathBuf)
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
                Err(source) => {
                    let err = GitCmdError::StartFailed{
                        source,
                        args: owned_strs_to_strings(git_args),
                    };
                    if i == 0 {
                        return Err(FetchError::RetrieveFailed(err));
                    } else {
                        return Err(FetchError::VersionChangeFailed(err));
                    }
                }
            };

            if !output.status.success() {
                let err = GitCmdError::NotSuccess{
                    args: owned_strs_to_strings(git_args),
                    output,
                };
                if i == 0 {
                    return Err(FetchError::RetrieveFailed(err));
                } else {
                    return Err(FetchError::VersionChangeFailed(err));
                }
            }
        }

        Ok(())
    }
}

fn owned_strs_to_strings(strs: Vec<&str>) -> Vec<String> {
    strs.into_iter()
        .map(String::from)
        .collect()
}

#[derive(Debug)]
pub enum GitCmdError {
    StartFailed{source: IoError, args: Vec<String>},
    NotSuccess{args: Vec<String>, output: Output},
}

impl Display for GitCmdError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            GitCmdError::StartFailed{source, args} => {
                write!(
                    f,
                    "couldn't start `git {}`: {}",
                    args.join(" "),
                    source,
                )
            },
            GitCmdError::NotSuccess{args, output} => {
                let render_output = |bytes, name, prefix| {
                    if let Ok(s) = str::from_utf8(bytes) {
                        prefix_lines(s, prefix)
                    } else {
                        format!("{} (not UTF-8): {:?}", name, bytes)
                    }
                };

                write!(
                    f,
                    "`git {}` failed with the following output:\n\n{}{}",
                    args.join(" "),
                    render_output(&output.stdout, "STDOUT", "[>] "),
                    render_output(&output.stderr, "STDERR", "[!] "),
                )
            },
        }
    }
}

fn prefix_lines(src: &str, pre: &str) -> String {
    if src.is_empty() {
        return "".to_string();
    }

    let tgt = format!(
        "{}{}",
        pre,
        &src.replace("\n", &format!("\n{}", pre)),
    );

    if src.ends_with('\n') {
        match tgt.strip_suffix(pre) {
            Some(s) => s.to_string(),
            None => tgt,
        }
    } else {
        tgt
    }
}
