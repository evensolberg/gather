# Gather

Gathers files from directories and subdirectories into a target directory.

## Usage

`gather [FLAGS] <FILE(S)>... [--] <TARGET>`

Example:

`gather -m -p **/*.png **/*.jpg -- ../images/`

### Flags

|Short Form|Long Form|Description|
|:----|:---|:----------|
`-d`|`--debug`|Output debug information as we go. Supply it twice for trace-level logs.
`-h`|`--help`|Prints help information.
`-m`|`--move`|Move files instead of copying them.
`-o`|`--detail-off`|Don't print detailed information about each file processed.
`-p`|`--print-summary`|Print summary information about the number of files gathered.
`-q`|`--quiet`|Don't produce any output except errors while working.
`-r`|`--dry-run`|Iterate through the files and produce output without actually processing anything.
`-s`|`--stop-on-error`|Stop on error. If this flag isn't set, the application will attempt to continue in case of error.
`-V`|`--version`|Prints version information

### Arguments

|Argument|Description|
|:-------|:----------|
`<FILE(S)>...`|One or more file(s) to process. Wildcards and multiple files (e.g. `2019*.pdf 2020*.pdf`) are supported. Use `**` glob to recurse (i.e. `**/*.pdf`).<br>**Note: Case sensitive.**
`<TARGET>`|The target directory into which files are to be gathered.

## Notes

By default when using `zsh` on the Mac, the program exits with an error if one of the `<FILE>` arguments isn't found (ie. `*.jpg *.jpeg *.png` - `*.jpeg` not found). This is due to how this is handled in the shell.

You can work around this by issuing the following command: `setopt +o NO_MATCH`
