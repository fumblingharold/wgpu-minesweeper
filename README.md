# How to get started
Just compile the project and run the executable. I'll get some precompiled binaries for the project once I'm more happy
with the state of it, but that is not now.

# How to play
Classic minesweeper: left click to reveal a cell, right click to flag, win when all non-mine cells are revealed.
Left-clicking the face at the top will reset the board. The left seven-segment display is a timer that stops when the game 
ends while the right display tells you how many mines are left unflagged (assuming all of your placed flags are 
correct).

There's also some of the creature comforts sometimes not found such as left-clicking on a revealed cell will reveal all 
unflagged cells around it and left-clicking a flagged cell turns it into a question marked cell. Let the debate over the
utility of question marked cells ensue.

# Why did you make this?
I wanted to get some more practice with Rust and thought I'd also learn about graphics while I'm at it. Minesweeper felt
like a rather obvious choice for its simplicity and since minesweeperonline.com and other alternatives are not good. I 
mean, using anything but nearest neighbor for scaling up pixel art is sacrilege! And the Windows XP assets are obviously
the best since that's what I grew up with! All snark aside, I made something I'd actually want to use. I did slowly 
experience scope creep, though, and now there's a few things left that will take a good while to sort out so I leave 
them to future me to complete.

# What's left
- Right-click on face should open window to allow resizing the board and other settings
- Best completion times should be stored in file
- Rendering should be optimized since it is currently drawing the whole scene every time (yes, I know it doesn't matter)

### Additional notes
Yes, I know that some things are a pixel off or so. They felt like bugs in the original implementation and I wanted to
resolve them, even if that means slightly changing the look. Then again, if you're the sort of person to care that much,
just run the OG version in a VM.