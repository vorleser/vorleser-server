# Vorleser - A Server Based Audiobook Application

[![Build Status](https://travis-ci.org/vorleser/vorleser-server.svg?branch=master)](https://travis-ci.org/vorleser/vorleser-server)
[![Coverage Status](https://coveralls.io/repos/github/vorleser/vorleser-server/badge.svg?branch=master)](https://coveralls.io/github/vorleser/vorleser-server?branch=master)

## Building
Run `cargo build`, you will need a somewhat recent version of FFmpeg, including headerfiles, installed on your system.

**Optionally** if you want to include the web interface in your build:

- Init the web frontend submodule `git submodule update --recursive --remote`
- Build the frontend according to the instructions in [vorleser-web/README.md](https://github.com/vorleser/vorleser-web)
- Run `cargo build --features webfrontend`

## Library
The library directory will contain your audiobooks.
Simply follow these simple rules when copying audiobooks to the directory:
* For single file books: copy them to the top level of the library directory
* For books with multiple files: create one top-level directory containing all chapters

Following these rules will mean you create a directory structure like this one:

```
├── another-book.mp3
├── my-fun-book.m4b
└── that_other_book
    ├── chapter 01.mp3
    ├── chapter 02.mp3
    └── chapter 03.mp3
```

When renaming files the new directory will not be associated with the old book.

### Regex
The rules above can be customized using a regular expression.
Provide a regex that matches only the audiobooks. Meaning either files or directories which form audiobooks and NOTHING else!

Specify them at library creation using `vorleser create-library /data/my-library ^[^/]+$`

The default regex is `^[^/]+$` meaning any file name without a slash will match.
This means it will match any top level directory or file but won't match anything that is not top level, requiring a directory structure as defined above.


## Config File
`default-config.toml` contains an example configuration file.
We will explain some of the values in this document:

- `data_directory` a directory where vorleser will store data. This data consists of remuxed audiobooks as well as cover art. This directory can, depending on the size of your collection, get very large.
- `register_web` enable or disable registration of new accounts via the API.
- `sentry_dsn` supply a sentry instance for errors to be reported to.
- `database` specify the URL of the database that should be used
- The `[web]` section allows you to specify setting that affect the web server
    - `port` the port the web server should run on
    - `address` hostname or ip to serve the API on
- The `[logging]` section allows you to specify which events to log
    - `level` which level of logs to show, with the default being `info`. If you want to see less logs consider setting this to `error`.
    - `file` a file path for vorleser to write its logs to. Make sure the directory exists and vorleser can write it.

## Audio File Formats

We have tested things with `mp3`, `m4a` and `m4b` files. However, since all audio handling is done by FFmpeg, any format supported by your FFmpeg installation should work.

For multi-file audiobooks made of `mp3` files with variable bitrate we generate MLLT tags to aid precise seeking. Clients without support for MLLT may not be able to seek precisely or display correct durations.

Clients may only support some audio formats, as we don't do server-side transcoding (yet?).

## Docker

The server is available on Docker Hub as `vorleser/server`.

Mount a volume for the database and remuxed audio files at `/var/lib/vorleser`. Mount directories containing your audiobooks anywhere you want.

Run `vorleser-server create-library` in the container to add the audiobook directories you mounted, and run `vorleser-server create-user` to create any users you want. See the `--help` output of both for more info.

The container exposes port 8000 for the HTTP server.

### Example

`docker-compose.yml`

```
version: '3'

volumes:
  data:

services:
  server:
    restart: always
    image: vorleser/server
    volumes:
      - data:/var/lib/vorleser
      - /path/to/my/audiobooks:/audiobooks:ro
```

Then run:

```
$ docker-compose up -d
$ docker-compose exec server vorleser-server create-library /audiobooks '^[^/]+/[^./][^/]*$'
$ docker-compose exec server vorleser-server create-user very_user much_secure
```
