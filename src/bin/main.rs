extern crate exa;
use exa::Exa;

use std::io::{stdout, stderr, Write, ErrorKind};
use std::process::exit;

fn main() {
    let mut stdout = stdout();

    match Exa::new(&mut stdout) {
        Ok(mut exa) => if let Err(e) = exa.run() {
            match e.kind() {
                ErrorKind::BrokenPipe => exit(0),
                _ => {
                    writeln!(stderr(), "{}", e).unwrap();
                    exit(1);
                },
            };
        },
        Err(e) => {
            writeln!(stderr(), "{}", e).unwrap();
            exit(e.error_code());
        },
    };
}
