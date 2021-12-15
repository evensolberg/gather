# Gather

Gathers files from directories and subdirectories into a target directory.

## Usage

`gather [FLAGS] <FILE(S)>... [--] <TARGET>`

### Flags

|Short Form|Long Form|Description|
|:----|:---|:----------|
`-d`|`--debug`|Output debug information as we go. Supply it twice for trace-level logs.
`-o`|`--detail-off`|Don't print detailed information about each file processed.
`-h`|`--help`|Prints help information.
`-m`|`--move`|Move files instead of copying them.
`-q`|`--quiet`|Don't produce any output except errors while working.
`-s`|`--stop-on-error`|Stop on error. If this flag isn't set, the application will attempt to continue in case of error.
`-u`|`--print-summary`|Print summary information about the number of files gathered.
`-V`|`--version`|Prints version information

### Arguments

|Argument|Description|
|:-------|:----------|
`<FILE(S)>...`|One or more file(s) to process. Wildcards and multiple files (e.g. `2019*.pdf 2020*.pdf`) are supported. Use `**` glob to recurse (i.e. `**/*.pdf`).
`<TARGET>`|The target directory into which files are to be gathered.

## Notes

Currently, using `zsh` on the Mac, the program exits with an error if one of the `<FILE>` arguments isn't found (ie. `*.jpg *.jpeg *.png` - `*.jpeg` not found). This is due to how this is handled in the shell.

You can work around this by using the following command: `setopt NO_MATCH`
