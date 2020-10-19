// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Write;
use std::iter::Enumerate;
use std::path::PathBuf;
use std::process;
use std::str::Lines;
use std::string::FromUtf8Error;

mod dep_tools;
#[macro_use] mod wrap_err;

use dep_tools::DepTool;
use dep_tools::FetchError;
use dep_tools::Git;

extern crate regex;

use regex::Regex;

fn main() {
    let deps_file_name = "dpnd.txt";
    let state_file_name = format!("current_{}", deps_file_name);
    let bad_dep_name_chars = Regex::new(r"[^a-zA-Z0-9._-]").unwrap();

    let install_result = install(
        &deps_file_name,
        &state_file_name,
        &bad_dep_name_chars,
    );
    if let Err(err) = install_result {
        print_install_error(err, deps_file_name);
        process::exit(1);
    }
}

fn install(
    deps_file_name: &str,
    state_file_name: &str,
    bad_dep_name_chars: &Regex,
)
    -> Result<(), InstallError<String>>
{
    let cwd = wrap_err!(
        env::current_dir(),
        InstallError::GetCurrentDirFailed,
    );

    let (deps_dir, deps_spec) = match read_deps_file(cwd, &deps_file_name) {
        Some(v) => v,
        None => return Err(InstallError::NoDepsFileFound),
    };

    let deps_spec = wrap_err!(
        String::from_utf8(deps_spec),
        InstallError::ConvDepsFileUtf8Failed,
    );

    let mut tools: HashMap<String, &dyn DepTool<String>> = HashMap::new();
    tools.insert("git".to_string(), &Git{});

    let conf = wrap_err!(
        parse_deps_conf(
            &deps_spec,
            state_file_name,
            bad_dep_name_chars,
            &tools,
        ),
        InstallError::ParseDepsConfFailed,
    );

    let output_dir = deps_dir.join(conf.output_dir);
    let state_file_path = output_dir.join(state_file_name);
    let state_file_conts = match fs::read(&state_file_path) {
        Ok(conts) => conts,
        // TODO Only ignore the underlying error if it's that the file doesn't
        // exist.
        Err(_) => vec![],
    };

    let state_spec = wrap_err!(
        String::from_utf8(state_file_conts),
        InstallError::ConvStateFileUtf8Failed,
        state_file_path,
    );

    let cur_deps =
        wrap_err!(
            parse_deps(
                &mut state_spec.lines().enumerate(),
                state_file_name,
                bad_dep_name_chars,
                &tools,
            ),
            InstallError::ParseStateFileFailed,
            state_file_path,
        );

    wrap_err!(
        fs::create_dir_all(&output_dir),
        InstallError::CreateMainOutputDirFailed,
        output_dir,
    );

    let state_file_path = output_dir.join(state_file_name);
    wrap_err!(
        install_deps(&output_dir, state_file_path, cur_deps, conf.deps),
        InstallError::InstallDepsError,
    );

    Ok(())
}

#[derive(Debug)]
enum InstallError<E> {
    GetCurrentDirFailed(IoError),
    NoDepsFileFound,
    ConvDepsFileUtf8Failed(FromUtf8Error),
    ParseDepsConfFailed(ParseDepsConfError),
    ConvStateFileUtf8Failed(FromUtf8Error, PathBuf),
    ParseStateFileFailed(ParseDepsError, PathBuf),
    CreateMainOutputDirFailed(IoError, PathBuf),
    InstallDepsError(InstallDepsError<E>),
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
    state_file_name: &str,
    bad_dep_name_chars: &Regex,
    tools: &HashMap<String, &'a (dyn DepTool<String> + 'a)>,
)
    -> Result<DepsConf<'a, String>, ParseDepsConfError>
{
    let mut lines = conts.lines().enumerate();

    if let Some(output_dir) = parse_output_dir(&mut lines) {
        Ok(DepsConf {
            output_dir,
            deps: wrap_err!(
                parse_deps(
                    &mut lines,
                    state_file_name,
                    bad_dep_name_chars,
                    &tools,
                ),
                ParseDepsConfError::ParseDepsFailed,
            ),
        })
    } else {
        Err(ParseDepsConfError::MissingOutputDir)
    }
}

#[derive(Debug)]
enum ParseDepsConfError {
    MissingOutputDir,
    ParseDepsFailed(ParseDepsError),
}

struct DepsConf<'a, E> {
    output_dir: PathBuf,
    deps: HashMap<String, Dependency<'a, E>>,
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
    state_file_name: &str,
    bad_dep_name_chars: &Regex,
    tools: &HashMap<String, &'a (dyn DepTool<String> + 'a)>,
)
    -> Result<HashMap<String, Dependency<'a, String>>, ParseDepsError>
{
    let mut dep_defns: Vec<(String, Dependency<'a, String>, usize)> = vec![];

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
        if let Some(found) = bad_dep_name_chars.find(&local_name) {
            return Err(ParseDepsError::DepNameContainsInvalidChar(
                ln_num,
                local_name.clone(),
                found.start(),
            ));
        } else if local_name == state_file_name {
            return Err(ParseDepsError::ReservedDepName(
                ln_num,
                local_name.clone(),
            ));
        }

        for (dep_local_name, _dep, defn_ln_num) in &dep_defns {
            if *dep_local_name == local_name {
                return Err(ParseDepsError::DupDepName(
                    ln_num,
                    local_name,
                    *defn_ln_num,
                ));
            }
        }

        let tool_name = words[1].to_string();
        let tool = match tools.get(&tool_name) {
            Some(tool) => *tool,
            None => return Err(ParseDepsError::UnknownTool(
                ln_num,
                local_name,
                tool_name,
            )),
        };

        dep_defns.push((
            local_name,
            Dependency {
                tool,
                source: words[2].to_string(),
                version: words[3].to_string(),
            },
            ln_num,
        ));
    }

    let deps =
        dep_defns.into_iter()
            .map(|(local_name, dep, _)| {
                (local_name, dep)
            })
            .collect();

    Ok(deps)
}

struct Dependency<'a, E> {
    tool: &'a (dyn DepTool<E> + 'a),
    source: String,
    version: String,
}

#[derive(Debug)]
enum ParseDepsError {
    DupDepName(usize, String, usize),
    DepNameContainsInvalidChar(usize, String, usize),
    ReservedDepName(usize, String),
    InvalidDepSpec(usize, String),
    UnknownTool(usize, String, String),
}

fn install_deps<'a>(
    output_dir: &PathBuf,
    state_file_path: PathBuf,
    mut cur_deps: HashMap<String, Dependency<'a, String>>,
    mut new_deps: HashMap<String, Dependency<'a, String>>,
)
    -> Result<(), InstallDepsError<String>>
{
    let mut actions = actions(&cur_deps, &new_deps);

    while let Some((act, dep_name)) = actions.pop() {
        let dir = output_dir.join(&dep_name);
        if let Err(err) = fs::remove_dir_all(&dir) {
            if err.kind() != ErrorKind::NotFound {
                return Err(InstallDepsError::RemoveOldDepOutputDirFailed(
                    err,
                    dep_name,
                    dir,
                ));
            }
        }
        cur_deps.remove(&dep_name);

        wrap_err!(
            write_state_file(&state_file_path, &cur_deps),
            InstallDepsError::WriteCurDepsAfterRemoveFailed,
            dep_name,
            state_file_path,
        );

        if act != Action::Install {
            continue;
        }

        let new_dep =
            new_deps.remove(&dep_name)
                .unwrap_or_else(|| panic!(
                    "dependency '{}' wasn't in the map of current \
                     dependencies",
                    dep_name,
                ));

        let dir = output_dir.join(&dep_name);
        wrap_err!(
            fs::create_dir(&dir),
            InstallDepsError::CreateDepOutputDirFailed,
            dep_name.clone(),
            dir,
        );

        wrap_err!(
            new_dep.tool.fetch(
                (&new_dep.source).to_string(),
                (&new_dep.version).to_string(),
                &dir,
            ),
            InstallDepsError::FetchFailed,
            dep_name.clone(),
        );
        cur_deps.insert(dep_name.clone(), new_dep);

        wrap_err!(
            write_state_file(&state_file_path, &cur_deps),
            InstallDepsError::WriteCurDepsAfterInstallFailed,
            dep_name.clone(),
            state_file_path,
        );
    }

    // We write the state file one final time in case there were no actions.
    wrap_err!(
        write_state_file(&state_file_path, &cur_deps),
        InstallDepsError::FinalWriteCurDepsFailed,
        state_file_path,
    );

    Ok(())
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
enum InstallDepsError<E> {
    RemoveOldDepOutputDirFailed(IoError, String, PathBuf),
    WriteCurDepsAfterRemoveFailed(WriteStateFileError, String, PathBuf),
    CreateDepOutputDirFailed(IoError, String, PathBuf),
    WriteCurDepsAfterInstallFailed(WriteStateFileError, String, PathBuf),
    FinalWriteCurDepsFailed(WriteStateFileError, PathBuf),
    FetchFailed(FetchError<E>, String),
}

// `actions` returns the actions that must be taken to transform `cur_deps`
// into `new_deps`.
fn actions<'a>(
    cur_deps: &HashMap<String, Dependency<'a, String>>,
    new_deps: &HashMap<String, Dependency<'a, String>>,
)
    -> Vec<(Action, String)>
{
    let mut actions = vec![];

    for (new_dep_name, new_dep) in new_deps {
        if let Some(cur_dep) = cur_deps.get(new_dep_name) {
            if cur_dep.tool.name() != new_dep.tool.name()
                    || cur_dep.source != new_dep.source
                    || cur_dep.version != new_dep.version {
                actions.push((Action::Install, new_dep_name.clone()));
            }
        } else {
            actions.push((Action::Install, new_dep_name.clone()));
        }
    }

    for cur_dep_name in cur_deps.keys() {
        if !new_deps.contains_key(cur_dep_name) {
            actions.push((Action::Remove, cur_dep_name.clone()));
        }
    }

    actions
}

#[derive(Debug, PartialEq)]
enum Action {
    Install,
    Remove,
}

fn write_state_file<'a>(
    state_file_path: &PathBuf,
    cur_deps: &HashMap<String, Dependency<'a, String>>,
)
    -> Result<(), WriteStateFileError>
{
    let mut file = wrap_err!(
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&state_file_path),
        WriteStateFileError::OpenFailed,
    );

    for (cur_dep_name, cur_dep) in cur_deps {
        wrap_err!(
            file.write(format!(
                "{} {} {} {}\n",
                cur_dep_name,
                cur_dep.tool.name(),
                cur_dep.source,
                cur_dep.version,
            ).as_bytes()),
            WriteStateFileError::WriteDepLineFailed,
        );
    }

    Ok(())
}

#[derive(Debug)]
enum WriteStateFileError {
    OpenFailed(IoError),
    WriteDepLineFailed(IoError),
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
            print_parse_deps_conf_error(err),
        InstallError::ConvStateFileUtf8Failed(err, state_file_path) =>
            eprintln!(
                "The state file ('{}') contains an invalid UTF-8 sequence \
                 after byte {}",
                render_path(&state_file_path),
                err.utf8_error().valid_up_to(),
            ),
        InstallError::ParseStateFileFailed(err, state_file_path) =>
            eprintln!(
                "The state file ('{}') is invalid ({}), please remove this \
                 file and try again",
                render_path(&state_file_path),
                render_parse_deps_error(err),
            ),
        InstallError::CreateMainOutputDirFailed(io_err, path) =>
            eprintln!(
                "Couldn't create {}, the main output directory: {}",
                render_path(&path),
                io_err,
            ),
        InstallError::InstallDepsError(err) =>
            print_install_deps_error(err),
    }
}

fn print_install_deps_error(err: InstallDepsError<String>) {
    match err {
        InstallDepsError::RemoveOldDepOutputDirFailed(io_err, dep, path) =>
            eprintln!(
                "Couldn't remove {}, the output directory for the '{}' \
                 dependency: {}",
                render_path(&path),
                dep,
                io_err,
            ),
        InstallDepsError::WriteCurDepsAfterRemoveFailed(
            err,
            dep,
            state_file_path
        ) =>
            print_write_cur_deps_err(
                err,
                &state_file_path,
                &format!("removing '{}'", dep),
            ),
        InstallDepsError::CreateDepOutputDirFailed(io_err, dep, path) =>
            eprintln!(
                "Couldn't create {}, the output directory for the '{}' \
                 dependency: {}",
                render_path(&path),
                dep,
                io_err,
            ),
        InstallDepsError::WriteCurDepsAfterInstallFailed(
            err,
            dep,
            state_file_path,
        ) =>
            print_write_cur_deps_err(
                err,
                &state_file_path,
                &format!("installing '{}'", dep),
            ),
        InstallDepsError::FinalWriteCurDepsFailed(
            err,
            state_file_path,
        ) =>
            print_write_cur_deps_err(
                err,
                &state_file_path,
                "updating dependencies",
            ),
        InstallDepsError::FetchFailed(err, dep_name) =>
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

fn print_parse_deps_conf_error(err: ParseDepsConfError) {
    match err {
        ParseDepsConfError::MissingOutputDir =>
            eprintln!(
                "The dependency file doesn't contain an output directory"
            ),
        ParseDepsConfError::ParseDepsFailed(err) =>
            eprintln!("{}", render_parse_deps_error(err)),
    }
}

fn render_parse_deps_error(err: ParseDepsError) -> String {
    match err {
        ParseDepsError::DupDepName(ln_num, dep, orig_ln_num) => {
            format!(
                "Line {}: A dependency named '{}' is already defined on line \
                 {}",
                ln_num,
                dep,
                orig_ln_num,
            )
        },
        ParseDepsError::ReservedDepName(ln_num, dep) => {
            format!(
                "Line {}: '{}' is a reserved name and can't be used as a \
                 dependency name",
                ln_num,
                dep,
            )
        },
        ParseDepsError::DepNameContainsInvalidChar(
            ln_num,
            dep,
            bad_char_idx,
        ) => {
            let mut bad_char = "".to_string();
            if let Some(chr) = dep.chars().nth(bad_char_idx) {
                bad_char = format!(" ('{}')", chr);
            }
            format!(
                "Line {}: '{}' contains an invalid character{} at position \
                 {}; dependency names can only contain numbers, letters, \
                 hyphens, underscores and periods",
                ln_num,
                dep,
                bad_char,
                bad_char_idx + 1,
            )
        },
        ParseDepsError::InvalidDepSpec(ln_num, ln) => {
            format!(
                "Line {}: Invalid dependency specification: '{}'",
                ln_num,
                ln,
            )
        },
        ParseDepsError::UnknownTool(
            ln_num,
            dep_name,
            tool_name,
        ) => {
            format!(
                "Line {}: The '{}' dependency specifies an invalid tool name \
                 ('{}'); the supported tool is 'git'",
                ln_num,
                dep_name,
                tool_name,
            )
        },
    }
}

fn print_write_cur_deps_err(
    err: WriteStateFileError,
    state_file_path: &PathBuf,
    action: &str,
) {
    match err {
        WriteStateFileError::OpenFailed(io_err) =>
            eprintln!(
                "Couldn't open the state file ('{}') for writing after {}: {}",
                render_path(state_file_path),
                action,
                io_err,
            ),
        WriteStateFileError::WriteDepLineFailed(io_err) =>
            eprintln!(
                "Couldn't write to the state file ('{}') after {}: {}",
                render_path(state_file_path),
                action,
                io_err,
            ),
    }
}

fn render_path(path: &PathBuf) -> String {
    if let Some(s) = path.to_str() {
        format!("'{}'", s)
    } else {
        format!("{:?}", path)
    }
}
