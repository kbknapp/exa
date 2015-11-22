#![warn(trivial_casts, trivial_numeric_casts)]
#![warn(unused_qualifications)]
#![warn(unused_results)]

extern crate ansi_term;
extern crate datetime;
extern crate libc;
extern crate locale;
extern crate natord;
extern crate num_cpus;
extern crate number_prefix;
extern crate scoped_threadpool;
extern crate term_grid;
extern crate unicode_width;
extern crate users;

#[cfg(feature="git")] extern crate git2;
#[macro_use] extern crate lazy_static;

#[macro_use] extern crate clap;

use std::path::{Component, Path};
use std::env;
use std::ffi::OsStr;

use clap::{App, Arg, ArgGroup};

use dir::Dir;
use file::File;
use options::{Options, Misfire, View, SortField, TerminalColours, TimeFields};

mod colours;
mod dir;
mod feature;
mod file;
mod filetype;
mod options;
mod output;
mod term;

#[macro_use]
mod macros {
    macro_rules! git_arg {
        () => ({
            Arg::from_usage("--git 'show git status (implies --long)'")
        });
    }

    #[cfg(all(feature = "git", any(target = "macos", target = "linux")))]
    macro_rules! conditional_git_args {
        () => ({
            vec![
                git_arg!(),
                Arg::with_name("extended")
                    .short("@")
                    .long("extended")
                    .help("display extended attribute keys and sizes (implies --long)")
            ]
        });
    }

    #[cfg(all(feature = "git", not(any(target = "linux", target = "macos"))))]
    macro_rules! conditional_args {
        () => ({
            vec![git_arg!()]
        });
    }
    #[cfg(not(feature = "git"))]
    macro_rules! conditional_args {
        () => ({
            vec![]
        });
    }
}

struct Exa {
    options: Options,
    paths: Vec<String>
}

impl Exa {
    pub fn build_cli<'a, 'b, 'c, 'd, 'e, 'f>() -> App<'a, 'b, 'c, 'd, 'e, 'f>
        let sort_types = SortField::variants();
        let colours = TerminalColours::variants();
        let time_vals = TimeFields::variants();
        let time_conflicts = ["modified", "created", "accessed"];
        let list_conflicts = ["recurse", "tree"];
        let oneline_conflicts = ["grid", "across", "tree"];
        let ver = format!("v{}", crate_version!());
        let cli = App::new("exa")
            .version(&ver)
            .author("Benjamin Sago <ogham@bsago.me>")
            .about("Replacement for ls which lists files")
            .help_short("?")

            // Display options
            .args_from_usage(
                 "-G, --grid     'display entries in a grid view (default)'
                 -l, --long      'display extended details and attributes'
                 -R, --recurse   'recurse into directories'
                 -T, --tree      'recurse into subdirectories in a tree view'
                 -x, --across    'sort multi-column view entries across (implies --grid)'")
            .arg(Arg::from_usage("[color] --color [WHEN] 'when to show anything in colors'")
                .value_name("WHEN")
                .possible_values(&colours))
            .arg(Arg::from_usage("[colour] --colour [WHEN] 'when to show anything in colours (alternate spelling)'")
                .possible_values(&colours)
                .value_name("WHEN"))
            .arg(Arg::from_usage("-1, --oneline   'display one entry per line'")
                 .conflicts_with_all(&oneline_conflicts))

            // Filtering and sorting options
            .args_from_usage(
                "    --group-directories-first 'list directories before other files'
                 -a, --all                     'show dot-files'
                 -r, --reverse                 'reverse order of files'")
            .arg(Arg::from_usage("-s, --sort [FIELD] 'field to sort by'")
                 .possible_values(&sort_types)
                 .value_name("FIELD"))
            .arg(Arg::from_usage("-d, --list-dirs    'list directories as regular files'")
                 .conflicts_with_all(&list_conflicts))

            // Long view options
            .args_from_usage(
                "-b, --binary           'use binary prefixes in file sizes (implies --long)'
                 -g, --group            'show group as well as user (implies --long)'
                 -h, --header           'show a header row at the top (implies --long)'
                 -H, --links            'show number of hard links (implies --long)'
                 -i, --inode            'show each file's inode number (implies --long)'
                 -m, --modified         'display timestamp of most recent modification'
                 -S, --blocks           'show number of file system blocks (implies --long)'
                 -u, --accessed         'display timestamp of last access for a file (implies --long)'
                 -U, --created          'display timestamp of creation for a file (implies --long)'")
            .arg(Arg::from_usage("-t, --time [WORD]... 'which timestamp to show for a file (implies --long)'")
                .value_name("WORD")
                .possible_values(&time_vals)
                .conflicts_with_all(&time_conflicts))
            .arg(Arg::from_usage("-B, --bytes    'list file sizes in bytes, without prefixes (implies --long)'")
                 .conflicts_with("binary"))
            .arg(Arg::from_usage("-L, --level [DEPTH] 'maximum depth of recursion'")
                .requires("needs_level")
                .value_name("DEPTH"))
            .arg_group(ArgGroup::with_name("needs_level")
                .add("recurse")
                .add("tree"))
            .args(conditional_args!())
            .arg_from_usage("[paths]... 'paths to filter and display'")
    }

    pub fn new() -> Result<Self, Misfire> {
        let matches = Exa::build_cli().get_matches();
        Ok(Exa { 
            options: try!(Options::deduce(&matches)),
            paths: matches.values_of("paths").unwrap_or(vec!["."]).iter().map(|&p| p.to_owned()).collect()
        })
    }

    fn run(&mut self) {
        let mut files = Vec::new();
        let mut dirs = Vec::new();

        for file_name in &self.paths {
            match File::from_path(Path::new(&file_name), None) {
                Err(e) => {
                    println!("{}: {}", file_name, e);
                },
                Ok(f) => {
                    if f.is_directory() && !self.options.dir_action.treat_dirs_as_files() {
                        match f.to_dir(self.options.should_scan_for_git()) {
                            Ok(d) => dirs.push(d),
                            Err(e) => println!("{}: {}", file_name, e),
                        }
                    }
                    else {
                        files.push(f);
                    }
                },
            }
        }

        let no_files = files.is_empty();
        if !no_files {
            self.print_files(None, files);
        }

        let is_only_dir = dirs.len() == 1;
        self.print_dirs(dirs, no_files, is_only_dir);
    }

    fn print_dirs(&self, dir_files: Vec<Dir>, mut first: bool, is_only_dir: bool) {
        for dir in dir_files {
            // Put a gap between directories, or between the list of files and the
            // first directory.
            if first {
                first = false;
            }
            else {
                print!("\n");
            }

            if !is_only_dir {
                println!("{}:", dir.path.display());
            }

            let mut children = Vec::new();
            for file in dir.files() {
                match file {
                    Ok(file)       => children.push(file),
                    Err((path, e)) => println!("[{}: {}]", path.display(), e),
                }
            };

            self.options.filter.filter_files(&mut children);
            self.options.filter.sort_files(&mut children);

            if let Some(recurse_opts) = self.options.dir_action.recurse_options() {
                let depth = dir.path.components().filter(|&c| c != Component::CurDir).count() + 1;
                if !recurse_opts.tree && !recurse_opts.is_too_deep(depth) {

                    let mut child_dirs = Vec::new();
                    for child_dir in children.iter().filter(|f| f.is_directory()) {
                        match child_dir.to_dir(false) {
                            Ok(d)  => child_dirs.push(d),
                            Err(e) => println!("{}: {}", child_dir.path.display(), e),
                        }
                    }

                    self.print_files(Some(&dir), children);

                    if !child_dirs.is_empty() {
                        self.print_dirs(child_dirs, false, false);
                    }

                    continue;
                }
            }

            self.print_files(Some(&dir), children);
        }
    }

    fn print_files(&self, dir: Option<&Dir>, files: Vec<File>) {
        match self.options.view {
            View::Grid(g)         => g.view(&files),
            View::Details(d)      => d.view(dir, files),
            View::GridDetails(gd) => gd.view(dir, &files),
            View::Lines(l)        => l.view(&files),
        }
    }
}

fn main() {
    Exa::new().unwrap_or_else(|e| e.exit()).run();
}


#[cfg(test)]
mod test {
    use super::Exa;
    use clap::{ClapError, ArgMatches};

    fn parse_cli<'z, 'a, I, S>(args: I) -> Result<ArgMatches<'a, 'a>, ClapError> 
        where I: IntoIterator<Item=S>,
              S: AsRef<OsStr> + 'z {
        Exa::build_cli().get_matches_from_safe(args)
    }

    #[test]
    fn files() {
        let args = parse_cli(&["", "this file", "that file"]).unwrap();
        assert_eq!(&args.values_of("paths").unwrap(), &[ "this file", "that file"])
    }

    #[test]
    fn no_args() {
        let args = parse_cli(&[""]).unwrap();
        assert!(!args.is_present("paths"));  
    }

    #[test]
    fn file_sizes() {
        let res = parse_cli(&["", "--long", "--binary", "--bytes"]);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().error_kind(), Misfire::Conflict("binary", "bytes"))
    }

    #[test]
    fn just_binary() {
        let opts = Options::getopts(&[ "--binary".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("binary", false, "long"))
    }

    #[test]
    fn just_bytes() {
        let opts = Options::getopts(&[ "--bytes".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("bytes", false, "long"))
    }

    #[test]
    fn long_across() {
        let opts = Options::getopts(&[ "--long".to_string(), "--across".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("across", true, "long"))
    }

    #[test]
    fn oneline_across() {
        let opts = Options::getopts(&[ "--oneline".to_string(), "--across".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("across", true, "oneline"))
    }

    #[test]
    fn just_header() {
        let opts = Options::getopts(&[ "--header".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("header", false, "long"))
    }

    #[test]
    fn just_group() {
        let opts = Options::getopts(&[ "--group".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("group", false, "long"))
    }

    #[test]
    fn just_inode() {
        let opts = Options::getopts(&[ "--inode".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("inode", false, "long"))
    }

    #[test]
    fn just_links() {
        let opts = Options::getopts(&[ "--links".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("links", false, "long"))
    }

    #[test]
    fn just_blocks() {
        let opts = Options::getopts(&[ "--blocks".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("blocks", false, "long"))
    }

    #[test]
    #[cfg(feature="git")]
    fn just_git() {
        let opts = Options::getopts(&[ "--git".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("git", false, "long"))
    }

    #[test]
    fn extended_without_long() {
        if xattr::ENABLED {
            let opts = Options::getopts(&[ "--extended".to_string() ]);
            assert_eq!(opts.unwrap_err(), Misfire::Useless("extended", false, "long"))
        }
    }

    #[test]
    fn level_without_recurse_or_tree() {
        let opts = Options::getopts(&[ "--level".to_string(), "69105".to_string() ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless2("level", "recurse", "tree"))
    }
}