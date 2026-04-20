use crate::minesweeper::{
    Count,
    Dim,
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
\tsets the number of mines in the minesweeper game, defaults to 20
\tcannot be used if --percent-mines is also used
--percent_mines <percent_mines>
\tsets what percent of the board will be mines
\tcannot be used if -m or --mines is also used";

/// Wraps an error message with formatting text all error messages should have.
fn wrap_error_msg(msg: String) -> String {
    format!("minesweeper: {msg}\nTry 'minesweeper --help' for more information.")
}

/// A value read from the command line.
/// Contains methods for updating the value given a [String] with the new value.
/// Only intended to be written to once since writing to it multiple times would indicate multiple
/// uses of equivalent flags. (i.e. a use of both `-w` and `--width`)
struct ArgValue<T: FromStr> {
    name: &'static str,
    value: Option<T>,
    is_set: bool,
}

impl<T: FromStr> ArgValue<T> {
    /// Creates a new [ArgValue] with the given name and default value.
    /// Should there be no default value, it can be set to None.
    fn new(name: &'static str, value: Option<T>) -> Self {
        Self {
            name,
            value,
            is_set: false,
        }
    }

    /// Updates the [ArgValue] using the value from parsing the given [String].
    /// Returns an error if `self` has already been updated before, if `arg` is [None], or if `arg`
    /// cannot be parsed.
    fn update(&mut self, flag: &str, arg: Option<String>) -> Result<(), String> {
        if self.is_set {
            Err(format!("{} already set", self.name))
        } else if let Some(arg) = arg {
            match arg.parse() {
                Ok(val) => {
                    self.value = Some(val);
                    self.is_set = true;
                    Ok(())
                }
                Err(_) => Err(format!("invalid value for flag {flag}: {arg}")),
            }
        } else {
            Err(format!("no value provided for flag {flag}"))
        }
    }
}

/// Gets the starting parameters for minesweeper (width, height, num_mines) from command line
/// arguments or falling back to defaults. If there's an error parsing the command line args or
/// `--help` is passed, returns a message in the form of a string instead.
pub fn get_starting_params() -> Result<(Dim, Dim, Count), String> {
    // Get cmd line args, skipping program name
    let mut args = env::args().skip(1);

    // Set defaults
    let mut width = ArgValue::new("width", Some(DEFAULT_WIDTH));
    let mut height = ArgValue::new("height", Some(DEFAULT_HEIGHT));
    let mut num_mines = ArgValue::new("num_mines", Some(DEFAULT_NUM_MINES));
    let mut percent_mines: ArgValue<f32> = ArgValue::new("percent_mines", None);

    // Loop through args until end, error, or --help
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-w" | "--width" => width.update(&arg, args.next()),
            "-h" | "--height" => height.update(&arg, args.next()),
            "-m" | "--mines" => num_mines.update(&arg, args.next()),
            "-p" | "--percent-mines" => percent_mines.update(&arg, args.next()),
            "--help" => return Err(HELP_TEXT.to_string()), // returns to prevent error wrapping
            _ => Err(format!("unknown argument: {}", arg)),
        }
        .map_err(wrap_error_msg)? // wrap all error messages with some standard text
    }

    // Return an error if both num_mines and percent_mines were set with command line args
    if num_mines.is_set && percent_mines.is_set {
        return Err(wrap_error_msg(
            "cannot set both num_mines and percent_mines".to_string(),
        ));
    }

    // Get values for width and height for ease of use
    let width = width.value.unwrap();
    let height = height.value.unwrap();

    // Calculate num_mines using percent_mines if it was set
    // Otherwise, use value in num_mines
    let num_mines = if let Some(val) = percent_mines.value {
        f32::round(width as f32 * height as f32 * val / 100.0) as Count
    } else {
        num_mines.value.unwrap()
    };

    // Return error if any value is too small
    if width <= 7 {
        Err(format!("width must be greater than 7: {}", width))
    } else if height == 0 {
        Err(format!("height must be greater than 0: {}", height))
    } else if num_mines == 0 {
        Err(format!("num mines must be greater than 0: {}", num_mines))
    } else {
        Ok(())
    }
    .map_err(wrap_error_msg)?;

    // Return error if grid has too many mines
    if (width as Count * height as Count) <= num_mines as Count {
        let num_cells = width as Count * height as Count;
        return Err(wrap_error_msg(format!(
            "num_mines must be less than num cells (width * height): \
            {num_mines} < {num_cells} ({width} * {height})"
        )));
    }

    Ok((width, height, num_mines))
}
