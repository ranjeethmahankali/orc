#[derive(Debug)]
pub enum Error {
    ArrayLengthMismatch(usize, usize),
    InvalidHandle,
    DeckTypeMismatch,
    DeckBorrowError,
}
