use std::cmp::PartialEq;
use std::ops::{Index, IndexMut};
use rand::Rng;
use crate::minesweeper::GameState::DuringGame;

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

    fn shown(&self) -> bool {
        match self {
            CellImage::Hidden         => false,
            CellImage::Flagged        => false,
            CellImage::QuestionMarked => false,
            _                         => true,
        }
    }
}

#[derive(Clone, Debug)]
struct Cell {
    image: CellImage,
    mine: bool,
    mines_around: Option<u8>,
}

#[derive(Debug, PartialEq)]
enum GameState {
    BeforeGame,
    DuringGame,
    AfterGame,
}

#[derive(Debug)]
struct GameGrid {
    data: Vec<Vec<Cell>>,
}

impl Index<(u32, u32)> for GameGrid {
    type Output = Cell;

    fn index(&self, (row, col): (u32, u32)) -> &Self::Output {
        &self.data[row as usize][col as usize]
    }
}

impl IndexMut<(u32, u32)> for GameGrid {
    fn index_mut(&mut self, (row, col): (u32, u32)) -> &mut Self::Output {
        &mut self.data[row as usize][col as usize]
    }
}

#[derive(Debug)]
pub struct Game {
    grid: GameGrid,
    game_state: GameState,
    flags: u64,
    hidden: u64,
    total_mines: u64,
}

impl Game {
    // Creates a new Game of Minesweeper with the given size and number of mines.
    // Returns None if the inputs cannot create a valid game
    pub fn new(width: u32, height: u32, mines: u64) -> Option<Self> {
        if width as u64 * height as u64 <= mines || width == 0 || height == 0 || mines == 0 {
            return None
        }
        let empty_cell = Cell { image: CellImage::Hidden, mine: false, mines_around: None };
        let row = vec![empty_cell; width as usize];
        let grid = GameGrid { data: vec![row; height as usize] };
        Some(Game {
            grid,
            game_state: GameState::BeforeGame,
            flags: 0,
            hidden: width as u64 * height as u64,
            total_mines: mines,
        })
    }

    pub fn reset(&mut self) {
        self.game_state = GameState::BeforeGame;
    }

    // Performs the left click operations for minesweeper
    pub fn left_click(&mut self, pos: (u32, u32)) -> Vec<(u32, u32, CellImage)> {
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
                self.game_state = GameState::AfterGame;
            }
        }
        println!("(hidden, flags): ({}, {})", self.hidden, self.flags);
        result
    }

    // Performs the right click operations for minesweeper
    pub fn right_click(&mut self, pos: (u32, u32)) -> Vec<(u32, u32, CellImage)> {
        assert!(pos.0 < self.height() && pos.1 < self.width(), "toggle_flag invalid location");
        // Does nothing if the cell is shown, otherwise toggle the flag
        if self.game_state == GameState::AfterGame || self.grid[pos].image.shown() {
            Vec::new()
        } else {
            vec!(self.toggle_tofrom_hidden(pos))
        }
    }

    // Changes the given Cell to show it
    // Updates the value of mines_around before showing
    // Performs 0 propagation
    fn show(&mut self, mut cells: Vec<(u32, u32)>) -> Vec<(u32, u32, CellImage)> {
        // If any of the cells are mines, end the game
        for pos in cells.iter_mut() { // Check if each cell is a mine
            let cell = &mut self.grid[*pos];
            // If the cell is a mine that would be shown, end the game
            if cell.mine {
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

    fn toggle_tofrom_hidden(&mut self, pos: (u32, u32)) -> (u32, u32, CellImage) {
        self.toggle_tofrom_given(pos, CellImage::Hidden)
    }

    fn toggle_tofrom_question_marked(&mut self, pos: (u32, u32)) -> (u32, u32, CellImage) {
        self.toggle_tofrom_given(pos, CellImage::QuestionMarked)
    }

    fn toggle_tofrom_given(&mut self, (row, col): (u32, u32), given: CellImage) -> (u32, u32, CellImage) {
        assert!(row < self.height() && col < self.width(), "invalid location");
        //let mut cell = &mut self.grid[row as usize][col as usize];
        let mut cell =  &mut self.grid[(row, col)];
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

    // Width of the grid
    pub fn width(&self) -> u32 {
        self.grid.data[0].len() as u32
    }

    // Height of the grid
    pub fn height(&self) -> u32 {
        self.grid.data.len() as u32
    }

    pub fn print(&self) {
        self.grid.data.iter().for_each(|row| {
            row.iter().for_each(|cell| {
                match cell.image {
                    CellImage::Zero => print!("0"),
                    CellImage::One => print!("1"),
                    CellImage::Two => print!("2"),
                    CellImage::Three => print!("3"),
                    CellImage::Four => print!("4"),
                    CellImage::Five => print!("5"),
                    CellImage::Six => print!("6"),
                    CellImage::Seven => print!("7"),
                    CellImage::Eight => print!("8"),
                    CellImage::Mine => print!("M"),
                    CellImage::WronglyFlagged => print!("X"),
                    CellImage::SelectedMine => print!("M"),
                    CellImage::Hidden => print!("_"),
                    CellImage::Flagged => print!("F"),
                    CellImage::QuestionMarked => print!("?"),
                };
            });
            println!();
        })
    }

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

    fn start_game(&mut self, (row, col): (u32, u32)) {
        self.game_state = DuringGame;
        self.hidden = self.height() as u64 * self.width() as u64;
        self.flags = 0;
        let width = self.width();
        let height = self.height();
        // Finds all cells that should not be mines
        let mut safe_cells = self.get_3x3((row, col));
        safe_cells.iter().for_each(|pos| self.grid[*pos].image = CellImage::Hidden);
        // Remove cells from safe array if needed to get desired number of mines
        let mut cells_remaining = self.hidden - safe_cells.len() as u64;
        let mut mines_remaining = self.total_mines;
        let mut rng = rand::thread_rng();
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
        let mut fill_with_mines = move | row_range, col_range: core::ops::Range<u32> | {
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
        let (first_special_row, first_special_col) = safe_cells[0];
        let (last_special_row, last_special_col) = *safe_cells.iter().max().unwrap();
        let (next_normal_row, next_normal_col) = (last_special_row + 1, last_special_col + 1);
        fill_with_mines(0..first_special_row, 0..width);
        fill_with_mines(first_special_row..next_normal_row, 0..first_special_col);
        fill_with_mines(first_special_row..next_normal_row, next_normal_col..width);
        fill_with_mines(next_normal_row..height, 0..width);
    }

    // Returns a vector containing the locations of all the hidden adjacent cells
    fn get_hidden_neighbors(&self, pos: (u32, u32)) -> Vec<(u32, u32)> {
        self.get_neighbors(pos)
            .into_iter()
            .filter(|pos| self.grid[*pos].image == CellImage::Hidden)
            .collect()
    }

    // Returns a vector containing the locations of all the adjacent cells
    fn get_neighbors(&self, (row, col): (u32, u32)) -> Vec<(u32, u32)> {
        let mut result = self.get_3x3((row, col));
        for index in 0..result.len() {
            if result[index] == (row, col) {
                result.swap_remove(index);
                break;
            }
        }
        result
    }

    // Returns a vector containing the locations of all adjacent cells and the given cell
    fn get_3x3(&self, (row, col): (u32, u32)) -> Vec<(u32, u32)> {
        let mut result = Vec::with_capacity(8);
        let row = row as i64;
        let col = col as i64;
        let height = self.height() as i64;
        let width = self.width() as i64;
        for row_difference in -1..=1 {
            let neighbor_row = row + row_difference;
            if neighbor_row >= 0 && neighbor_row < height {
                for col_difference in -1..=1 {
                    let neighbor_col = col + col_difference;
                    if neighbor_col >= 0 && neighbor_col < width {
                        result.push((neighbor_row as u32, neighbor_col as u32))
                    }
                }
            }
        }
        result
    }

    // Returns the number of mines surrounding the Cell at (row, col).
    // Technically includes the cell in the count but since this function should never be called
    // on a mine that should never cause issues.
    fn get_mines_around(&self, pos: (u32, u32)) -> u8 {
        let mut num_mines = 0;
        for pos in self.get_3x3(pos) {
            if self.grid[pos].mine {
                num_mines += 1;
            }
        }
        num_mines
    }
}