fn main() {
    let mut b = michi_rust::board::Board::new(19);
    let r1 = b.play(3, 3, michi_rust::board::Color::Black);
    let r2 = b.play(16, 16, michi_rust::board::Color::White);
    println!("Move results: B {:?}, W {:?}", r1, r2);
    println!("{}", b);
}
