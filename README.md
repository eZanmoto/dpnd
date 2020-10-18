`dpnd`
======

About
-----

`dpnd` (pronounced "depend") is a tool for pulling project dependencies. What
`make` is to build automation, `dpnd` is to dependency automation.

In the same way that `make` approaches build automation in a language-agnostic
way, `dpnd` aims to provide dependency automation in a language-agnostic way.
This can be useful for a number of reasons:

* There are many types of files that don't have a dependency management system
  associated with them, but are nonetheless useful to share and put under
  version control. Some examples are Makefiles, Dockerfiles, Bash scripts, and
  even configuration files, many of which which allow importing or executing
  further "utility"/"shared" files.
* Most mainstream languages have their own language-specific dependency
  management system. These work well, but the fact that they are
  language-specific means that they're usually not flexible enough to handle
  dependency management for other, more general file types. `dpnd` attempts to
  provide support for such files in a uniform way.
* Many dependency management systems work off of the basis of public artefact
  repositories that require project publishers to push artefacts for consumers
  to use. `dpnd` allows specifying dependencies in the consumer directly,
  meaning that a consumer can depend on projects that haven't been pushed to a
  public artefact repository. This can be useful for depending on such projects
  in a uniform way, especially if such dependencies don't need to be built
  before being used.

Note that `dpnd` only handles fetching dependencies and doesn't support building
them, which must therefore be handled in a separate build step.

Usage
-----

`dpnd` only requires a `dpnd.txt` file in the root of the project in order to
run. Here is an example `dpnd.txt` file:

    target/deps

    deploy git https://github.com/eZanmoto/deploy_scripts v3.0
    # We use the `create_user.sh` convenience script from `example`.
    example git git@github.com:eZanmoto/example.git fedcba

With the above in place, running `dpnd install` within the project will pull all
of the named projects under `target/deps`, so the first project will be
accessible under `target/deps/deploy`, the second under `target/deps/example`,
and so on.

Development
-----------

### Build environment

The build environment for the project is defined in `scripts/build.Dockerfile`.
The build environment can be replicated locally by following the setup defined
in the Dockerfile, or Docker can be used to mount the local directory in the
build environment by running the following:

    $ bash scripts/with_build_env.sh bash

### Building

The project can be built locally using `cargo build`, or can be built using
Docker by running the following:

    $ bash scripts/with_build_env.sh cargo build

### Testing

The project can be tested locally using `make check`, or can be built using
Docker by running the following:

    $ bash scripts/with_build_env.sh make check

A subset of integration tests can be run by passing `TESTS` to Make:

    $ make check_intg TESTS=add

The command above will run all integration tests whose name contains "add".

FAQs
----

### How do I use different versions of the same project at the same time?

This isn't recommended because of the complicated nature of using different
versions of a project at the same time, but it is possible by using different
names for the project in `dpnd.txt`:

    target/deps

    deploy_v1 git https://github.com/eZanmoto/deploy_scripts v1.2
    deploy_v3 git https://github.com/eZanmoto/deploy_scripts v3.0
