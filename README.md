# usi-run

[![Build Status](https://travis-ci.org/nozaq/usi-run.svg?branch=master)](https://travis-ci.org/nozaq/usi-run)
[![Build status](https://ci.appveyor.com/api/projects/status/glvdcd6tdohmmhrl?svg=true)](https://ci.appveyor.com/project/nozaq/usi-run)
[![Coverage Status](https://coveralls.io/repos/github/nozaq/usi-run/badge.svg?branch=master)](https://coveralls.io/github/nozaq/usi-run?branch=master)
[![crates.io](https://img.shields.io/crates/v/usi-run.svg)](https://crates.io/crates/usi-run)

A command line utility for automatically running games between USI compliant Shogi engines and collect match statistics.

Tested with popular USI engines, e.g. [Apery](https://github.com/HiraokaTakuya/apery), [Gikou(技巧)](https://github.com/gikou-official/Gikou), [YaneuraOu(やねうら王)](https://github.com/yaneurao/YaneuraOu).

## Installing

`usi-run` can be installed from Cargo.

```
$ cargo install usi-run
```

## Usage

```
A command line utility for running games between USI compliant Shogi engines.

USAGE:
    usirun [OPTIONS] --config <TOML>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <TOML>     Loads a configuration file for setting up match rules
    -d, --display <MODE>    Displays  [default: simple]  [values: board, csa, command, simple]
```

A configuration file looks like the following. See [example.toml](https://github.com/nozaq/usi-run/blob/master/example.toml) for more detail.
```toml
num_games = 10
max_ply = 256

[time_control]
black_time = 60000
white_time = 60000
black_inc = 10000
white_inc = 10000

[black]
engine_path = "/path/to/executable"
working_dir = "/path/to/dir"
ponder = false

    [black.options]
    USI_Hash = 128
    Threads = 1

[white]
engine_path = "/path/to/executable"
working_dir = "/path/to/dir"
ponder = false

    [white.options]
    USI_Hash = 128
    Threads = 1
```
