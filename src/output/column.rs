use std::str::FromStr;

use clap::ArgMatches;

use fs::Dir;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Column {
    Permissions,
    FileSize(SizeFormat),
    Timestamp(TimeType),
    Blocks,
    User,
    Group,
    HardLinks,
    Inode,

    GitStatus,
}

/// Each column can pick its own **Alignment**. Usually, numbers are
/// right-aligned, and text is left-aligned.
#[derive(Copy, Clone)]
pub enum Alignment {
    Left, Right,
}

impl Column {

    /// Get the alignment this column should use.
    pub fn alignment(&self) -> Alignment {
        match *self {
            Column::FileSize(_) => Alignment::Right,
            Column::HardLinks   => Alignment::Right,
            Column::Inode       => Alignment::Right,
            Column::Blocks      => Alignment::Right,
            Column::GitStatus   => Alignment::Right,
            _                   => Alignment::Left,
        }
    }

    /// Get the text that should be printed at the top, when the user elects
    /// to have a header row printed.
    pub fn header(&self) -> &'static str {
        match *self {
            Column::Permissions   => "Permissions",
            Column::FileSize(_)   => "Size",
            Column::Timestamp(t)  => t.header(),
            Column::Blocks        => "Blocks",
            Column::User          => "User",
            Column::Group         => "Group",
            Column::HardLinks     => "Links",
            Column::Inode         => "inode",
            Column::GitStatus     => "Git",
        }
    }
}


#[derive(PartialEq, Copy, Clone, Debug, Default)]
pub struct Columns {
    pub size_format: SizeFormat,
    pub time_types: TimeTypes,
    pub inode: bool,
    pub links: bool,
    pub blocks: bool,
    pub group: bool,
    pub git: bool
}

impl Columns {
    pub fn should_scan_for_git(&self) -> bool {
        self.git
    }

    pub fn for_dir(&self, dir: Option<&Dir>) -> Vec<Column> {
        let mut columns = vec![];

        if self.inode {
            columns.push(Column::Inode);
        }

        columns.push(Column::Permissions);

        if self.links {
            columns.push(Column::HardLinks);
        }

        columns.push(Column::FileSize(self.size_format));

        if self.blocks {
            columns.push(Column::Blocks);
        }

        columns.push(Column::User);

        if self.group {
            columns.push(Column::Group);
        }

        if self.time_types.modified {
            columns.push(Column::Timestamp(TimeType::Modified));
        }

        if self.time_types.created {
            columns.push(Column::Timestamp(TimeType::Created));
        }

        if self.time_types.accessed {
            columns.push(Column::Timestamp(TimeType::Accessed));
        }

        if cfg!(feature="git") {
            if let Some(d) = dir {
                if self.should_scan_for_git() && d.has_git_repo() {
                    columns.push(Column::GitStatus);
                }
            }
        }

        columns
    }
}

impl<'a> From<&'a ArgMatches<'a>> for Columns {
    fn from(matches: &ArgMatches<'a>) -> Self {
        Columns {
            size_format: SizeFormat::from(matches),
            time_types:  TimeTypes::from(matches),
            inode:  matches.is_present("inode"),
            links:  matches.is_present("links"),
            blocks: matches.is_present("blocks"),
            group:  matches.is_present("group"),
            git:    cfg!(feature="git") && matches.is_present("git"),
        }
    }
}

/// Formatting options for file sizes.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum SizeFormat {

    /// Format the file size using **decimal** prefixes, such as “kilo”,
    /// “mega”, or “giga”.
    DecimalBytes,

    /// Format the file size using **binary** prefixes, such as “kibi”,
    /// “mebi”, or “gibi”.
    BinaryBytes,

    /// Do no formatting and just display the size as a number of bytes.
    JustBytes,
}

impl Default for SizeFormat {
    fn default() -> SizeFormat {
        SizeFormat::DecimalBytes
    }
}

/// The types of a file’s time fields. These three fields are standard
/// across most (all?) operating systems.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum TimeType {

    /// The file’s accessed time (`st_atime`).
    Accessed,

    /// The file’s modified time (`st_mtime`).
    Modified,

    /// The file’s creation time (`st_ctime`).
    Created,
}

impl TimeType {

    /// Returns the text to use for a column’s heading in the columns output.
    pub fn header(&self) -> &'static str {
        match *self {
            TimeType::Accessed  => "Date Accessed",
            TimeType::Modified  => "Date Modified",
            TimeType::Created   => "Date Created",
        }
    }
}

/// Fields for which of a file’s time fields should be displayed in the
/// columns output.
///
/// There should always be at least one of these--there's no way to disable
/// the time columns entirely (yet).
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct TimeTypes {
    pub accessed: bool,
    pub modified: bool,
    pub created:  bool,
}

impl<'a> From<&'a ArgMatches<'a>> for TimeTypes {

    /// Determine which of a file’s time fields should be displayed for it
    /// based on the user’s options.
    ///
    /// There are two separate ways to pick which fields to show: with a
    /// flag (such as `--modified`) or with a parameter (such as
    /// `--time=modified`). An error is signaled if both ways are used.
    ///
    /// It’s valid to show more than one column by passing in more than one
    /// option, but passing *no* options means that the user just wants to
    /// see the default set.
    fn from(matches: &ArgMatches<'a>) -> TimeTypes {
        let modified = matches.is_present("modified");
        let created  = matches.is_present("created");
        let accessed = matches.is_present("accessed");

        if let Some(word) = matches.value_of("time") {
            word.parse().unwrap()
        }
        else {
            TimeTypes { accessed: accessed, modified: modified, created: created }
        }
    }
}

impl Default for TimeTypes {
    fn default() -> Self {
        TimeTypes { accessed: false, modified: true,  created: false }       
    }
}

impl FromStr for TimeTypes {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match s {
            "mod" | "modified"  => Ok(TimeTypes { accessed: false, modified: true,  created: false }),
            "acc" | "accessed"  => Ok(TimeTypes { accessed: true,  modified: false, created: false }),
            "cr"  | "created"   => Ok(TimeTypes { accessed: false, modified: false, created: true  }),
            _                   => unreachable!()
        }
    }
}
