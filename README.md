# Vorleser - A Server Based Audiobook Application

[![Build Status](https://travis-ci.org/hatzel/vorleser-server.svg?branch=master)](https://travis-ci.org/hatzel/vorleser-server)
## Library
The library directory will contain your audiobooks.
Simply follow these simple rules when copying audiobooks to the directory:
* For single file books: copy them to the top level of the library directory
* For books with multiple files: create one top-level directory containing all chapters

When renaming files the new directory will not be associated with the old book.

## Regex
Provide a regular expression that matches only the audiobooks. Meaning either files or directories which form audiobooks and NOTHING else!
For example the default regex is:
`^[^/]+$` meaning anything without a slash will match.
This means it will match any top level directory or file but won't match anything that is not top level.

## Config file
`default-config.toml` contains an example configuration file.
We will explain some of the values in this document:

- `data_directory` a directory where vorleser will store data. This data consists of remuxed audiobooks as well as cover art. This directory can, depending on the size of your collection, get very large.
- `register_web` enable or disable registration of new accounts via the API. We will add some verification method to allow only a certain set of people to register with your vorleser instance.
- `database` specify the URL of the database that should be used
- The `[web]` section allows you to specify setting that affect the web server
    - `port` the port the web server should run on
    - `address` hostname or ip to serve the API on
