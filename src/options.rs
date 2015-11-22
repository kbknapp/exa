use std::cmp;
use std::env::var_os;
use std::fmt;
use std::num::ParseIntError;
use std::os::unix::fs::MetadataExt;

use clap::{ArgMatches, ClapError};
use natord;

use colours::Colours;
use feature::xattr;
use file::File;
use output::{Grid, Details, GridDetails, Lines};
use output::column::{Columns, TimeTypes, SizeFormat};
use term::dimensions;


/// These **options** represent a parsed, error-checked versions of the
/// user's command-line options.
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Options {
    /// The action to perform when encountering a directory rather than a
    /// regular file.
    pub dir_action: DirAction,

    /// How to sort and filter files before outputting them.
    pub filter: FileFilter,

    /// The type of output to use (lines, grid, or details).
    pub view: View,
}

impl Options {

    /// Whether the View specified in this set of options includes a Git
    /// status column. It's only worth trying to discover a repository if the
    /// results will end up being displayed.
    pub fn should_scan_for_git(&self) -> bool {
        match self.view {
            View::Details(Details { columns: Some(cols), .. }) => cols.should_scan_for_git(),
            View::GridDetails(GridDetails { details: Details { columns: Some(cols), .. }, .. }) => cols.should_scan_for_git(),
            _ => false,
        }
    }
}

impl Options {
    pub fn deduce(matches: &ArgMatches) -> Result<Options, Misfire> {
        let dir_action = DirAction::deduce(&matches);
        let filter = FileFilter::deduce(&matches);
        let view = try!(View::deduce(&matches, filter, dir_action));

        Ok(Options {
            dir_action: dir_action,
            view:       view,
            filter:     filter,
        })
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum View {
    Details(Details),
    Grid(Grid),
    GridDetails(GridDetails),
    Lines(Lines),
}

impl View {
    fn deduce(matches: &ArgMatches, filter: FileFilter, dir_action: DirAction) -> Result<View, Misfire> {
        let other_options_scan = || {
            let term_colours = TerminalColours::deduce(matches);
            let term_width   = try!(TerminalWidth::deduce(matches));

            if let TerminalWidth::Set(width) = term_width {
                let colours = match term_colours {
                    TerminalColours::Always    => Colours::colourful(),
                    TerminalColours::Never     => Colours::plain(),
                    TerminalColours::Automatic => Colours::colourful(),
                };

                if matches.is_present("oneline") {
                        Ok(View::Lines(Lines {
                             colours: colours,
                        }))
                }
                else if matches.is_present("tree") {
                    let details = Details {
                        columns: None,
                        header: false,
                        recurse: dir_action.recurse_options(),
                        filter: filter,
                        xattr: false,
                        colours: colours,
                    };

                    Ok(View::Details(details))
                }
                else {
                    let grid = Grid {
                        across: matches.is_present("across"),
                        console_width: width,
                        colours: colours,
                    };

                    Ok(View::Grid(grid))
                }
            }
            else {
                // If the terminal width couldn’t be matched for some reason, such
                // as the program’s stdout being connected to a file, then
                // fallback to the lines view.

                let colours = match term_colours {
                    TerminalColours::Always    => Colours::colourful(),
                    TerminalColours::Never     => Colours::plain(),
                    TerminalColours::Automatic => Colours::plain(),
                };

                if matches.is_present("tree") {
                    let details = Details {
                        columns: None,
                        header: false,
                        recurse: dir_action.recurse_options(),
                        filter: filter,
                        xattr: false,
                        colours: colours,
                    };

                    Ok(View::Details(details))
                }
                else {
                    let lines = Lines {
                         colours: colours,
                    };

                    Ok(View::Lines(lines))
                }
            }
        };

        if matches.is_present("long") {
            let long_options = {
                let term_colours = TerminalColours::deduce(matches);
                let colours = match term_colours {
                    TerminalColours::Always    => Colours::colourful(),
                    TerminalColours::Never     => Colours::plain(),
                    TerminalColours::Automatic => {
                        if dimensions().is_some() {
                            Colours::colourful()
                        }
                        else {
                            Colours::plain()
                        }
                    }
                };

                let details = Details {
                    columns: Some(Columns::deduce(matches)),
                    header: matches.is_present("header"),
                    recurse: dir_action.recurse_options(),
                    filter: filter,
                    xattr: xattr::ENABLED && matches.is_present("extended"),
                    colours: colours,
                };

                details
            };

            if matches.is_present("grid") {
                match other_options_scan() {
                    Ok(View::Grid(grid)) => return Ok(View::GridDetails(GridDetails { grid: grid, details: long_options })),
                    Ok(lines)            => return Ok(lines),
                    Err(e)               => return Err(e),
                }
            }
            else {
                return Ok(View::Details(long_options));
            }
        }

        other_options_scan()
    }
}

/// The **file filter** processes a vector of files before outputting them,
/// filtering and sorting the files depending on the user’s command-line
/// flags.
#[derive(Default, PartialEq, Debug, Copy, Clone)]
pub struct FileFilter {
    list_dirs_first: bool,
    reverse: bool,
    show_invisibles: bool,
    sort_field: SortField,
}

impl FileFilter {
    fn deduce(matches: &ArgMatches) -> FileFilter {
        let sort_field = if matches.is_present("sort") {
            value_t_or_exit!(matches.value_of("sort"), SortField)
        } else {
            SortField::default()
        };

        FileFilter {
            list_dirs_first: matches.is_present("group-directories-first"),
            reverse:         matches.is_present("reverse"),
            show_invisibles: matches.is_present("all"),
            sort_field:      sort_field,
        }
    }
}

impl FileFilter {

    /// Remove every file in the given vector that does *not* pass the
    /// filter predicate.
    pub fn filter_files(&self, files: &mut Vec<File>) {
        if !self.show_invisibles {
            files.retain(|f| !f.is_dotfile());
        }
    }

    /// Sort the files in the given vector based on the sort field option.
    pub fn sort_files(&self, files: &mut Vec<File>) {
        files.sort_by(|a, b| self.compare_files(a, b));

        if self.reverse {
            files.reverse();
        }

        if self.list_dirs_first {
            // This relies on the fact that `sort_by` is stable.
            files.sort_by(|a, b| b.is_directory().cmp(&a.is_directory()));
        }
    }

    pub fn compare_files(&self, a: &File, b: &File) -> cmp::Ordering {
        match self.sort_field {
            SortField::Unsorted      => cmp::Ordering::Equal,
            SortField::Name          => natord::compare(&*a.name, &*b.name),
            SortField::Size          => a.metadata.len().cmp(&b.metadata.len()),
            SortField::FileInode     => a.metadata.ino().cmp(&b.metadata.ino()),
            SortField::ModifiedDate  => a.metadata.mtime().cmp(&b.metadata.mtime()),
            SortField::AccessedDate  => a.metadata.atime().cmp(&b.metadata.atime()),
            SortField::CreatedDate   => a.metadata.ctime().cmp(&b.metadata.ctime()),
            SortField::Extension     => match a.ext.cmp(&b.ext) {
                cmp::Ordering::Equal  => natord::compare(&*a.name, &*b.name),
                order                 => order,
            },
        }
    }
}

/// What to do when encountering a directory?
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum DirAction {
    AsFile,
    List,
    Recurse(RecurseOptions),
}

impl DirAction {

    pub fn deduce(matches: &ArgMatches) -> DirAction {
        let recurse = matches.is_present("recurse");
        let list    = matches.is_present("list-dirs");
        let tree    = matches.is_present("tree");

        if tree || (recurse && !(list && tree)) {
            return DirAction::Recurse(RecurseOptions {
                max_depth: Some(value_t_or_exit!(matches.value_of("level"), usize)),
                tree: tree
            });
        } else if !recurse && list {
            return DirAction::AsFile;
        } 
        DirAction::List
    }

    pub fn recurse_options(&self) -> Option<RecurseOptions> {
        match *self {
            DirAction::Recurse(opts) => Some(opts),
            _ => None,
        }
    }

    pub fn treat_dirs_as_files(&self) -> bool {
        match *self {
            DirAction::AsFile => true,
            DirAction::Recurse(RecurseOptions { tree, .. }) => tree,
            _ => false,
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct RecurseOptions {
    pub tree:      bool,
    pub max_depth: Option<usize>,
}

impl RecurseOptions {

    pub fn is_too_deep(&self, depth: usize) -> bool {
        match self.max_depth {
            None    => false,
            Some(d) => {
                d <= depth
            }
        }
    }
}

/// User-supplied field to sort by.
arg_enum! {
    #[derive(PartialEq, Debug, Copy, Clone)]
    pub enum SortField {
        Unsorted, 
        Name, 
        Extension, 
        Size, 
        FileInode,
        ModifiedDate, 
        AccessedDate, 
        CreatedDate
    }
}

impl Default for SortField {
    fn default() -> SortField {
        SortField::Name
    }
}

#[derive(PartialEq, Debug)]
enum TerminalWidth {
    Set(usize),
    Unset,
}

impl TerminalWidth {
    fn deduce(_: &ArgMatches) -> Result<TerminalWidth, Misfire> {
        if let Some(columns) = var_os("COLUMNS").and_then(|s| s.into_string().ok()) {
            match columns.parse() {
                Ok(width)  => Ok(TerminalWidth::Set(width)),
                Err(e)     => Err(Misfire::FailedParse(e)),
            }
        }
        else if let Some((width, _)) = dimensions() {
            Ok(TerminalWidth::Set(width))
        }
        else {
            Ok(TerminalWidth::Unset)
        }
    }
}

arg_enum! {
    #[derive(PartialEq, Debug)]
    pub enum TerminalColours {
        Always,
        Automatic,
        Never
    }
}

impl Default for TerminalColours {

    fn default() -> TerminalColours {
        TerminalColours::Automatic
    }
}

impl TerminalColours {

    fn deduce(matches: &ArgMatches) -> TerminalColours {
        if matches.is_present("color") {
            return value_t_or_exit!(matches.value_of("color"), TerminalColours);
        } else if matches.is_present("colour") {
            return value_t_or_exit!(matches.value_of("colour"), TerminalColours);
        } 

        TerminalColours::default()
    }
}

impl Columns {

    fn deduce(matches: &ArgMatches) -> Columns {
        Columns {
            size_format: SizeFormat::deduce(matches),
            time_types:  TimeTypes::deduce(matches),
            inode:  matches.is_present("inode"),
            links:  matches.is_present("links"),
            blocks: matches.is_present("blocks"),
            group:  matches.is_present("group"),
            git:    cfg!(feature="git") && matches.is_present("git"),
        }
    }
}

impl SizeFormat {

    /// Determine which file size to use in the file size column based on
    /// the user’s options.
    ///
    /// The default mode is to use the decimal prefixes, as they are the
    /// most commonly-understood, and don’t involve trying to parse large
    /// strings of digits in your head. Changing the format to anything else
    /// involves the `--binary` or `--bytes` flags, and these conflict with
    /// each other.
    fn deduce(matches: &ArgMatches) -> SizeFormat {
        if matches.is_present("binary") {
            return SizeFormat::BinaryBytes;
        } else if matches.is_present("bytes") {
            return SizeFormat::JustBytes;
        }
        SizeFormat::DecimalBytes
    }
}


arg_enum! {
    pub enum TimeFields {
        Modified,
        Accessed,
        Created
    }
}

impl TimeTypes {

    /// based on the user’s options.
    ///
    /// There are two separate ways to pick which fields to show: with a
    /// flag (such as `--modified`) or with a parameter (such as
    /// `--time=modified`). An error is signaled if both ways are used.
    ///
    /// It’s valid to show more than one column by passing in more than one
    /// option, but passing *no* options means that the user just wants to
    /// see the default set.
    fn deduce(matches: &ArgMatches) -> TimeTypes {
        let mut modified = matches.is_present("modified");
        let mut created  = matches.is_present("created");
        let mut accessed = matches.is_present("accessed");
        if matches.is_present("time") {
            for t in value_t_or_exit!(matches.values_of("time"), TimeFields) {
                match t {
                    TimeFields::Modified => modified = true,
                    TimeFields::Accessed => accessed = true,
                    TimeFields::Created => created = true,
                }
            }
        } 
        
        if modified || created || accessed {
            return TimeTypes { accessed: accessed, modified: modified, created: created };
        } 
        TimeTypes::default()
    }
}

macro_rules! wlnerr(
    ($($arg:tt)*) => ({
        use std::io::{Write, stderr};
        writeln!(&mut stderr(), $($arg)*).expect("Failed to write to stderr");
    })
);

/// One of these things could happen instead of listing files.
#[derive(Debug)]
pub enum Misfire {
    /// A CLI parsing error from clap
    CliError(ClapError),
    /// A numeric option was given that failed to be parsed as a number.
    FailedParse(ParseIntError),
}

impl Misfire {

    /// Print this error and immediately exit the program.
    ///
    /// If the error is non-fatal then the error is printed to stdout and the
    /// exit status will be `0`. Otherwise, when the error is fatal, the error
    /// is printed to stderr and the exit status will be `1`.
    pub fn exit(&self) -> ! {
        if self.use_stderr() {
            wlnerr!("{}", self);
            ::std::process::exit(self.error_code())
        }
        println!("{}", self);
        ::std::process::exit(0)
    }

    /// Return whether this was a fatal error or not.
    pub fn use_stderr(&self) -> bool {
        match *self {
            Misfire::CliError(ref e) => e.use_stderr(),
            _ => true
        }
    }

    /// The OS return code this misfire should signify.
    #[cfg(target = "windows")]
    pub fn error_code(&self) -> i32 {
        3
    }
    #[cfg(not(target = "windows"))]
    pub fn error_code(&self) -> i32 {
        1
    }
}

impl fmt::Display for Misfire {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Misfire::FailedParse(ref e)     => write!(f, "Failed to parse number: {}", e),
            Misfire::CliError(ref e) => write!(f, "{}", e),
        }
    }
}

impl From<ClapError> for Misfire {
    fn from(e: ClapError) -> Self {
        Misfire::CliError(e)
    }
}