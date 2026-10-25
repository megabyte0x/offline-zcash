fn main() {
    let h = hex::decode(std::env::args().nth(1).unwrap()).unwrap();
    assert_eq!(h.len(), 177);
    let r = equihash::is_valid_solution(48, 5, &h[..108], &h[108..140], &h[141..]);
    println!("Regtest header: solution len {} (compactsize byte {}), Equihash(48,5) valid={:?}", h.len() - 141, h[140], r.is_ok());
}
