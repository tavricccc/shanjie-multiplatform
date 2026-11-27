// Keycap order and bit assignments from Nanako0129/shanjie PR #132.
pub const ALL_MASK: u32 = (1 << 27) - 1;

const SHIFT: [(char, char); 21] = [
    ('\x60', '~'), ('1', '!'), ('2', '@'), ('3', '#'), ('4', '$'), ('5', '%'),
    ('6', '^'), ('7', '&'), ('8', '*'), ('9', '('), ('0', ')'), ('-', '_'),
    ('=', '+'), ('[', '{'), (']', '}'), ('\\', '|'), (';', ':'), ('\'', '"'),
    (',', '<'), ('.', '>'), ('/', '?'),
];
const PLAIN: [char; 6] = ['\x60', '=', '[', ']', '\\', '\''];

pub fn selected(mask: u32, key: char, shifted: bool) -> Option<char> {
    if mask == 0 {
        return None;
    }
    let (bit, symbol) = if shifted {
        let bit = SHIFT.iter().position(|&(keycap, _)| keycap == key)?;
        (bit, SHIFT[bit].1)
    } else {
        let bit = PLAIN.iter().position(|&keycap| keycap == key)? + SHIFT.len();
        (bit, key)
    };
    (mask & (1 << bit) != 0).then_some(symbol)
}
