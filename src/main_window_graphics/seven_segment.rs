use std::cmp::PartialEq;
use std::mem;

pub const DIGIT_WIDTH: u16 = 13;
pub const DIGIT_HEIGHT: u16 = 23;
pub(crate) const DIGITS_PER_DISPLAY: usize = 3;

/// Represents the two different seven-segment displays.
#[derive(Debug, PartialEq, Clone)]
pub enum Display {
    MinesUnflagged,
    Timer,
}

/// Represents all the possibilities for a digit on a seven-segment display.
///
/// A seven-segment display can, of course, display more than these, but this is all that's needed
/// for minesweeper.
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
enum Image {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Blank = 10,
    Negative = 11,
}

impl Image {
    /// Creates an [Image] from number.
    ///
    /// Panics if given an invalid number.
    fn from_value(value: u8) -> Image {
        if value > 9 {
            panic!("Invalid number: {}", value);
        }
        // SAFETY: The value is in the range 0-9, so it can be cast to Image.
        unsafe { mem::transmute(value) }
    }

    /// Gives the texture coordinates
    fn get_tex_coords(image: Image) -> [u16; 2] {
        [
            (image as u16 & 0b11) * DIGIT_WIDTH,
            (image as u16 >> 2) * DIGIT_HEIGHT,
        ]
    }
}

/// Gives the [Image]s to be displayed for the given value.
fn get_images(val: i32) -> [Image; 3] {
    use Image::*;
    if val >= 999 {
        [Nine, Nine, Nine]
    } else if val <= -99 {
        [Negative, Nine, Nine]
    } else {
        let mag_val = cgmath::num_traits::abs(val) as u16 % 999;
        let (digit_100s, digit_10s, digit_1s) = (
            (mag_val / 100) as u8,
            (mag_val / 10) as u8 % 10,
            mag_val as u8 % 10,
        );
        let result_1s = Image::from_value(digit_1s);
        let result_10s = if mag_val > 9 {
            Image::from_value(digit_10s)
        } else if val < 0 {
            Negative
        } else {
            Blank
        };
        let result_100s = if mag_val > 9 && val < 0 {
            Negative
        } else if val < 0 || mag_val < 100 {
            Blank
        } else {
            Image::from_value(digit_100s)
        };
        [result_100s, result_10s, result_1s]
    }
}

/// Gives the texture coordinates to be used for rendering the given value.
pub fn get_texture_coords(val: i32) -> [[u16; 2]; 3] {
    let mut result = get_images(val)
        .into_iter()
        .map(|image| Image::get_tex_coords(image));
    [
        result.next().unwrap(),
        result.next().unwrap(),
        result.next().unwrap(),
    ]
}
