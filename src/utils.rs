pub fn last_position_of(input: &str, needle: u8) -> usize {
    input
        .as_bytes()
        .iter()
        .enumerate()
        .filter(|(_i, c)| **c == needle)
        .next_back()
        .unwrap()
        .0
}

pub fn get_only<T>(input: impl IntoIterator<Item = T>, error: &str) -> T {
    let mut iter = input.into_iter();
    let only = iter.next().expect(error);
    if let Some(_) = iter.next() {
        panic!("{error} - too big")
    } else {
        only
    }
}
