// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Write;
use std::iter::Enumerate;
use std::path::PathBuf;
use std::process;
use std::str;
use std::str::Lines;
use std::string::FromUtf8Error;

mod dep_tools;

use dep_tools::DepTool;
use dep_tools::FetchError;
use dep_tools::Git;
use dep_tools::GitCmdError;

extern crate regex;
extern crate snafu;

use regex::Regex;
use snafu::ResultExt;
use snafu::Snafu;

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
    -> Result<(), InstallError<GitCmdError>>
{
    let cwd = env::current_dir()
        .context(GetCurrentDirFailed{})?;

    let (deps_dir, deps_spec) = match read_deps_file(cwd, &deps_file_name) {
        Some(v) => v,
        None => return Err(InstallError::NoDepsFileFound),
    };

    let deps_spec = String::from_utf8(deps_spec)
        .context(ConvDepsFileUtf8Failed{})?;

    let mut tools: HashMap<String, &dyn DepTool<GitCmdError>> = HashMap::new();
    tools.insert("git".to_string(), &Git{});

    let conf = parse_deps_conf(
        &deps_spec,
        state_file_name,
        bad_dep_name_chars,
        &tools,
    )
        .context(ParseDepsConfFailed{})?;

    let output_dir = deps_dir.join(conf.output_dir);
    let state_file_path = output_dir.join(state_file_name);
    let state_file_conts = match fs::read(&state_file_path) {
        Ok(conts) => {
            conts
        },
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                vec![]
            } else {
                return Err(InstallError::ReadStateFileFailed{
                    source: err,
                    path: state_file_path,
                });
            }
        },
    };

    let state_spec = String::from_utf8(state_file_conts)
        .with_context(
            || ConvStateFileUtf8Failed{path: state_file_path.clone()}
        )?;

    let cur_deps = parse_deps(
        &mut state_spec.lines().enumerate(),
        state_file_name,
        bad_dep_name_chars,
        &tools,
    )
        .with_context(|| ParseStateFileFailed{path: state_file_path.clone()})?;

    fs::create_dir_all(&output_dir)
        .with_context(|| CreateMainOutputDirFailed{path: output_dir.clone()})?;

    let state_file_path = output_dir.join(state_file_name);
    install_deps(&output_dir, state_file_path, cur_deps, conf.deps)
        .context(InstallDepsFailed{})?;

    Ok(())
}

#[derive(Debug, Snafu)]
enum InstallError<E>
where
    E: Error + 'static
{
    GetCurrentDirFailed{source: IoError},
    NoDepsFileFound,
    ConvDepsFileUtf8Failed{source: FromUtf8Error},
    ParseDepsConfFailed{source: ParseDepsConfError},
    ReadStateFileFailed{source: IoError, path: PathBuf},
    ConvStateFileUtf8Failed{source: FromUtf8Error, path: PathBuf},
    ParseStateFileFailed{source: ParseDepsError, path: PathBuf},
    CreateMainOutputDirFailed{source: IoError, path: PathBuf},
    InstallDepsFailed{source: InstallDepsError<E>},
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
    tools: &HashMap<String, &'a (dyn DepTool<GitCmdError> + 'a)>,
)
    -> Result<DepsConf<'a, GitCmdError>, ParseDepsConfError>
{
    let mut lines = conts.lines().enumerate();

    if let Some(output_dir) = parse_output_dir(&mut lines) {
        Ok(DepsConf {
            output_dir,
            deps: parse_deps(
                &mut lines,
                state_file_name,
                bad_dep_name_chars,
                &tools,
            )
                .context(ParseDepsFailed{})?,
        })
    } else {
        Err(ParseDepsConfError::MissingOutputDir)
    }
}

#[derive(Debug, Snafu)]
enum ParseDepsConfError {
    MissingOutputDir,
    ParseDepsFailed{source: ParseDepsError},
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
    tools: &HashMap<String, &'a (dyn DepTool<GitCmdError> + 'a)>,
)
    -> Result<HashMap<String, Dependency<'a, GitCmdError>>, ParseDepsError>
{
    let mut dep_defns: Vec<(String, Dependency<'a, GitCmdError>, usize)> =
        vec![];

    for (i, line) in lines {
        let ln_num = i + 1;

        let ln = line.trim_start();
        if conf_line_is_skippable(ln) {
            continue;
        }

        let words: Vec<&str> = ln.split_ascii_whitespace().collect();
        if words.len() != 4 {
            return Err(ParseDepsError::InvalidDepSpec{
                ln_num,
                line: ln.to_string(),
            });
        }

        let local_name = words[0].to_string();
        if let Some(found) = bad_dep_name_chars.find(&local_name) {
            return Err(ParseDepsError::DepNameContainsInvalidChar{
                ln_num,
                dep_name: local_name.clone(),
                bad_char_idx: found.start(),
            });
        } else if local_name == state_file_name {
            return Err(ParseDepsError::ReservedDepName{
                ln_num,
                dep_name: local_name.clone(),
            });
        }

        for (dep_local_name, _dep, defn_ln_num) in &dep_defns {
            if *dep_local_name == local_name {
                return Err(ParseDepsError::DupDepName{
                    ln_num,
                    dep_name: local_name,
                    orig_ln_num: *defn_ln_num,
                });
            }
        }

        let tool_name = words[1].to_string();
        let tool = match tools.get(&tool_name) {
            Some(tool) => *tool,
            None => return Err(ParseDepsError::UnknownTool{
                ln_num,
                dep_name: local_name,
                tool_name,
            }),
        };

        dep_defns.push((
            local_name,
            Dependency{
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

#[derive(Debug, Snafu)]
enum ParseDepsError {
    DupDepName{ln_num: usize, dep_name: String, orig_ln_num: usize},
    DepNameContainsInvalidChar{
        ln_num: usize,
        dep_name: String,
        bad_char_idx: usize,
    },
    ReservedDepName{ln_num: usize, dep_name: String},
    InvalidDepSpec{ln_num: usize, line: String},
    UnknownTool{ln_num: usize, dep_name: String, tool_name: String},
}

fn install_deps<'a>(
    output_dir: &PathBuf,
    state_file_path: PathBuf,
    mut cur_deps: HashMap<String, Dependency<'a, GitCmdError>>,
    mut new_deps: HashMap<String, Dependency<'a, GitCmdError>>,
)
    -> Result<(), InstallDepsError<GitCmdError>>
{
    let mut actions = actions(&cur_deps, &new_deps);

    while let Some((act, dep_name)) = actions.pop() {
        let dir = output_dir.join(&dep_name);
        if let Err(source) = fs::remove_dir_all(&dir) {
            if source.kind() != ErrorKind::NotFound {
                return Err(InstallDepsError::RemoveOldDepOutputDirFailed{
                    source,
                    dep_name,
                    path: dir,
                });
            }
        }
        cur_deps.remove(&dep_name);

        write_state_file(&state_file_path, &cur_deps)
            .with_context(|| WriteCurDepsAfterRemoveFailed{
                dep_name: dep_name.clone(),
                state_file_path: state_file_path.clone(),
            })?;

        if act != Action::Install {
            continue;
        }

        let new_dep = new_deps.remove(&dep_name)
            .unwrap_or_else(|| panic!(
                "dependency '{}' wasn't in the map of current \
                 dependencies",
                dep_name,
            ));

        let dir = output_dir.join(&dep_name);
        fs::create_dir(&dir)
            .context(CreateDepOutputDirFailed{
                dep_name: dep_name.clone(),
                path: &dir,
            })?;

        new_dep.tool.fetch(
            (&new_dep.source).to_string(),
            (&new_dep.version).to_string(),
            &dir,
        )
            .context(FetchFailed{dep_name: dep_name.clone()})?;
        cur_deps.insert(dep_name.clone(), new_dep);

        write_state_file(&state_file_path, &cur_deps)
            .with_context(|| WriteCurDepsAfterInstallFailed{
                dep_name: dep_name.clone(),
                state_file_path: state_file_path.clone(),
            })?;
    }

    // We write the state file one final time in case there were no actions.
    write_state_file(&state_file_path, &cur_deps)
        .context(FinalWriteCurDepsFailed{state_file_path})?;

    Ok(())
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Snafu)]
enum InstallDepsError<E>
where
    E: Error + 'static
{
    RemoveOldDepOutputDirFailed{
        source: IoError,
        dep_name: String,
        path: PathBuf,
    },
    WriteCurDepsAfterRemoveFailed{
        source: WriteStateFileError,
        dep_name: String,
        state_file_path: PathBuf,
    },
    CreateDepOutputDirFailed{source: IoError, dep_name: String, path: PathBuf},
    WriteCurDepsAfterInstallFailed{
        source: WriteStateFileError,
        dep_name: String,
        state_file_path: PathBuf,
    },
    FinalWriteCurDepsFailed{
        source: WriteStateFileError,
        state_file_path: PathBuf,
    },
    FetchFailed{source: FetchError<E>, dep_name: String},
}

// `actions` returns the actions that must be taken to transform `cur_deps`
// into `new_deps`.
fn actions<'a>(
    cur_deps: &HashMap<String, Dependency<'a, GitCmdError>>,
    new_deps: &HashMap<String, Dependency<'a, GitCmdError>>,
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
    cur_deps: &HashMap<String, Dependency<'a, GitCmdError>>,
)
    -> Result<(), WriteStateFileError>
{
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&state_file_path)
        .context(OpenFailed)?;

    for (cur_dep_name, cur_dep) in cur_deps {
        file.write(format!(
            "{} {} {} {}\n",
            cur_dep_name,
            cur_dep.tool.name(),
            cur_dep.source,
            cur_dep.version,
        ).as_bytes())
            .context(WriteDepLineFailed)?;
    }

    Ok(())
}

#[derive(Debug, Snafu)]
enum WriteStateFileError {
    OpenFailed{source: IoError},
    WriteDepLineFailed{source: IoError},
}

fn print_install_error(err: InstallError<GitCmdError>, deps_file_name: &str) {
    match err {
        InstallError::GetCurrentDirFailed{source} =>
            eprintln!("Couldn't get the current directory: {}", source),
        InstallError::NoDepsFileFound =>
            eprintln!(
                "Couldn't find the dependency file '{}' in the current \
                 directory or parent directories",
                deps_file_name,
            ),
        InstallError::ConvDepsFileUtf8Failed{source} =>
            eprintln!(
                "The dependency file contains an invalid UTF-8 sequence after \
                 byte {}",
                source.utf8_error().valid_up_to(),
            ),
        InstallError::ParseDepsConfFailed{source} =>
            print_parse_deps_conf_error(source),
        InstallError::ReadStateFileFailed{source, path} =>
            eprintln!(
                "Couldn't read the state file ('{}'): {}",
                render_path(&path),
                source,
            ),
        InstallError::ConvStateFileUtf8Failed{source, path} =>
            eprintln!(
                "The state file ('{}') contains an invalid UTF-8 sequence \
                 after byte {}",
                render_path(&path),
                source.utf8_error().valid_up_to(),
            ),
        InstallError::ParseStateFileFailed{source, path} =>
            eprintln!(
                "The state file ('{}') is invalid ({}), please remove this \
                 file and try again",
                render_path(&path),
                render_parse_deps_error(source),
            ),
        InstallError::CreateMainOutputDirFailed{source, path} =>
            eprintln!(
                "Couldn't create {}, the main output directory: {}",
                render_path(&path),
                source,
            ),
        InstallError::InstallDepsFailed{source} =>
            print_install_deps_error(source),
    }
}

fn print_install_deps_error(err: InstallDepsError<GitCmdError>) {
    match err {
        InstallDepsError::RemoveOldDepOutputDirFailed{
            source,
            dep_name,
            path,
        } =>
            eprintln!(
                "Couldn't remove '{}', the output directory for the '{}' \
                 dependency: {}",
                render_path(&path),
                dep_name,
                source,
            ),
        InstallDepsError::WriteCurDepsAfterRemoveFailed{
            source,
            dep_name,
            state_file_path,
        } =>
            print_write_cur_deps_err(
                source,
                &state_file_path,
                &format!("removing '{}'", dep_name),
            ),
        InstallDepsError::CreateDepOutputDirFailed{source, dep_name, path} =>
            eprintln!(
                "Couldn't create {}, the output directory for the '{}' \
                 dependency: {}",
                render_path(&path),
                dep_name,
                source,
            ),
        InstallDepsError::WriteCurDepsAfterInstallFailed{
            source,
            dep_name,
            state_file_path,
        } =>
            print_write_cur_deps_err(
                source,
                &state_file_path,
                &format!("installing '{}'", dep_name),
            ),
        InstallDepsError::FinalWriteCurDepsFailed{source, state_file_path} =>
            print_write_cur_deps_err(
                source,
                &state_file_path,
                "updating dependencies",
            ),
        InstallDepsError::FetchFailed{source, dep_name} =>
            match source {
                FetchError::RetrieveFailed{source} =>
                    eprintln!(
                        "Couldn't retrieve the source for the '{}' \
                         dependency: {}",
                        dep_name,
                        render_git_cmd_err(source),
                    ),
                FetchError::VersionChangeFailed{source} =>
                    eprintln!(
                        "Couldn't change the version for the '{}' dependency: \
                         {}",
                        dep_name,
                        render_git_cmd_err(source),
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
        ParseDepsConfError::ParseDepsFailed{source} =>
            eprintln!("{}", render_parse_deps_error(source)),
    }
}

fn render_parse_deps_error(err: ParseDepsError) -> String {
    match err {
        ParseDepsError::DupDepName{ln_num, dep_name, orig_ln_num} => {
            format!(
                "Line {}: A dependency named '{}' is already defined on line \
                 {}",
                ln_num,
                dep_name,
                orig_ln_num,
            )
        },
        ParseDepsError::ReservedDepName{ln_num, dep_name} => {
            format!(
                "Line {}: '{}' is a reserved name and can't be used as a \
                 dependency name",
                ln_num,
                dep_name,
            )
        },
        ParseDepsError::DepNameContainsInvalidChar{
            ln_num,
            dep_name,
            bad_char_idx,
        } => {
            let mut bad_char = "".to_string();
            if let Some(chr) = dep_name.chars().nth(bad_char_idx) {
                bad_char = format!(" ('{}')", chr);
            }
            format!(
                "Line {}: '{}' contains an invalid character{} at position \
                 {}; dependency names can only contain numbers, letters, \
                 hyphens, underscores and periods",
                ln_num,
                dep_name,
                bad_char,
                bad_char_idx + 1,
            )
        },
        ParseDepsError::InvalidDepSpec{ln_num, line} => {
            format!(
                "Line {}: Invalid dependency specification: '{}'",
                ln_num,
                line,
            )
        },
        ParseDepsError::UnknownTool{ln_num, dep_name, tool_name} => {
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
        WriteStateFileError::OpenFailed{source} =>
            eprintln!(
                "Couldn't open the state file ('{}') for writing after {}: {}",
                render_path(state_file_path),
                action,
                source,
            ),
        WriteStateFileError::WriteDepLineFailed{source} =>
            eprintln!(
                "Couldn't write to the state file ('{}') after {}: {}",
                render_path(state_file_path),
                action,
                source,
            ),
    }
}

fn render_path(path: &PathBuf) -> String {
    if let Some(s) = path.to_str() {
        s.to_string()
    } else {
        format!("{:?}", path)
    }
}

fn render_git_cmd_err(err: GitCmdError) -> String {
    match err {
        GitCmdError::StartFailed{source, args} => {
            format!("couldn't start `git {}`: {}", args.join(" "), source)
        },
        GitCmdError::NotSuccess{args, output} => {
            let render_output = |bytes, name, prefix| {
                if let Ok(s) = str::from_utf8(bytes) {
                    prefix_lines(s, prefix)
                } else {
                    format!("{} (not UTF-8): {:?}", name, bytes)
                }
            };

            format!(
                "`git {}` failed with the following output:\n\n{}{}",
                args.join(" "),
                render_output(&output.stdout, "STDOUT", "[>] "),
                render_output(&output.stderr, "STDERR", "[!] "),
            )
        },
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
