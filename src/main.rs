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
use std::path::Path;
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
use dep_tools::Version;

extern crate clap;
extern crate regex;
extern crate snafu;

use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::SubCommand;
use regex::Regex;
use snafu::ResultExt;
use snafu::Snafu;

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
            let mut tools: HashMap<String, &dyn DepTool<GitCmdError>> =
                HashMap::new();
            tools.insert("git".to_string(), &Git{});

            let bad_dep_name_chars = Regex::new(r"[^a-zA-Z0-9._-]").unwrap();
            let install_result = install(
                &Installer{
                    deps_file_name: deps_file_name.to_string(),
                    state_file_name: format!("current_{}", deps_file_name),
                    bad_dep_name_chars,
                    tools,
                },
                sub_args.is_present(install_recursive_flag),
            );
            if let Err(err) = install_result {
                print_install_error(err, &deps_file_name);
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

struct Installer<'a, E> {
    deps_file_name: String,
    state_file_name: String,
    bad_dep_name_chars: Regex,
    tools: HashMap<String, &'a (dyn DepTool<E> + 'a)>,
}

fn install(installer: &Installer<GitCmdError>, recurse: bool)
    -> Result<(), InstallError<GitCmdError>>
{
    let cwd = env::current_dir()
        .context(GetCurrentDirFailed{})?;

    let (proj_dir, deps_file_path, raw_deps_spec) =
        match read_deps_file(cwd, &installer.deps_file_name) {
            Some(v) => v,
            None => return Err(InstallError::NoDepsFileFound),
        };

    let mut projs = vec![(proj_dir, None, deps_file_path, raw_deps_spec)];

    while let Some(proj) = projs.pop() {
        let (proj_dir, dep_name, deps_file_path, raw_deps_spec) = proj;
        let deps_spec = String::from_utf8(raw_deps_spec)
            .with_context(|| ConvDepsFileUtf8Failed{
                dep_name: dep_name.clone(),
                path: deps_file_path.clone(),
            })?;

        let conf = parse_deps_conf(&installer, &deps_spec)
            .context(ParseDepsConfFailed{dep_name, path: deps_file_path})?;

        install_proj_deps(&installer, &proj_dir, &conf)
            .context(InstallProjDepsFailed{})?;

        if !recurse {
            break;
        }

        for dep_name in conf.deps.keys() {
            let dep_proj_path = proj_dir.join(&conf.output_dir).join(dep_name);
            let dep_deps_file_path =
                dep_proj_path.join(&installer.deps_file_name);
            let maybe_raw_deps_spec = try_read(&dep_deps_file_path)
                .with_context(|| ReadNestedDepsFileFailed{
                    path: dep_deps_file_path.clone(),
                    dep_name,
                    dep_proj_path: dep_proj_path.clone(),
                })?;

            if let Some(raw_deps_spec) = maybe_raw_deps_spec {
                projs.push((
                    dep_proj_path,
                    Some(dep_name.to_string()),
                    dep_deps_file_path,
                    raw_deps_spec,
                ));
            }
        }
    }

    Ok(())
}

fn try_read<P: AsRef<Path>>(path: P) -> Result<Option<Vec<u8>>, IoError> {
    match fs::read(path) {
        Ok(conts) => {
            Ok(Some(conts))
        },
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(err)
            }
        },
    }
}

#[derive(Debug, Snafu)]
enum InstallError<E>
where
    E: Error + 'static
{
    GetCurrentDirFailed{source: IoError},
    NoDepsFileFound,
    ConvDepsFileUtf8Failed{
        source: FromUtf8Error,
        path: PathBuf,
        dep_name: Option<String>,
    },
    ParseDepsConfFailed{
        source: ParseDepsConfError,
        path: PathBuf,
        dep_name: Option<String>,
    },
    InstallProjDepsFailed{source: InstallProjDepsError<E>},
    ReadNestedDepsFileFailed{
        source: IoError,
        path: PathBuf,
        dep_name: String,
        dep_proj_path: PathBuf,
    },
}

fn install_proj_deps<'a>(
    installer: &Installer<'a, GitCmdError>,
    proj_dir: &PathBuf,
    conf: &DepsConf<'a, GitCmdError>,
) -> Result<(), InstallProjDepsError<GitCmdError>> {
    let output_dir = proj_dir.join(&conf.output_dir);
    let state_file_path = output_dir.join(&installer.state_file_name);
    let state_file_conts = match fs::read(&state_file_path) {
        Ok(conts) => {
            conts
        },
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                vec![]
            } else {
                return Err(InstallProjDepsError::ReadStateFileFailed{
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

    let cur_deps = parse_deps(&installer, &mut state_spec.lines().enumerate())
        .with_context(|| ParseStateFileFailed{path: state_file_path.clone()})?;

    fs::create_dir_all(&output_dir)
        .with_context(|| CreateMainOutputDirFailed{path: output_dir.clone()})?;

    install_deps(&output_dir, state_file_path, cur_deps, conf.deps.clone())
        .context(InstallDepsFailed{})?;

    Ok(())
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Snafu)]
enum InstallProjDepsError<E>
where
    E: Error + 'static
{
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
    -> Option<(PathBuf, PathBuf, Vec<u8>)>
{
    let mut dir = start;
    loop {
        let deps_file_path = dir.clone().join(deps_file_name);
        if let Ok(conts) = fs::read(&deps_file_path) {
            return Some((dir, deps_file_path, conts));
        }

        if !dir.pop() {
            return None;
        }
    }
}

fn parse_deps_conf<'a>(installer: &Installer<'a, GitCmdError>, conts: &str)
    -> Result<DepsConf<'a, GitCmdError>, ParseDepsConfError>
{
    let mut lines = conts.lines().enumerate();

    if let Some(output_dir) = parse_output_dir(&mut lines) {
        Ok(DepsConf {
            output_dir,
            deps: parse_deps(&installer, &mut lines)
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
    installer: &Installer<'a, GitCmdError>,
    lines: &mut Enumerate<Lines>,
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
        if let Some(found) = installer.bad_dep_name_chars.find(&local_name) {
            return Err(ParseDepsError::DepNameContainsInvalidChar{
                ln_num,
                dep_name: local_name.clone(),
                bad_char_idx: found.start(),
            });
        } else if local_name == installer.state_file_name {
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
        let tool = match installer.tools.get(&tool_name) {
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
                version: Version(words[3].to_string()),
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
    version: Version,
}

impl<'a, E> Clone for Dependency<'a, E> {
    fn clone(&self) -> Self {
        Dependency{
            tool: self.tool,
            source: self.source.clone(),
            version: self.version.clone(),
        }
    }
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
                "dependency '{}' wasn't in the map of current dependencies",
                dep_name,
            ));

        let dir = output_dir.join(&dep_name);
        fs::create_dir(&dir)
            .context(CreateDepOutputDirFailed{
                dep_name: dep_name.clone(),
                path: &dir,
            })?;

        new_dep.tool.fetch(
            new_dep.source.clone(),
            new_dep.version.clone(),
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
        InstallError::ConvDepsFileUtf8Failed{source, path, dep_name} =>
            if let Some(name) = dep_name {
                eprintln!(
                    "{}: This nested dependency file (for '{}') contains an \
                     invalid UTF-8 sequence after byte {}",
                    render_path(&path),
                    source.utf8_error().valid_up_to(),
                    name,
                )
            } else {
                eprintln!(
                    "{}: This dependency file contains an invalid UTF-8 \
                     sequence after byte {}",
                    render_path(&path),
                    source.utf8_error().valid_up_to(),
                )
            },
        InstallError::ParseDepsConfFailed{source, path, dep_name} =>
            print_parse_deps_conf_error(source, &path, dep_name),
        InstallError::InstallProjDepsFailed{source} =>
            print_install_proj_deps_error(source),
        InstallError::ReadNestedDepsFileFailed{
            source,
            path,
            dep_name,
            dep_proj_path,
        } =>
            eprintln!(
                "Couldn't read the dependency file ('{}') for the nested \
                 dependency '{}' ('{}'): {}",
                render_path(&path),
                dep_name,
                render_path(&dep_proj_path),
                source,
            ),
    }
}

fn print_install_proj_deps_error(err: InstallProjDepsError<GitCmdError>) {
    match err {
        InstallProjDepsError::ReadStateFileFailed{source, path} =>
            eprintln!(
                "Couldn't read the state file ('{}'): {}",
                render_path(&path),
                source,
            ),
        InstallProjDepsError::ConvStateFileUtf8Failed{source, path} =>
            eprintln!(
                "The state file ('{}') contains an invalid UTF-8 sequence \
                 after byte {}",
                render_path(&path),
                source.utf8_error().valid_up_to(),
            ),
        InstallProjDepsError::ParseStateFileFailed{source, path} =>
            eprintln!(
                "The state file ('{}') is invalid ({}), please remove this \
                 file and try again",
                render_path(&path),
                render_parse_deps_error(source),
            ),
        InstallProjDepsError::CreateMainOutputDirFailed{source, path} =>
            eprintln!(
                "Couldn't create {}, the main output directory: {}",
                render_path(&path),
                source,
            ),
        InstallProjDepsError::InstallDepsFailed{source} =>
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
                "Couldn't create '{}', the output directory for the '{}' \
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

fn print_parse_deps_conf_error(
    err: ParseDepsConfError,
    dep_file_path: &PathBuf,
    dep_name: Option<String>,
) {
    match err {
        ParseDepsConfError::MissingOutputDir =>
            if let Some(name) = dep_name {
                eprintln!(
                    "{}: This nested dependency file (for '{}') doesn't \
                     contain an output directory",
                    render_path(&dep_file_path),
                    name,
                )
            } else {
                eprintln!(
                    "{}: This dependency file doesn't contain an output \
                     directory",
                    render_path(&dep_file_path),
                )
            },
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
