// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::path::Path;
use std::path::PathBuf;
use std::str;

use dep_tools::FetchError;
use dep_tools::GitCmdError;
use install::InstallDepsError;
use install::InstallError;
use install::InstallProjDepsError;
use install::ParseDepsConfError;
use install::ParseDepsError;
use install::ParseOutputDirError;
use install::ReadDepsFileError;
use install::WriteStateFileError;

pub fn render_install_error(
    err: InstallError<GitCmdError>,
    cwd: &Path,
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
                render_rel_path_else_abs(cwd, &deps_file_path),
                source,
            )
        },
        InstallError::ConvDepsFileUtf8Failed{source, path, dep_name} => {
            if let Some(name) = dep_name {
                format!(
                    "{}: This nested dependency file (for '{}') contains an \
                     invalid UTF-8 sequence after byte {}",
                    render_rel_path_else_abs(cwd, &path),
                    source.utf8_error().valid_up_to(),
                    name,
                )
            } else {
                format!(
                    "{}: This dependency file contains an invalid UTF-8 \
                     sequence after byte {}",
                    render_rel_path_else_abs(cwd, &path),
                    source.utf8_error().valid_up_to(),
                )
            }
        },
        InstallError::ParseDepsConfFailed{source, path, dep_name} => {
            render_parse_deps_conf_error(source, cwd, &path, dep_name)
        },
        InstallError::InstallProjDepsFailed{source, dep_name} => {
            let dep_descr =
                if let Some(n) = dep_name {
                    format!(" in the nested dependency '{}'", n)
                } else {
                    "".to_string()
                };
            render_install_proj_deps_error(source, cwd, &dep_descr)
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
                render_rel_path_else_abs(cwd, &path),
                dep_name,
                render_rel_path_else_abs(cwd, &dep_proj_path),
                source,
            )
        },
    }
}

fn render_install_proj_deps_error(
    err: InstallProjDepsError<GitCmdError>,
    cwd: &Path,
    dep_descr: &str,
)
    -> String
{
    match err {
        InstallProjDepsError::ReadStateFileFailed{source, path} =>
            format!(
                "Couldn't read the state file ('{}'): {}",
                render_rel_path_else_abs(cwd, &path),
                source,
            ),
        InstallProjDepsError::ConvStateFileUtf8Failed{source, path} =>
            format!(
                "The state file ('{}') contains an invalid UTF-8 sequence \
                 after byte {}",
                render_rel_path_else_abs(cwd, &path),
                source.utf8_error().valid_up_to(),
            ),
        InstallProjDepsError::ParseStateFileFailed{source, path} =>
            format!(
                "The state file ('{}') is invalid ({}), please remove this \
                 file and try again",
                render_rel_path_else_abs(cwd, &path),
                render_parse_deps_error(source, cwd, &path, None),
            ),
        InstallProjDepsError::CreateMainOutputDirFailed{source, path} =>
            format!(
                "Couldn't create {}, the main output directory: {}",
                render_rel_path_else_abs(cwd, &path),
                source,
            ),
        InstallProjDepsError::InstallDepsFailed{source} =>
            render_install_deps_error(source, cwd, dep_descr),
    }
}

fn render_install_deps_error(
    err: InstallDepsError<GitCmdError>,
    cwd: &Path,
    dep_descr: &str,
)
    -> String
{
    match err {
        InstallDepsError::RemoveOldDepOutputDirFailed{
            source,
            dep_name,
            path,
        } =>
            format!(
                "Couldn't remove '{}', the output directory for the '{}' \
                 dependency: {}",
                render_rel_path_else_abs(cwd, &path),
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
                render_rel_path_else_abs(cwd, &path),
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
                cwd,
                &state_file_path,
                &format!("installing '{}'", dep_name),
            ),
        InstallDepsError::WriteInitialCurDepsFailed{source, state_file_path} =>
            render_write_cur_deps_err(
                source,
                cwd,
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
    cwd: &Path,
    deps_file_path: &Path,
    dep_name: Option<String>,
)
    -> String
{
    match err {
        ParseDepsConfError::ParseOutputDirFailed{source} =>
            match source {
                ParseOutputDirError::MissingOutputDir =>
                    if let Some(name) = dep_name {
                        format!(
                            "{}: This nested dependency file (for '{}') \
                             doesn't contain an output directory",
                            render_rel_path_else_abs(cwd, deps_file_path),
                            name,
                        )
                    } else {
                        format!(
                            "{}: This dependency file doesn't contain an \
                             output directory",
                            render_rel_path_else_abs(cwd, deps_file_path),
                        )
                    },
                ParseOutputDirError::InvalidPart{ln_num, part} =>
                    if let Some(name) = dep_name {
                        format!(
                            "{}:{}: This nested dependency file (for '{}') \
                             contains an invalid component ('{}') in its \
                             output directory",
                            render_rel_path_else_abs(cwd, deps_file_path),
                            ln_num,
                            name,
                            part,
                        )
                    } else {
                        format!(
                            "{}:{}: This dependency file contains an invalid \
                             component ('{}') in its output directory",
                            render_rel_path_else_abs(cwd, deps_file_path),
                            ln_num,
                            part,
                        )
                    },
            },
        ParseDepsConfError::ParseDepsFailed{source} =>
            render_parse_deps_error(source, cwd, deps_file_path, dep_name),
    }
}

fn render_parse_deps_error(
    err: ParseDepsError,
    cwd: &Path,
    file_path: &Path,
    proj_name: Option<String>,
)
    -> String
{
    match err {
        ParseDepsError::DupDepName{ln_num, dep_name, orig_ln_num} => {
            if let Some(name) = proj_name {
                format!(
                    "{}:{}: A dependency named '{}' is already defined on \
                     line {} in the nested dependency '{}'",
                    render_rel_path_else_abs(cwd, file_path),
                    ln_num,
                    dep_name,
                    orig_ln_num,
                    name,
                )
            } else {
                format!(
                    "{}:{}: A dependency named '{}' is already defined on \
                     line {}",
                    render_rel_path_else_abs(cwd, file_path),
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
                render_rel_path_else_abs(cwd, file_path),
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
                render_rel_path_else_abs(cwd, file_path),
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
                    render_rel_path_else_abs(cwd, file_path),
                    ln_num,
                    name,
                    line,
                )
            } else {
                format!(
                    "{}:{}: Invalid dependency specification: '{}'",
                    render_rel_path_else_abs(cwd, file_path),
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
                    render_rel_path_else_abs(cwd, file_path),
                    ln_num,
                    dep_name,
                    name,
                    tool_name,
                )
            } else {
                format!(
                    "{}:{}: The dependency '{}' specifies an invalid tool \
                     name ('{}'); the supported tool is 'git'",
                    render_rel_path_else_abs(cwd, file_path),
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
    cwd: &Path,
    state_file_path: &Path,
    action: &str,
)
    -> String
{
    match err {
        WriteStateFileError::OpenFailed{source} =>
            format!(
                "Couldn't open the state file ('{}') for writing after {}: {}",
                render_rel_path_else_abs(cwd, state_file_path),
                action,
                source,
            ),
        WriteStateFileError::WriteDepLineFailed{source} =>
            format!(
                "Couldn't write to the state file ('{}') after {}: {}",
                render_rel_path_else_abs(cwd, state_file_path),
                action,
                source,
            ),
    }
}

// `render_rel_path_else_abs` renders `path` with `pre` stripped if `path` is a
// subdirectory of `pre`, otherwise `path` is rendered as an absolute path.
fn render_rel_path_else_abs(pre: &Path, path: &Path) -> String {
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

fn render_path(path: &Path) -> String {
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
