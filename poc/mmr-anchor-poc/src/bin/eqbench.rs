fn main() {
    let v: Vec<Vec<u8>> = include_str!("../../vec.txt").split_whitespace().map(|x| hex::decode(x).unwrap()).collect();
    let n = 2000;
    let t = std::time::Instant::now();
    for _ in 0..n { equihash::is_valid_solution(200, 9, &v[0], &v[1], &v[2]).unwrap(); }
    let per = t.elapsed() / n;
    let mut bad = v[2].clone(); bad[10] ^= 1;
    println!("Equihash(200,9) verify: {:?} per header; corrupted solution rejected={}", per, equihash::is_valid_solution(200, 9, &v[0], &v[1], &bad).is_err());
}
