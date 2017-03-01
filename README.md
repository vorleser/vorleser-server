# Vorleser - A Server Based Audiobook Client

[![Build Status](https://travis-ci.org/hatzel/vorleser-server.svg?branch=master)](https://travis-ci.org/hatzel/vorleser-server)
## Library
The library directory will contain your audiobooks.
Simply follow these simple rules when copying audiobooks to the directory:
* For single file books: copy them to the top level of the library directory
* For books with multiple files: create one top-level directory containing all chapters

When renaming files the new directory will not be associated with the old book.

## Regex
Provide a regex that matches only the audiobooks. Meaning either files or directories which form audiobooks and NOTHING else!
