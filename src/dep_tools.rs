// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

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

#[derive(Debug)]
pub struct Git {}

impl DepTool<String> for Git {
    fn name(&self) -> String {
        "git".to_string()
    }

    fn fetch(&self, src: String, vsn: String, out_dir: &PathBuf)
        -> Result<(), FetchError<String>>
    {
        let gits_args = vec![
            vec!["clone", &src, "."],
            vec!["checkout", &vsn],
        ];

        for (i, git_args) in gits_args.iter().enumerate() {
            let maybe_output =
                Command::new("git")
                    .args(git_args)
                    .current_dir(out_dir)
                    .output();

            let output = match maybe_output {
                Ok(output) => output,
                Err(err) => {
                    let msg = format!(
                        "couldn't start `git {}`: {}",
                        git_args.join(" "),
                        err,
                    );
                    if i == 0 {
                        return Err(FetchError::RetrieveFailed(msg));
                    } else {
                        return Err(FetchError::VersionChangeFailed(msg));
                    }
                }
            };

            if !output.status.success() {
                let msg = render_git_failure(&git_args.join(" "), &output);
                if i == 0 {
                    return Err(FetchError::RetrieveFailed(msg));
                } else {
                    return Err(FetchError::VersionChangeFailed(msg));
                }
            }
        }

        Ok(())
    }
}

fn render_git_failure(args: &str, output: &Output) -> String {
    let render_output = |bytes, name, prefix| {
        if let Ok(s) = str::from_utf8(bytes) {
            prefix_lines(s, prefix)
        } else {
            format!("{} (not UTF-8): {:?}", name, bytes)
        }
    };

    format!(
        "`git {}` failed with the following output:\n\n{}{}",
        args,
        render_output(&output.stdout, "STDOUT", "[>] "),
        render_output(&output.stderr, "STDERR", "[!] "),
    )
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
