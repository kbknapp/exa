use colours::Colours;
use file::File;

use super::filename;


#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lines {
    pub colours: Colours,
}

/// The lines view literally just displays each file, line-by-line.
impl Lines {
    pub fn view(&self, files: &[File]) {
        for file in files {
            println!("{}", filename(file, &self.colours, true));
        }
    }
}
