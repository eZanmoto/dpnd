// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Write;
use std::iter::Enumerate;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::str::Lines;
use std::string::FromUtf8Error;

use dep_tools::DepTool;
use dep_tools::FetchError;
use dep_tools::GitCmdError;
use dep_tools::Version;

use regex::Regex;
use snafu::ResultExt;
use snafu::Snafu;

pub struct Installer<'a, E> {
    pub deps_file_name: String,
    pub state_file_name: String,
    pub bad_dep_name_chars: Regex,
    pub tools: HashMap<String, &'a (dyn DepTool<E> + 'a)>,
}

impl<'a> Installer<'a, GitCmdError> {
    pub fn install(&self, cwd: &PathBuf, recurse: bool)
        -> Result<(), InstallError<GitCmdError>>
    {
        let (proj_dir, deps_file_path, raw_deps_spec) =
            match read_deps_file(&cwd, &self.deps_file_name) {
                Ok(maybe_v) => {
                    if let Some(v) = maybe_v {
                        v
                    } else {
                        return Err(InstallError::NoDepsFileFound);
                    }
                },
                Err(err) => {
                    return Err(InstallError::ReadDepsFileFailed{source: err});
                },
            };

        let mut projs = vec![(proj_dir, None, deps_file_path, raw_deps_spec)];

        while let Some(proj) = projs.pop() {
            let (proj_dir, dep_name, deps_file_path, raw_deps_spec) = proj;
            let deps_spec = String::from_utf8(raw_deps_spec)
                .with_context(|| ConvDepsFileUtf8Failed{
                    dep_name: dep_name.clone(),
                    path: deps_file_path.clone(),
                })?;

            let conf = &self.parse_deps_conf(&deps_spec)
                .with_context(|| ParseDepsConfFailed{
                    dep_name: dep_name.clone(),
                    path: deps_file_path.clone(),
                })?;

            self.install_proj_deps(&proj_dir, &conf)
                .context(InstallProjDepsFailed{dep_name})?;

            if !recurse {
                break;
            }

            for dep_name in conf.deps.keys() {
                let dep_proj_path =
                    proj_dir.join(&conf.output_dir).join(dep_name);
                let dep_deps_file_path =
                    dep_proj_path.join(&self.deps_file_name);
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

    fn install_proj_deps<'b>(
        &self,
        proj_dir: &PathBuf,
        conf: &DepsConf<'b, GitCmdError>,
    ) -> Result<(), InstallProjDepsError<GitCmdError>> {
        let output_dir = proj_dir.join(&conf.output_dir);
        let state_file_path = output_dir.join(&self.state_file_name);
        let (state_file_exists, state_file_conts) =
            match try_read(&state_file_path) {
                Ok(maybe_conts) => {
                    if let Some(conts) = maybe_conts {
                        (true, conts)
                    } else {
                        (false, vec![])
                    }
                },
                Err(err) => {
                    return Err(InstallProjDepsError::ReadStateFileFailed{
                        source: err,
                        path: state_file_path,
                    });
                },
            };

        let state_spec = String::from_utf8(state_file_conts)
            .with_context(
                || ConvStateFileUtf8Failed{path: state_file_path.clone()}
            )?;

        let cur_deps = self.parse_deps(&mut state_spec.lines().enumerate())
            .with_context(||
                ParseStateFileFailed{path: state_file_path.clone()}
            )?;

        fs::create_dir_all(&output_dir)
            .with_context(||
                CreateMainOutputDirFailed{path: output_dir.clone()}
            )?;

        install_deps(
            &output_dir,
            state_file_path,
            state_file_exists,
            cur_deps,
            conf.deps.clone(),
        )
            .context(InstallDepsFailed{})?;

        Ok(())
    }

    fn parse_deps_conf(&self, conts: &str)
        -> Result<DepsConf<'a, GitCmdError>, ParseDepsConfError>
    {
        let mut lines = conts.lines().enumerate();

        let output_dir = parse_output_dir(&mut lines)
            .context(ParseOutputDirFailed{})?;

        let deps = self.parse_deps(&mut lines)
            .context(ParseDepsFailed{})?;

        Ok(DepsConf{output_dir, deps})
    }

    fn parse_deps(&self, lines: &mut Enumerate<Lines>)
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
            if let Some(found) = self.bad_dep_name_chars.find(&local_name) {
                return Err(ParseDepsError::DepNameContainsInvalidChar{
                    ln_num,
                    dep_name: local_name.clone(),
                    bad_char_idx: found.start(),
                });
            } else if local_name == self.state_file_name {
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
            let tool = match self.tools.get(&tool_name) {
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
}

#[derive(Debug, Snafu)]
pub enum InstallError<E>
where
    E: Error + 'static
{
    NoDepsFileFound,
    ReadDepsFileFailed{source: ReadDepsFileError},
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
    InstallProjDepsFailed{
        source: InstallProjDepsError<E>,
        dep_name: Option<String>,
    },
    ReadNestedDepsFileFailed{
        source: IoError,
        path: PathBuf,
        dep_name: String,
        dep_proj_path: PathBuf,
    },
}

// `try_read` returns the contents of the file at `path`, or `None` if it
// doesn't exist, or an error if one occurred.
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

#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Snafu)]
pub enum InstallProjDepsError<E>
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
fn read_deps_file(start: &PathBuf, deps_file_name: &str)
    -> Result<Option<(PathBuf, PathBuf, Vec<u8>)>, ReadDepsFileError>
{
    let mut dir = start.to_path_buf();
    loop {
        let deps_file_path = dir.clone().join(deps_file_name);

        match try_read(&deps_file_path) {
            Ok(Some(conts)) => {
                return Ok(Some((dir, deps_file_path, conts)));
            },
            Ok(None) => {
            },
            Err(err) => {
                return Err(ReadDepsFileError::ReadFailed{
                    source: err,
                    deps_file_path,
                });
            },
        }

        if !dir.pop() {
            return Ok(None);
        }
    }
}

#[derive(Debug, Snafu)]
pub enum ReadDepsFileError {
    ReadFailed{source: IoError, deps_file_path: PathBuf},
}

#[derive(Debug, Snafu)]
pub enum ParseDepsConfError {
    ParseOutputDirFailed{source: ParseOutputDirError},
    ParseDepsFailed{source: ParseDepsError},
}

struct DepsConf<'a, E> {
    output_dir: PathBuf,
    deps: HashMap<String, Dependency<'a, E>>,
}

fn parse_output_dir(lines: &mut Enumerate<Lines>)
    -> Result<PathBuf, ParseOutputDirError>
{
    for (i, line) in lines {
        let ln = line.trim_start();
        if !conf_line_is_skippable(ln) {
            let mut path = PathBuf::new();
            for part in ln.split('/') {
                if part == "." || part == ".." {
                    return Err(ParseOutputDirError::InvalidPart{
                        ln_num: i + 1,
                        part: part.to_string(),
                    });
                }
                path.push(part);
            }
            return Ok(path);
        }
    }

    Err(ParseOutputDirError::MissingOutputDir)
}

fn conf_line_is_skippable(ln: &str) -> bool {
    ln.is_empty() || ln.starts_with('#')
}

#[derive(Debug, Snafu)]
pub enum ParseOutputDirError {
    MissingOutputDir,
    InvalidPart{ln_num: usize, part: String},
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
pub enum ParseDepsError {
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
    state_file_exists: bool,
    mut cur_deps: HashMap<String, Dependency<'a, GitCmdError>>,
    mut new_deps: HashMap<String, Dependency<'a, GitCmdError>>,
)
    -> Result<(), InstallDepsError<GitCmdError>>
{
    let mut actions = actions(&cur_deps, &new_deps);

    if actions.is_empty() {
        if !state_file_exists {
            write_state_file(&state_file_path, &cur_deps)
                .context(WriteInitialCurDepsFailed{state_file_path})?;
        }
        return Ok(());
    }

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

    Ok(())
}

#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Snafu)]
pub enum InstallDepsError<E>
where
    E: Error + 'static
{
    WriteInitialCurDepsFailed{
        source: WriteStateFileError,
        state_file_path: PathBuf,
    },
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
pub enum WriteStateFileError {
    OpenFailed{source: IoError},
    WriteDepLineFailed{source: IoError},
}

pub fn render_install_error(
    err: InstallError<GitCmdError>,
    cwd: &PathBuf,
    deps_file_name: &str,
)
    -> String
{
    match err {
        InstallError::NoDepsFileFound => {
            format!(
                "Couldn't find the dependency file '{}' in the current \
                 directory or parent directories",
                deps_file_name,
            )
        },
        InstallError::ReadDepsFileFailed{
            source: ReadDepsFileError::ReadFailed{source, deps_file_path},
        } => {
            format!(
                "Couldn't read the dependency file at '{}': {}",
                render_rel_path_else_abs(&cwd, &deps_file_path),
                source,
            )
        },
        InstallError::ConvDepsFileUtf8Failed{source, path, dep_name} => {
            if let Some(name) = dep_name {
                format!(
                    "{}: This nested dependency file (for '{}') contains an \
                     invalid UTF-8 sequence after byte {}",
                    render_rel_path_else_abs(&cwd, &path),
                    source.utf8_error().valid_up_to(),
                    name,
                )
            } else {
                format!(
                    "{}: This dependency file contains an invalid UTF-8 \
                     sequence after byte {}",
                    render_rel_path_else_abs(&cwd, &path),
                    source.utf8_error().valid_up_to(),
                )
            }
        },
        InstallError::ParseDepsConfFailed{source, path, dep_name} => {
            render_parse_deps_conf_error(source, &cwd, &path, dep_name)
        },
        InstallError::InstallProjDepsFailed{source, dep_name} => {
            let dep_descr =
                if let Some(n) = dep_name {
                    format!(" in the nested dependency '{}'", n)
                } else {
                    "".to_string()
                };
            render_install_proj_deps_error(source, &cwd, &dep_descr)
        },
        InstallError::ReadNestedDepsFileFailed{
            source,
            path,
            dep_name,
            dep_proj_path,
        } => {
            format!(
                "Couldn't read the dependency file ('{}') for the nested \
                 dependency '{}' ('{}'): {}",
                render_rel_path_else_abs(&cwd, &path),
                dep_name,
                render_rel_path_else_abs(&cwd, &dep_proj_path),
                source,
            )
        },
    }
}

fn render_install_proj_deps_error(
    err: InstallProjDepsError<GitCmdError>,
    cwd: &PathBuf,
    dep_descr: &str,
) -> String {
    match err {
        InstallProjDepsError::ReadStateFileFailed{source, path} =>
            format!(
                "Couldn't read the state file ('{}'): {}",
                render_rel_path_else_abs(&cwd, &path),
                source,
            ),
        InstallProjDepsError::ConvStateFileUtf8Failed{source, path} =>
            format!(
                "The state file ('{}') contains an invalid UTF-8 sequence \
                 after byte {}",
                render_rel_path_else_abs(&cwd, &path),
                source.utf8_error().valid_up_to(),
            ),
        InstallProjDepsError::ParseStateFileFailed{source, path} =>
            format!(
                "The state file ('{}') is invalid ({}), please remove this \
                 file and try again",
                render_rel_path_else_abs(&cwd, &path),
                render_parse_deps_error(source, &cwd, &path, None),
            ),
        InstallProjDepsError::CreateMainOutputDirFailed{source, path} =>
            format!(
                "Couldn't create {}, the main output directory: {}",
                render_rel_path_else_abs(&cwd, &path),
                source,
            ),
        InstallProjDepsError::InstallDepsFailed{source} =>
            render_install_deps_error(source, &cwd, &dep_descr),
    }
}

fn render_install_deps_error(
    err: InstallDepsError<GitCmdError>,
    cwd: &PathBuf,
    dep_descr: &str,
) -> String {
    match err {
        InstallDepsError::RemoveOldDepOutputDirFailed{
            source,
            dep_name,
            path,
        } =>
            format!(
                "Couldn't remove '{}', the output directory for the '{}' \
                 dependency: {}",
                render_rel_path_else_abs(&cwd, &path),
                dep_name,
                source,
            ),
        InstallDepsError::WriteCurDepsAfterRemoveFailed{
            source,
            dep_name,
            state_file_path,
        } =>
            render_write_cur_deps_err(
                source,
                cwd,
                &state_file_path,
                &format!("removing '{}'", dep_name),
            ),
        InstallDepsError::CreateDepOutputDirFailed{source, dep_name, path} =>
            format!(
                "Couldn't create '{}', the output directory for the '{}' \
                 dependency: {}",
                render_rel_path_else_abs(&cwd, &path),
                dep_name,
                source,
            ),
        InstallDepsError::WriteCurDepsAfterInstallFailed{
            source,
            dep_name,
            state_file_path,
        } =>
            render_write_cur_deps_err(
                source,
                &cwd,
                &state_file_path,
                &format!("installing '{}'", dep_name),
            ),
        InstallDepsError::WriteInitialCurDepsFailed{source, state_file_path} =>
            render_write_cur_deps_err(
                source,
                &cwd,
                &state_file_path,
                "updating dependencies",
            ),
        InstallDepsError::FetchFailed{source, dep_name} =>
            match source {
                FetchError::RetrieveFailed{source} =>
                    format!(
                        "Couldn't retrieve the source for the dependency \
                         '{}'{}: {}",
                        dep_name,
                        dep_descr,
                        render_git_cmd_err(source),
                    ),
                FetchError::VersionChangeFailed{source} =>
                    format!(
                        "Couldn't change the version for the '{}' dependency: \
                         {}",
                        dep_name,
                        render_git_cmd_err(source),
                    ),
            },
    }
}

fn render_parse_deps_conf_error(
    err: ParseDepsConfError,
    cwd: &PathBuf,
    deps_file_path: &PathBuf,
    dep_name: Option<String>,
) -> String {
    match err {
        ParseDepsConfError::ParseOutputDirFailed{source} =>
            match source {
                ParseOutputDirError::MissingOutputDir =>
                    if let Some(name) = dep_name {
                        format!(
                            "{}: This nested dependency file (for '{}') \
                             doesn't contain an output directory",
                            render_rel_path_else_abs(&cwd, &deps_file_path),
                            name,
                        )
                    } else {
                        format!(
                            "{}: This dependency file doesn't contain an \
                             output directory",
                            render_rel_path_else_abs(&cwd, &deps_file_path),
                        )
                    },
                ParseOutputDirError::InvalidPart{ln_num, part} =>
                    if let Some(name) = dep_name {
                        format!(
                            "{}:{}: This nested dependency file (for '{}') \
                             contains an invalid component ('{}') in its \
                             output directory",
                            render_rel_path_else_abs(&cwd, &deps_file_path),
                            ln_num,
                            name,
                            part,
                        )
                    } else {
                        format!(
                            "{}:{}: This dependency file contains an invalid \
                             component ('{}') in its output directory",
                            render_rel_path_else_abs(&cwd, &deps_file_path),
                            ln_num,
                            part,
                        )
                    },
            },
        ParseDepsConfError::ParseDepsFailed{source} =>
            render_parse_deps_error(source, &cwd, &deps_file_path, dep_name),
    }
}

fn render_parse_deps_error(
    err: ParseDepsError,
    cwd: &PathBuf,
    file_path: &PathBuf,
    proj_name: Option<String>,
) -> String {
    match err {
        ParseDepsError::DupDepName{ln_num, dep_name, orig_ln_num} => {
            if let Some(name) = proj_name {
                format!(
                    "{}:{}: A dependency named '{}' is already defined on \
                     line {} in the nested dependency '{}'",
                    render_rel_path_else_abs(&cwd, &file_path),
                    ln_num,
                    dep_name,
                    orig_ln_num,
                    name,
                )
            } else {
                format!(
                    "{}:{}: A dependency named '{}' is already defined on \
                     line {}",
                    render_rel_path_else_abs(&cwd, &file_path),
                    ln_num,
                    dep_name,
                    orig_ln_num,
                )
            }
        },
        ParseDepsError::ReservedDepName{ln_num, dep_name} => {
            format!(
                "{}:{}: '{}' is a reserved name and can't be used as a \
                 dependency name",
                render_rel_path_else_abs(&cwd, &file_path),
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
                "{}:{}: '{}' contains an invalid character{} at position {}; \
                 dependency names can only contain numbers, letters, hyphens, \
                 underscores and periods",
                render_rel_path_else_abs(&cwd, &file_path),
                ln_num,
                dep_name,
                bad_char,
                bad_char_idx + 1,
            )
        },
        ParseDepsError::InvalidDepSpec{ln_num, line} => {
            if let Some(name) = proj_name {
                format!(
                    "{}:{}: Invalid dependency specification in nested \
                     dependency '{}': '{}'",
                    render_rel_path_else_abs(&cwd, &file_path),
                    ln_num,
                    name,
                    line,
                )
            } else {
                format!(
                    "{}:{}: Invalid dependency specification: '{}'",
                    render_rel_path_else_abs(&cwd, &file_path),
                    ln_num,
                    line,
                )
            }
        },
        ParseDepsError::UnknownTool{ln_num, dep_name, tool_name} => {
            if let Some(name) = proj_name {
                format!(
                    "{}:{}: The dependency '{}' of the nested dependency '{}' \
                     specifies an invalid tool name ('{}'); the supported \
                     tool is 'git'",
                    render_rel_path_else_abs(&cwd, &file_path),
                    ln_num,
                    dep_name,
                    name,
                    tool_name,
                )
            } else {
                format!(
                    "{}:{}: The dependency '{}' specifies an invalid tool \
                     name ('{}'); the supported tool is 'git'",
                    render_rel_path_else_abs(&cwd, &file_path),
                    ln_num,
                    dep_name,
                    tool_name,
                )
            }
        },
    }
}

fn render_write_cur_deps_err(
    err: WriteStateFileError,
    cwd: &PathBuf,
    state_file_path: &PathBuf,
    action: &str,
) -> String {
    match err {
        WriteStateFileError::OpenFailed{source} =>
            format!(
                "Couldn't open the state file ('{}') for writing after {}: {}",
                render_rel_path_else_abs(&cwd, state_file_path),
                action,
                source,
            ),
        WriteStateFileError::WriteDepLineFailed{source} =>
            format!(
                "Couldn't write to the state file ('{}') after {}: {}",
                render_rel_path_else_abs(&cwd, state_file_path),
                action,
                source,
            ),
    }
}

// `render_rel_path_else_abs` renders `path` with `pre` stripped if `path` is a
// subdirectory of `pre`, otherwise `path` is rendered as an absolute path.
fn render_rel_path_else_abs(pre: &PathBuf, path: &PathBuf) -> String {
    let mut path_parts = path.iter();
    for pre_part in pre {
        if let Some(maybe_path_part) = path_parts.next() {
            if pre_part != maybe_path_part {
                return render_path(path)
            }
        } else {
            return render_path(path)
        }
    }

    render_path(&path_parts.collect::<PathBuf>())
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
