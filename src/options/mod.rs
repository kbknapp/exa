use std::path::PathBuf;

use clap::ArgMatches;

use output::{Details, GridDetails};

mod dir_action;
pub use self::dir_action::{DirAction, RecurseOptions};

mod filter;
pub use self::filter::{FileFilter, SortField, SortCase};

mod misfire;
pub use self::misfire::Misfire;

mod view;
pub use self::view::View;


/// These **options** represent a parsed, error-checked versions of the
/// user’s command-line options.
#[derive(PartialEq, Debug, Clone)]
pub struct Options {

    /// The action to perform when encountering a directory rather than a
    /// regular file.
    pub dir_action: DirAction,

    /// How to sort and filter files before outputting them.
    pub filter: FileFilter,

    /// The type of output to use (lines, grid, or details).
    pub view: View,

    /// A list of files/dirs to display
    pub paths: Vec<PathBuf>,
}

impl Options {
    /// Determines the complete set of options based on the given command-line
    /// arguments, after they’ve been parsed.
    pub fn from_matches(matches: ArgMatches) -> Result<Options, Misfire> {
        let dir_action = try!(DirAction::deduce(&matches));
        let filter = try!(FileFilter::deduce(&matches));
        let view = try!(View::deduce(&matches, filter.clone(), dir_action));

        Ok(Options {
            dir_action: dir_action,
            view:       view,
            filter:     filter,  // TODO: clone
            paths:      matches.values_of("paths").unwrap().map(|p| p.into()).collect(),
        })
    }

    /// Whether the View specified in this set of options includes a Git
    /// status column. It’s only worth trying to discover a repository if the
    /// results will end up being displayed.
    pub fn should_scan_for_git(&self) -> bool {
        match self.view {
            View::Details(Details { columns: Some(cols), .. }) => cols.should_scan_for_git(),
            View::GridDetails(GridDetails { details: Details { columns: Some(cols), .. }, .. }) => cols.should_scan_for_git(),
            _ => false,
        }
    }
}


#[cfg(test)]
mod test {
    use super::{Options, Misfire, SortField, SortCase};
    use fs::feature::xattr;

    #[test]
    fn files() {
        let args = Options::getopts(&[ "this file".to_string(), "that file".to_string() ]).unwrap().1;
        assert_eq!(args, vec![ "this file".to_string(), "that file".to_string() ])
    }

    #[test]
    fn file_sizes() {
        let opts = Options::getopts(&[ "--long", "--binary", "--bytes" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Conflict("binary", "bytes"))
    }

    #[test]
    fn just_binary() {
        let opts = Options::getopts(&[ "--binary" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("binary", false, "long"))
    }

    #[test]
    fn just_bytes() {
        let opts = Options::getopts(&[ "--bytes" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("bytes", false, "long"))
    }

    #[test]
    fn long_across() {
        let opts = Options::getopts(&[ "--long", "--across" ]);
        assert_eq!(opts, Err(Misfire::Useless("across", true, "long")))
    }

    #[test]
    fn oneline_across() {
        let opts = Options::getopts(&[ "--oneline", "--across" ]);
        assert_eq!(opts, Err(Misfire::Useless("across", true, "oneline")))
    }

    #[test]
    fn just_header() {
        let opts = Options::getopts(&[ "--header" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("header", false, "long"))
    }

    #[test]
    fn just_group() {
        let opts = Options::getopts(&[ "--group" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("group", false, "long"))
    }

    #[test]
    fn just_inode() {
        let opts = Options::getopts(&[ "--inode" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("inode", false, "long"))
    }

    #[test]
    fn just_links() {
        let opts = Options::getopts(&[ "--links" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("links", false, "long"))
    }

    #[test]
    fn just_blocks() {
        let opts = Options::getopts(&[ "--blocks" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("blocks", false, "long"))
    }

    #[test]
    fn test_sort_size() {
        let opts = Options::getopts(&[ "--sort=size" ]);
        assert_eq!(opts.unwrap().0.filter.sort_field, SortField::Size);
    }

    #[test]
    fn test_sort_name() {
        let opts = Options::getopts(&[ "--sort=name" ]);
        assert_eq!(opts.unwrap().0.filter.sort_field, SortField::Name(SortCase::Sensitive));
    }

    #[test]
    fn test_sort_name_lowercase() {
        let opts = Options::getopts(&[ "--sort=Name" ]);
        assert_eq!(opts.unwrap().0.filter.sort_field, SortField::Name(SortCase::Insensitive));
    }

    #[test]
    #[cfg(feature="git")]
    fn just_git() {
        let opts = Options::getopts(&[ "--git" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("git", false, "long"))
    }

    #[test]
    fn extended_without_long() {
        if xattr::ENABLED {
            let opts = Options::getopts(&[ "--extended" ]);
            assert_eq!(opts.unwrap_err(), Misfire::Useless("extended", false, "long"))
        }
    }

    #[test]
    fn level_without_recurse_or_tree() {
        let opts = Options::getopts(&[ "--level", "69105" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless2("level", "recurse", "tree"))
    }
}
