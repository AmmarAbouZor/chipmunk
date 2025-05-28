use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub enum Case {
    Sensitive,
    Insensitive,
}

#[derive(Debug, Clone, Copy)]
pub enum SearchType {
    Like,
    Regex,
}

/// Represents the functions needed to search tests and benchmarks.
pub trait Search {
    fn search(&mut self, pattern: String, typ: SearchType, case: Case) -> Vec<usize>;
}

pub fn run_benches(mut db: impl Search, pattern: String, typ: SearchType, case: Case) {
    let time = Instant::now();
    let res = db.search(pattern, typ, case);
    let elapsed = time.elapsed();
    println!("-------------------------------------------------");
    println!("Search took {} microseconds", elapsed.as_micros());
    println!("Search took {} milliseconds", elapsed.as_millis());
    println!("-------------------------------------------------");
    println!("Results len: {}", res.len());
    // println!("Results: {res:?}");
}
