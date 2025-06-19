use std::cmp::PartialEq;
use std::ops::{Index, IndexMut};
use rand::Rng;

/// All the different textures a Cell can have.
#[derive(Clone, Debug, PartialEq)]
pub enum CellImage {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Mine,
    WronglyFlagged,
    SelectedMine,
    Hidden,
    Flagged,
    QuestionMarked,
}

impl CellImage {
    /// Converts a number to the CellImage with that number.
    ///
    /// Panics if the number is >8 as it doesn't have an equivalent CellImage.
    fn from_number(num: u8) -> CellImage {
        match num {
            0 => CellImage::Zero,
            1 => CellImage::One,
            2 => CellImage::Two,
            3 => CellImage::Three,
            4 => CellImage::Four,
            5 => CellImage::Five,
            6 => CellImage::Six,
            7 => CellImage::Seven,
            8 => CellImage::Eight,
            _ => panic!("Invalid number: {}", num),
        }
    }

    /// Whether the given CellImage is a shown texture. Shown textures represent cells that have been revealed.
    fn shown(&self) -> bool {
        match self {
            CellImage::Hidden         => false,
            CellImage::Flagged        => false,
            CellImage::QuestionMarked => false,
            _                         => true,
        }
    }
}

/// A cell in the minesweeper grid. Keeps track of the cells current texture and whether it is a mine.
#[derive(Clone, Debug)]
struct Cell {
    image: CellImage,
    mine: bool,
}

/// The state of a minesweeper game. Different states allow different interactions and have different guarantees.
/// All states permit resetting at any time, which sets the state to [`GameState::BeforeGame`].
#[derive(Debug, PartialEq)]
enum GameState {
    /// Allows only left-clicking on the game. Guarantees the clicked [`Cell`] will be safe.
    /// This interaction starts the game: generates the grid if needed, places the mines,
    /// and transitions the [`GameState`] to [`GameState::DuringGame`].
    BeforeGame,
    /// Allows all interactions with the game. Ending the game transitions the [`GameState`] to [`GameState::AfterGame`].
    DuringGame,
    /// Prevents all interactions with the game.
    AfterGame,
}

/// Represents the grid of [`Cell`]s. Stored as a 2D vector of [`Cells`] and indexed using [`u8`] because of
/// obvious usability issues in minesweeper grid size >255x255.
#[derive(Debug)]
struct GameGrid {
    data: Vec<Vec<Cell>>,
}

impl GameGrid {
    /// Resizes to the given width and height. Fills the grid with [`Cell`]s where `mine = false`
    /// and texture [`CellImage::Hidden`].
    fn resize(&mut self, width: u8, height: u8) {
        if height != self.height() || width != self.width() {
            let cell = Cell { image: CellImage::Hidden, mine: false };
            self.data = vec![vec![cell; width as usize]; height as usize];
        }
    }

    /// Gives the width of the grid.
    fn width(&self) -> u8 {
        if self.height() == 0 { 0 } else { self.data[0].len() as u8 }
    }

    /// Gives the height of the grid.
    pub fn height(&self) -> u8 {
        self.data.len() as u8
    }
}

impl Index<(u8, u8)> for GameGrid {
    type Output = Cell;

    fn index(&self, (row, col): (u8, u8)) -> &Self::Output {
        &self.data[row as usize][col as usize]
    }
}

impl IndexMut<(u8, u8)> for GameGrid {
    fn index_mut(&mut self, (row, col): (u8, u8)) -> &mut Self::Output {
        &mut self.data[row as usize][col as usize]
    }
}

/// A game of minesweeper. Width and height are stored as [`u8`] because of obvious usability
/// issues in minesweeper grid size >255x255. Flags, hidden, and total_mines are [`u16`] to
/// account for this.
#[derive(Debug)]
pub struct Game {
    grid: GameGrid,
    game_state: GameState,
    width: u8,
    height: u8,
    flags: u16,
    hidden: u16,
    total_mines: u16,
}

impl Game {
    /// Creates a new game of minesweeper with the given dimensions and number of mines. Panics if the inputs are invalid.
    pub fn new(width: u8, height: u8, mines: u16) -> Self {
        assert!(width as u16 * height as u16 > mines && width != 0 && height != 0 && mines != 0, "Invalid grid");
        Game {
            grid: GameGrid { data: Vec::new() },
            game_state: GameState::BeforeGame,
            width,
            height,
            flags: 0,
            hidden: width as u16 * height as u16,
            total_mines: mines,
        }
    }

    /// Resets the game and resizes the grid to the given inputs.
    pub fn resize(&mut self, width: u8, height: u8, num_mines: u16) {
        self.reset();
        self.width = width;
        self.height = height;
        self.total_mines = num_mines;
    }

    /// Resets the game.
    pub fn reset(&mut self) {
        self.game_state = GameState::BeforeGame;
    }

    /// Performs the left click operations for minesweeper. Reveals the given [`Cell`] if it has the image
    /// [`CellImage::Hidden`] or all the [`Cell`]s with image [`CellImage::Hidden`] around the given [`Cell`]
    /// if it is shown. Does not perform any actions if the [`GameState`] is [`GameState::AfterGame`].
    pub fn left_click(&mut self, pos: (u8, u8)) -> Vec<(u8, u8, CellImage)> {
        assert!(pos.0 < self.height() && pos.1 < self.width(), "left_click invalid location: {:?}", pos);
        let mut result = Vec::new();
        if self.game_state == GameState::BeforeGame {
            self.start_game(pos);
        }
        let cell = &mut self.grid[pos];
        if self.game_state == GameState::DuringGame {
            if cell.image == CellImage::Hidden {
                result = self.show(vec!(pos));
            } else if !cell.image.shown() {
                result.push(self.toggle_tofrom_question_marked(pos));
            } else {
                result = self.show(self.get_hidden_neighbors(pos));
            }
            if self.hidden == self.total_mines {
                result.append(&mut self.handle_win());
            }
        }
        result
    }

    /// Performs the right click operations for minesweeper. This toggles [`Cell`]s images when
    /// hidden from [`CellImage::Hidden`] to [`CellImage::Flagged`] and other hidden values to
    /// [`CellImage::Hidden`].
    pub fn right_click(&mut self, pos: (u8, u8)) -> Vec<(u8, u8, CellImage)> {
        assert!(pos.0 < self.height() && pos.1 < self.width(), "toggle_flag invalid location");
        // Does nothing if the cell is shown, otherwise toggle the flag
        if self.game_state == GameState::BeforeGame
            || self.game_state == GameState::AfterGame
            || self.grid[pos].image.shown() {
            Vec::new()
        } else {
            vec!(self.toggle_tofrom_hidden(pos))
        }
    }

    /// Reveal the given [`Cell`]s and returns a list of tuples giving the row, column, and
    /// [`CellImage`] for every [`Cell`] texture updated. Performs 0 propagation.
    fn show(&mut self, mut cells: Vec<(u8, u8)>) -> Vec<(u8, u8, CellImage)> {
        // If any of the cells are mines, end the game
        for pos in cells.iter_mut() { // Check if each cell is a mine
            let cell = &mut self.grid[*pos];
            // If the cell is a mine that would be shown, end the game
            if cell.mine {
                self.game_state = GameState::AfterGame;
                cell.image = CellImage::SelectedMine;
                let mut result = vec!((pos.0, pos.1, CellImage::SelectedMine));
                for row in 0..self.height() {
                    for col in 0..self.width() {
                        let cell = &mut self.grid[(row, col)];
                        if cell.mine && cell.image == CellImage::Hidden {
                            cell.image = CellImage::Mine;
                            result.push((row, col, cell.image.clone()));
                        } else if !cell.mine && cell.image == CellImage::Flagged {
                            cell.image = CellImage::WronglyFlagged;
                            result.push((row, col, cell.image.clone()));
                        }
                    }
                };
                return result
            }
        }
        // If it isn't a mine, add it to the list of cells to reveal
        let mut result = vec!();
        // Reveal cells in stack until empty
        while !cells.is_empty() {
            let pos = cells.pop().unwrap();
            // If the cell isn't hidden, ignore it
            if self.grid[pos].image != CellImage::Hidden {
                continue;
            }
            self.hidden -= 1;
            // Change the cells image to reflect the number of mines around it
            let mines_around = self.get_mines_around(pos);
            self.grid[pos].image = CellImage::from_number(mines_around);
            result.push((pos.0, pos.1, CellImage::from_number(mines_around)));
            // If the cell is a 0, add its neighbors to the stack
            if mines_around == 0 {
                cells.append(&mut self.get_hidden_neighbors(pos));
            }
        }
        result
    }

    /// Toggles the given [`Cell`] to [`CellImage::Hidden`] if it is anything else and to
    /// [`CellImage::Flagged`] if it is [`CellImage::Hidden`].
    fn toggle_tofrom_hidden(&mut self, pos: (u8, u8)) -> (u8, u8, CellImage) {
        self.toggle_tofrom_given(pos, CellImage::Hidden)
    }

    /// Toggles the given [`Cell`] to [`CellImage::QuestionMarked`] if it is anything else and to
    /// [`CellImage::Flagged`] if it is [`CellImage::QuestionMarked`].
    fn toggle_tofrom_question_marked(&mut self, pos: (u8, u8)) -> (u8, u8, CellImage) {
        self.toggle_tofrom_given(pos, CellImage::QuestionMarked)
    }

    /// Toggles the given [`Cell`] to the given [`CellImage`] if it is anything else and to
    /// [`CellImage::Flagged`] if it is the given [`CellImage`].
    fn toggle_tofrom_given(&mut self, (row, col): (u8, u8), given: CellImage) -> (u8, u8, CellImage) {
        assert!(row < self.height() && col < self.width(), "invalid location");
        //let mut cell = &mut self.grid[row as usize][col as usize];
        let cell =  &mut self.grid[(row, col)];
        (row, col,
         if cell.image == given {
             cell.image = CellImage::Flagged;
             self.flags += 1;
             CellImage::Flagged
         } else {
             if cell.image == CellImage::Flagged {
                 self.flags -= 1;
             }
             cell.image = given.clone();
             given
         }
        )
    }

    fn handle_win(&mut self) -> Vec<(u8, u8, CellImage)> {
        let mut result = Vec::new();
        for row in 0..self.height() {
            for col in 0..self.width() {
                let cell = &mut self.grid[(row, col)];
                if cell.mine && cell.image != CellImage::Flagged {
                    cell.image = CellImage::Flagged;
                    result.push((row, col, cell.image.clone()));
                }
            }
        }
        result
    }

    /// Gives the width of the game.
    pub fn width(&self) -> u8 {
        self.width
    }

    /// Gives the height of the game.
    pub fn height(&self) -> u8 {
        self.height
    }

    /// Returns a 2D vector of [`CellImage`]s matching up with each [`Cell`]'s texture.
    pub fn get_all_images(&self) -> Vec<Vec<CellImage>> {
        let mut result = Vec::with_capacity(self.height() as usize);
        if self.game_state == GameState::BeforeGame {
            for _ in 0..self.height() {
                let mut row = Vec::with_capacity(self.width() as usize);
                for _ in 0..self.width() {
                    row.push(CellImage::Hidden);
                }
                result.push(row);
            }
        } else {
            self.grid.data.iter().for_each(|row| {
                let mut row_image = Vec::with_capacity(self.width() as usize);
                row.iter().for_each(|cell| {
                    row_image.push(cell.image.clone());
                });
                result.push(row_image);
            });
        }
        result
    }

    /// Starts the game of minesweeper: resizes the grid to widthxheight, fills the grid with
    /// mines, and changes the [`GameState`] to [`GameState::DuringGame`]. A mine will never be
    /// placed in the given row and col and the surrounding [`cell`]s will be avoided if possible.
    fn start_game(&mut self, (row, col): (u8, u8)) {
        self.game_state = GameState::DuringGame;
        self.hidden = self.height() as u16 * self.width() as u16;
        self.flags = 0;
        let width = self.width();
        let height = self.height();
        //If the grid is the wrong size, resize it
        self.grid.resize(width, height);
        // Finds all cells that should not be mines
        let mut safe_cells = self.get_3x3((row, col));
        safe_cells.iter().for_each(|pos| self.grid[*pos].image = CellImage::Hidden);
        // Remove cells from safe array if needed to get desired number of mines
        let mut cells_remaining = self.hidden - safe_cells.len() as u16;
        let mut mines_remaining = self.total_mines;
        let mut rng = rand::thread_rng();
        let (first_special_row, first_special_col) = safe_cells[0];
        let (last_special_row, last_special_col) = *safe_cells.iter().max().unwrap();
        let (next_normal_row, next_normal_col) = (last_special_row + 1, last_special_col + 1);
        if cells_remaining < mines_remaining {
            let cells_to_make_unsafe = mines_remaining - cells_remaining;
            mines_remaining = cells_remaining;
            for _ in 0..cells_to_make_unsafe {
                let index = rng.gen_range(0..(safe_cells.len() - 1));
                let index_to_be_mine =
                    if safe_cells[index] == (row, col) {
                        safe_cells.len() - 1
                    } else {
                        index
                    };
                self.grid[safe_cells.swap_remove(index_to_be_mine)].mine = true;
            }
        }
        safe_cells.iter().for_each(|pos| self.grid[*pos].mine = false);
        // Place mines in grid and reset all cells to be hidden
        let mut fill_with_mines = move | row_range, col_range: core::ops::Range<u8> | {
            for row in row_range {
                for col in col_range.clone() {
                    let cell = &mut self.grid[(row, col)];
                    let is_mine = rng.gen_range(0..cells_remaining) < mines_remaining;
                    cell.image = CellImage::Hidden;
                    cell.mine = is_mine;
                    cells_remaining -= 1;
                    if is_mine {
                        mines_remaining -= 1;
                    }
                }
            }
        };
        fill_with_mines(0..first_special_row, 0..width);
        fill_with_mines(first_special_row..next_normal_row, 0..first_special_col);
        fill_with_mines(first_special_row..next_normal_row, next_normal_col..width);
        fill_with_mines(next_normal_row..height, 0..width);
    }

    /// Returns the locations of all adjacent [`Cell`]s with [`CellImage::Hidden`].
    fn get_hidden_neighbors(&self, pos: (u8, u8)) -> Vec<(u8, u8)> {
        self.get_neighbors(pos)
            .into_iter()
            .filter(|pos| self.grid[*pos].image == CellImage::Hidden)
            .collect()
    }

    /// Returns the locations of all adjacent [`Cell`]s.
    fn get_neighbors(&self, (row, col): (u8, u8)) -> Vec<(u8, u8)> {
        let mut result = self.get_3x3((row, col));
        for index in 0..result.len() {
            if result[index] == (row, col) {
                result.swap_remove(index);
                break;
            }
        }
        result
    }

    /// Returns the locations of all adjacent [`Cell`]s and the [`Cell`] itself.
    fn get_3x3(&self, (row, col): (u8, u8)) -> Vec<(u8, u8)> {
        let mut result = Vec::with_capacity(8);
        let row = row as i16;
        let col = col as i16;
        let height = self.height() as i16;
        let width = self.width() as i16;
        for row_difference in -1..=1 {
            let neighbor_row = row + row_difference;
            if neighbor_row >= 0 && neighbor_row < height {
                for col_difference in -1..=1 {
                    let neighbor_col = col + col_difference;
                    if neighbor_col >= 0 && neighbor_col < width {
                        result.push((neighbor_row as u8, neighbor_col as u8))
                    }
                }
            }
        }
        result
    }

    /// Finds the number of mines surrounding the [`Cell`] at the given row and col.
    /// Technically includes the cell in the count but since this function should never be called
    /// on a mine that should never cause issues.
    fn get_mines_around(&self, pos: (u8, u8)) -> u8 {
        let mut num_mines = 0;
        for pos in self.get_3x3(pos) {
            if self.grid[pos].mine {
                num_mines += 1;
            }
        }
        num_mines
    }
}