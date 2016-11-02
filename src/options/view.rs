use std::env::var_os;
use std::str::FromStr;

use clap::ArgMatches;

use output::Colours;
use output::{Grid, Details, GridDetails, Lines};
use options::{FileFilter, DirAction, Misfire};
use output::column::{Columns, SizeFormat};
use term::dimensions;
use fs::feature::xattr;


/// The **view** contains all information about how to format output.
#[derive(PartialEq, Debug, Clone)]
pub enum View {
    Details(Details),
    Grid(Grid),
    GridDetails(GridDetails),
    Lines(Lines),
}

impl View {

    /// Determine which view to use and all of that view’s arguments.
    pub fn deduce(matches: &ArgMatches, filter: FileFilter, dir_action: DirAction) -> Result<View, Misfire> {
        let colour_scale = || {
            matches.is_present("color-scale") 
        };

        let long = || {
            let term_colours = TerminalColours::from(matches);
            let colours = match term_colours {
                TerminalColours::Always    => Colours::colourful(colour_scale()),
                TerminalColours::Never     => Colours::plain(),
                TerminalColours::Automatic => {
                    if dimensions().is_some() {
                        Colours::colourful(colour_scale())
                    }
                    else {
                        Colours::plain()
                    }
                },
            };

            Details {
                columns: Some(Columns::from(matches)),
                header: matches.is_present("header"),
                recurse: dir_action.recurse_options(),
                filter: filter.clone(),
                xattr: xattr::ENABLED && matches.is_present("extended"),
                colours: colours,
            }
        };

        let other_options_scan = || {
            let term_colours = TerminalColours::from(matches);
            let term_width   = try!(TerminalWidth::deduce());
            let details = |colours| {
                Details {
                    columns: None,
                    header: false,
                    recurse: dir_action.recurse_options(),
                    filter: filter.clone(),  // TODO: clone
                    xattr: false,
                    colours: colours,
                }
            };

            if let Some(&width) = term_width.as_ref() {
                let colours = match term_colours {
                    TerminalColours::Always    => Colours::colourful(colour_scale()),
                    TerminalColours::Never     => Colours::plain(),
                    TerminalColours::Automatic => Colours::colourful(colour_scale()),
                };

                if matches.is_present("oneline") {
                    Ok(View::Lines(Lines { colours: colours }))
                }
                else if matches.is_present("tree") {
                    Ok(View::Details(details(colours)))
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
                    TerminalColours::Always    => Colours::colourful(colour_scale()),
                    TerminalColours::Never     => Colours::plain(),
                    TerminalColours::Automatic => Colours::plain(),
                };

                if matches.is_present("tree") {
                    Ok(View::Details(details(colours)))
                }
                else {
                    Ok(View::Lines(Lines { colours: colours }))
                }
            }
        };

        if matches.is_present("long") {
            let long_options = long();

            if matches.is_present("grid") {
                match other_options_scan() {
                    Ok(View::Grid(grid)) => return Ok(View::GridDetails(GridDetails { grid: grid, details: long_options })),
                    Ok(lines)            => return Ok(lines),
                    _                    => unreachable!()
                };
            }
            else {
                return Ok(View::Details(long_options));
            }
        }

        other_options_scan()
    }
}


/// The width of the terminal requested by the user.
#[derive(PartialEq, Debug)]
enum TerminalWidth {

    /// The user requested this specific number of columns.
    Set(usize),

    /// The terminal was found to have this number of columns.
    Terminal(usize),

    /// The user didn’t request any particular terminal width.
    Unset,
}

impl TerminalWidth {

    /// Determine a requested terminal width from the command-line arguments.
    ///
    /// Returns an error if a requested width doesn’t parse to an integer.
    fn deduce() -> Result<TerminalWidth, Misfire> {
        if let Some(columns) = var_os("COLUMNS").and_then(|s| s.into_string().ok()) {
            match columns.parse() {
                Ok(width)  => Ok(TerminalWidth::Set(width)),
                Err(e)     => Err(Misfire::FailedParse(e)),
            }
        }
        else if let Some((width, _)) = dimensions() {
            Ok(TerminalWidth::Terminal(width))
        }
        else {
            Ok(TerminalWidth::Unset)
        }
    }

    fn as_ref(&self) -> Option<&usize> {
        match *self {
            TerminalWidth::Set(ref width)       => Some(width),
            TerminalWidth::Terminal(ref width)  => Some(width),
            TerminalWidth::Unset                => None,
        }
    }
}

impl<'a> From<&'a ArgMatches<'a>> for SizeFormat {

    /// Determine which file size to use in the file size column based on
    /// the user’s options.
    ///
    /// The default mode is to use the decimal prefixes, as they are the
    /// most commonly-understood, and don’t involve trying to parse large
    /// strings of digits in your head. Changing the format to anything else
    /// involves the `--binary` or `--bytes` flags, and these conflict with
    /// each other.
    fn from(matches: &ArgMatches<'a>) -> Self {
        if matches.is_present("binary") {
            SizeFormat::BinaryBytes
        } else if matches.is_present("bytes") {
            SizeFormat::JustBytes
        } else {
            SizeFormat::DecimalBytes
        }
    }
}


/// Under what circumstances we should display coloured, rather than plain,
/// output to the terminal.
///
/// By default, we want to display the colours when stdout can display them.
/// Turning them on when output is going to, say, a pipe, would make programs
/// such as `grep` or `more` not work properly. So the `Automatic` mode does
/// this check and only displays colours when they can be truly appreciated.
#[derive(PartialEq, Debug)]
enum TerminalColours {

    /// Display them even when output isn’t going to a terminal.
    Always,

    /// Display them when output is going to a terminal, but not otherwise.
    Automatic,

    /// Never display them, even when output is going to a terminal.
    Never,
}

impl FromStr for TerminalColours {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match s {
            "always"              => Ok(TerminalColours::Always),
            "auto" | "automatic"  => Ok(TerminalColours::Automatic),
            "never"               => Ok(TerminalColours::Never),
            _                     => unreachable!()
        }
    }
}

impl<'a> From<&'a ArgMatches<'a>> for TerminalColours {
    fn from(matches: &ArgMatches<'a>) -> Self {
        matches.value_of("color").unwrap().parse().unwrap()
    }
}
