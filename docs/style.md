Code Style
==========

About
-----

This document outlines code style conventions used throughout this codebase.

`rustfmt`
---------

`rustfmt` is not being used by this project because it doesn't honour the rules
defined in this document.

Rule of Thumb
-------------

Rust code tends to be relatively terse due to its explicit nature which means
that there can often be a lot going on on a single line. This project takes the
approach of being liberal with line breaks in an effort to try and increase the
"scan-ability" and readability of individual lines.

Rules
-----

### General

#### Line length

This project uses a maximum line length of 79 for Rust files.

This rule is relaxed for strings in test files, which may go beyond 79
characters.

##### Rationale

Rust code tends to be relatively terse due to its explicit nature which means
that there can often be a lot going on on a single line. A small maximum line
length is used to help work around this tendency. See
<https://github.com/rust-lang/rust-guidelines/pull/12> for the reason why 79 is
used instead of 80.

#### `wrap_err!` lines

`wrap_err!` parameters must be split onto separate lines, even if the full call
could fit onto one line. For example:

    let cwd = wrap_err!(
        env::current_dir(),
        InstallError::GetCurrentDirFailed,
    );

#### `.expect()`

`.expect()` must only be used in tests, and should always be on its own line.
For example, instead of the following:

    fs::create_dir(&path).expect("couldn't create directory");

Do the following:

    fs::create_dir(&path)
        .expect("couldn't create directory");

#### `match` branches

Branches of a `match` expression should all be in blocks, or else all outside
blocks; blocks should not be interspersed with non-blocks. For example, instead
of the following:

    let tool = match tool_factories.get(&tool_name) {
        Some(tool_factory) => tool_factory.create(),
        None => {
            let err = ParseDepsError::UnknownTool(ln_num, local_name, tool_name)
            return Err(err);
        },
    };

Do the following:

    let tool = match tool_factories.get(&tool_name) {
        Some(tool_factory) => {
            tool_factory.create()
        },
        None => {
            let err = ParseDepsError::UnknownTool(ln_num, local_name, tool_name)
            return Err(err);
        },
    };

### Functions

#### Return type line

If a return type is specified on its own line then it should be indented and the
function's opening brace should be on the following line. For example:

    fn read_deps_file(start: PathBuf, deps_file_name: &str)
        -> Option<(PathBuf, Vec<u8>)>
    {

Another example:

    fn parse_deps_conf<'a>(
        conts: &str,
        tool_factories: &HashMap<String, &'a (dyn DepToolFactory<String> + 'a)>,
    )
        -> Result<DepsConf<'a, String>, ParseDepsConfError>
    {

#### End-of-function returns

Functions that end by returning a value shouldn't use the `return` keyword for
its final expression. The final expression should instead have a blank above it,
unless it is the only expression in the function.

#### Chain head

The "head" of a chain of function calls, when there is more than one chained
call, must be on its own line. For example, instead of the following:

    let maybe_output = Command::new("git")
        .args(git_args)
        .current_dir(out_dir)
        .output();

Do the following:

    let maybe_output =
        Command::new("git")
            .args(git_args)
            .current_dir(out_dir)
            .output();

One chained function call is allowable on a single line. For example:

    let words: Vec<&str> = ln.split_ascii_whitespace().collect();

### Errors

Error `enum`s should end with `Error`. This project also identifies two types of
error value. An error that contains a nested error is considered a "failed
operation" error. Such error values should end with `Failed` and the nested
error(s) should be the first listed in the associated tuple data. For example:

    enum InstallError<E> {
        GetCurrentDirFailed(IoError),
        ...
    }

Any other error is considered a "root" error, and has no required naming or data
conventions. For example:

    enum InstallError<E> {
        ...
        NoDepsFileFound,
        ...
    }

    enum ParseDepsError {
        InvalidDependencySpec(usize, String),
        UnknownTool(usize, String, String),
    }
