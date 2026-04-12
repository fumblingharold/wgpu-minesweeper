use crate::{
    minesweeper::{
        Count,
        Dim,
    },
};
use std::{
    env,
    str::FromStr,
};

// Defaults for game params if left unspecified
const DEFAULT_WIDTH: Dim = 10;
const DEFAULT_HEIGHT: Dim = 10;
const DEFAULT_NUM_MINES: Count = 20;

/// Text printed for --help
const HELP_TEXT: &str = "Usage: minesweeper [OPTION] ...
Launches a game of minesweeper

Options:
--help
\tprints this message
-w --width <grid_width>
\tsets the width of the minesweeper board, defaults to 10
-h --height <grid_height>
\tsets the height of the minesweeper board, defaults to 10
-m --mines <num_mines>
\tsets the number of mines in the minesweeper game, defaults to 20";

/// Parses the given arg and stores the value in var. Should arg be none or parsing fail, returns
/// an [Err] wrapping a string message.
/// That error message is generated using the given flag.
///
/// Note that "returns" returns out of the calling function. This is a macro!
macro_rules! update_val {
    ( $var:ident, $flag:expr, $arg:expr) => {
        match to_val($flag, $arg) {
            Ok(val) => $var = val,
            Err(err) => return Err(wrap_error_msg(err)),
        }
    };
}

/// Wraps an error message with formatting text all error messages should have.
fn wrap_error_msg(msg: String) -> String {
    format!(
        "minesweeper: {}\nTry 'minesweeper --help' for more information.",
        msg
    )
}

/// Parses the given arg and returns the result. Should arg be none or parsing fail, returns an
/// [Err] wrapping a string message.
/// That error message is generated using the given flag.
fn to_val<T: FromStr>(flag: &str, arg: Option<String>) -> Result<T, String> {
    if let Some(arg) = arg {
        arg.parse()
            .map_err(|_| format!("invalid value for flag {}: {}", flag, arg))
    } else {
        Err(format!("missing value for flag {}", flag))
    }
}

/// Gets the starting parameters for minesweeper (width, height, num_mines) from command line
/// arguments or falling back to defaults. If there's an error parsing the command line args or
/// `--help` is passed, returns a message in the form of a string instead.
pub fn get_starting_params() -> Result<(Dim, Dim, Count), String> {
    let mut args = env::args().skip(1);

    // Set defaults
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut num_mines = DEFAULT_NUM_MINES;

    // Loop through args until end, error, or --help
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-w" | "--width" => update_val!(width, &arg, args.next()),
            "-h" | "--height" => update_val!(height, &arg, args.next()),
            "-m" | "--mines" => update_val!(num_mines, &arg, args.next()),
            "--help" => return Err(HELP_TEXT.to_string()),
            _ => return Err(wrap_error_msg(format!("unknown argument: {}", arg))),
        }
    }

    // Return error if grid has too many mines
    if (width as Count * height as Count) <= num_mines as Count {
        return Err(wrap_error_msg(format!(
            "num cells (width * height) less than or equal to num mines: {} ({width} * {height}) <= {num_mines}",
            width * height
        )));
    }

    Ok((width, height, num_mines))
}
