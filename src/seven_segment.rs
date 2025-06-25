use std::cmp::PartialEq;

/// Represents the two different seven-segment displays.
#[derive(Debug)]
pub enum Display {
    MinesUnflagged,
    Timer,
}

/// Represents all the possibilities for a digit on a seven-segment display.
///
/// A seven-segment display can, of course, display more than these, but this is all that's needed for minesweeper.
#[derive(PartialEq, Debug)]
pub enum Image {
    Blank,
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Negative,
}

impl Image {
    /// Creates an [Image] from number.
    ///
    /// Panics if given an invalid number.
    fn from_value(value: u8) -> Image {
        use Image::*;
        match value {
            0 => Zero,
            1 => One,
            2 => Two,
            3 => Three,
            4 => Four,
            5 => Five,
            6 => Six,
            7 => Seven,
            8 => Eight,
            9 => Nine,
            _ => panic!("Invalid number: {}", value),
        }
    }

    /// Gives the texture coordinates
    pub fn get_tex_coords(image: &Image) -> [f32; 2] {
        use Image::*;
        match image {
            Negative => [0.0,  0.0],
            Blank    => [0.0,  1.0 / 12.0],
            Nine     => [0.0,  2.0 / 12.0],
            Eight    => [0.0,  3.0 / 12.0],
            Seven    => [0.0,  4.0 / 12.0],
            Six      => [0.0,  5.0 / 12.0],
            Five     => [0.0,  6.0 / 12.0],
            Four     => [0.0,  7.0 / 12.0],
            Three    => [0.0,  8.0 / 12.0],
            Two      => [0.0,  9.0 / 12.0],
            One      => [0.0, 10.0 / 12.0],
            Zero     => [0.0, 11.0 / 12.0],
        }
    }
}

/// Gives the [Image]s to be displayed for the given value.
fn get_images(val: i32) -> [Image; 3]{
    if val >= 999 {
        [Image::Nine, Image::Nine, Image::Nine]
    } else if val <= -99 {
        [Image::Negative, Image::Nine, Image::Nine]
    } else {
        let mag_val = cgmath::num_traits::abs(val) as u16 % 999;
        let (digit_100s, digit_10s, digit_1s) =
            ((mag_val / 100) as u8, (mag_val / 10) as u8 % 10, mag_val as u8 % 10);
        let result_1s = Image::from_value(digit_1s);
        let result_10s =
            if mag_val > 9 {
                Image::from_value(digit_10s)
            } else if val < 0 {
                Image::Negative
            } else {
                Image::Blank
            };
        let result_100s =
            if mag_val > 9 && val < 0 {
                Image::Negative
            } else if val < 0 || mag_val < 100 {
                Image::Blank
            } else {
                Image::from_value(digit_100s)
            };
        [result_100s, result_10s, result_1s]
    }
}

/// Gives the texture coordinates to be used for rendering the given value.
pub fn get_texture_coords(val: i32) -> [[f32; 2]; 3] {
    let mut result = get_images(val)
        .into_iter()
        .map(|image| Image::get_tex_coords(&image));
    [result.next().unwrap(), result.next().unwrap(), result.next().unwrap()]
}

/// Gives the texture coordinates that need to be updated for rendering the given new and previous values.
pub fn get_updated_texture_coords(new: i32, old: i32) -> Vec<Option<[f32; 2]>> {
    let new = get_images(new);
    let old = get_images(old);
    new.iter().zip(old.iter()).map(|(new, old)|
        if new == old { None } else { Some(Image::get_tex_coords(new)) }
    ).collect()
}