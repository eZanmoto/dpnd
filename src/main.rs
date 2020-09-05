// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Error as IoError;
use std::iter::Enumerate;
use std::path::PathBuf;
use std::process;
use std::str::Lines;
use std::string::FromUtf8Error;

mod dep_tools;
#[macro_use] mod wrap_err;

use dep_tools::DepTool;
use dep_tools::DepToolFactory;
use dep_tools::FetchError;
use dep_tools::GitFactory;

fn main() {
    let deps_file_name = "dpnd.txt";

    if let Err(err) = install(&deps_file_name) {
        print_install_error(err, deps_file_name);
        process::exit(1);
    }
}

fn install(deps_file_name: &str) -> Result<(), InstallError<String>> {
    let cwd = wrap_err!(
        env::current_dir(),
        InstallError::GetCurrentDirFailed,
    );

    let (_deps_dir, deps_spec) = match read_deps_file(cwd, &deps_file_name) {
        Some(v) => v,
        None => return Err(InstallError::NoDepsFileFound),
    };

    let deps_spec = wrap_err!(
        String::from_utf8(deps_spec),
        InstallError::ConvDepsFileUtf8Failed,
    );

    let mut tool_factories: HashMap<String, &dyn DepToolFactory<String>> =
        HashMap::new();
    tool_factories.insert("git".to_string(), &GitFactory {});

    let mut conf = wrap_err!(
        parse_deps_conf(&deps_spec, &tool_factories),
        InstallError::ParseDepsConfFailed,
    );

    wrap_err!(
        fs::create_dir_all(&conf.output_dir),
        InstallError::CreateMainOutputDirFailed,
        conf.output_dir,
    );

    while let Some(dep) = conf.deps.pop() {
        let dir = conf.output_dir.join(&dep.local_name);
        wrap_err!(
            fs::create_dir(&dir),
            InstallError::CreateDepOutputDirFailed,
            dep.local_name,
            dir,
        );

        wrap_err!(
            dep.tool.fetch(
                (&dep.source).to_string(),
                (&dep.version).to_string(),
                &dir,
            ),
            InstallError::FetchFailed,
            dep.local_name.clone(),
        );
    }

    Ok(())
}

enum InstallError<E> {
    GetCurrentDirFailed(IoError),
    NoDepsFileFound,
    ConvDepsFileUtf8Failed(FromUtf8Error),
    ParseDepsConfFailed(ParseDepsConfError),
    CreateMainOutputDirFailed(IoError, PathBuf),
    CreateDepOutputDirFailed(IoError, String, PathBuf),
    FetchFailed(FetchError<E>, String),
}

// `read_deps_file` reads the file named `deps_file_name` in `start` or the
// deepest of `start`s ancestor directories that contains a file named
// `deps_file_name`.
fn read_deps_file(start: PathBuf, deps_file_name: &str)
    -> Option<(PathBuf, Vec<u8>)>
{
    let mut dir = start;
    loop {
        if let Ok(conts) = fs::read(dir.clone().join(deps_file_name)) {
            return Some((dir, conts));
        }

        if !dir.pop() {
            return None;
        }
    }
}

fn parse_deps_conf<'a>(
    conts: &str,
    tool_factories: &HashMap<String, &'a (dyn DepToolFactory<String> + 'a)>,
)
    -> Result<DepsConf<'a, String>, ParseDepsConfError>
{
    let mut lines = conts.lines().enumerate();

    if let Some(output_dir) = parse_output_dir(&mut lines) {
        Ok(DepsConf {
            output_dir,
            deps: wrap_err!(
                parse_deps(&mut lines, &tool_factories),
                ParseDepsConfError::ParseDepsFailed,
            ),
        })
    } else {
        Err(ParseDepsConfError::MissingOutputDir)
    }
}

enum ParseDepsConfError {
    MissingOutputDir,
    ParseDepsFailed(ParseDepsError),
}

struct DepsConf<'a, E> {
    output_dir: PathBuf,
    deps: Vec<Dependency<'a, E>>,
}

fn parse_output_dir(lines: &mut Enumerate<Lines>) -> Option<PathBuf> {
    for (_, line) in lines {
        let ln = line.trim_start();
        if !conf_line_is_skippable(ln) {
            let mut path = PathBuf::new();
            for part in ln.split('/') {
                path.push(part);
            }
            return Some(path);
        }
    }

    None
}

fn conf_line_is_skippable(ln: &str) -> bool {
    ln.is_empty() || ln.starts_with('#')
}

fn parse_deps<'a>(
    lines: &mut Enumerate<Lines>,
    tool_factories: &HashMap<String, &'a (dyn DepToolFactory<String> + 'a)>,
)
    -> Result<Vec<Dependency<'a, String>>, ParseDepsError>
{
    let mut dep_defns: Vec<(Dependency<'a, String>, usize)> = vec![];

    for (i, line) in lines {
        let ln_num = i + 1;

        let ln = line.trim_start();
        if conf_line_is_skippable(ln) {
            continue;
        }

        let words: Vec<&str> = ln.split_ascii_whitespace().collect();
        if words.len() != 4 {
            return Err(ParseDepsError::InvalidDepSpec(
                ln_num,
                ln.to_string(),
            ));
        }

        let local_name = words[0].to_string();
        for (dep, defn_ln_num) in &dep_defns {
            if dep.local_name == local_name {
                return Err(ParseDepsError::DupDepName(
                    ln_num,
                    local_name,
                    *defn_ln_num,
                ));
            }
        }

        let tool_name = words[1].to_string();
        let tool = match tool_factories.get(&tool_name) {
            Some(tool_factory) => tool_factory.create(),
            None => return Err(ParseDepsError::UnknownTool(
                ln_num,
                local_name,
                tool_name,
            )),
        };

        dep_defns.push((
            Dependency {
                local_name,
                tool,
                source: words[2].to_string(),
                version: words[3].to_string(),
            },
            ln_num,
        ));
    }

    let deps =
        dep_defns.into_iter()
            .map(|(dep, _)| {
                dep
            })
            .collect();

    Ok(deps)
}

struct Dependency<'a, E> {
    local_name: String,
    tool: &'a (dyn DepTool<E> + 'a),
    source: String,
    version: String,
}

enum ParseDepsError {
    DupDepName(usize, String, usize),
    InvalidDepSpec(usize, String),
    UnknownTool(usize, String, String),
}

fn print_install_error(err: InstallError<String>, deps_file_name: &str) {
    match err {
        InstallError::GetCurrentDirFailed(io_err) =>
            eprintln!("Couldn't get the current directory: {}", io_err),
        InstallError::NoDepsFileFound =>
            eprintln!(
                "Couldn't find the dependency file '{}' in the current \
                 directory or parent directories",
                deps_file_name,
            ),
        InstallError::ConvDepsFileUtf8Failed(err) =>
            eprintln!(
                "The dependency file contains an invalid UTF-8 sequence after \
                 byte {}",
                err.utf8_error().valid_up_to(),
            ),
        InstallError::ParseDepsConfFailed(err) =>
            match err {
                ParseDepsConfError::MissingOutputDir =>
                    eprintln!(
                        "The dependency file doesn't contain an output \
                         directory"
                    ),
                ParseDepsConfError::ParseDepsFailed(err) =>
                    match err {
                        ParseDepsError::DupDepName(ln_num, dep, orig_ln_num) =>
                            eprintln!(
                                "Line {}: A dependency named '{}' is already \
                                 defined on line {}",
                                ln_num,
                                dep,
                                orig_ln_num,
                            ),
                        ParseDepsError::InvalidDepSpec(ln_num, ln) =>
                            eprintln!(
                                "Line {}: Invalid dependency specification: \
                                 '{}'",
                                ln_num,
                                ln,
                            ),
                        ParseDepsError::UnknownTool(
                            ln_num,
                            dep_name,
                            tool_name,
                        ) =>
                            eprintln!(
                                "Line {}: The '{}' dependency specifies an \
                                 invalid tool name ('{}'); the supported tool \
                                 is 'git'",
                                ln_num,
                                dep_name,
                                tool_name,
                            ),
                    },
            },
        InstallError::CreateMainOutputDirFailed(io_err, path) =>
            eprintln!(
                "Couldn't create {}, the main output directory: {}",
                render_path(&path),
                io_err,
            ),
        InstallError::CreateDepOutputDirFailed(io_err, dep, path) =>
            eprintln!(
                "Couldn't create {}, the output directory for the '{}' \
                 dependency: {}",
                render_path(&path),
                dep,
                io_err,
            ),
        InstallError::FetchFailed(err, dep_name) =>
            match err {
                FetchError::RetrieveFailed(msg) =>
                    eprintln!(
                        "Couldn't retrieve the source for the '{}' \
                         dependency: {}",
                        dep_name,
                        msg,
                    ),
                FetchError::VersionChangeFailed(msg) =>
                    eprintln!(
                        "Couldn't change the version for the '{}' dependency: \
                         {}",
                        dep_name,
                        msg,
                    ),
            },
    }
}

fn render_path(path: &PathBuf) -> String {
    if let Some(s) = path.to_str() {
        format!("'{}'", s)
    } else {
        format!("{:?}", path)
    }
}
