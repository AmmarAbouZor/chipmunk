use std::time::Instant;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forwards,
    Backwards,
}

/// Represents the behaviors needed to paging tests and benchmarks.
pub trait Paging {
    fn records_count(&mut self) -> usize;
    fn load_records(&mut self, start: usize, end: usize) -> Vec<String>;
}

pub fn run_benches(mut db: impl Paging, dir: Direction) {
    let records_count = db.records_count();

    let mut times = Vec::new();

    const UPDATE_STEP: usize = 100;

    match dir {
        Direction::Forwards => {
            let mut curr_idx = 0;
            while curr_idx < records_count {
                let timer = Instant::now();
                let rows = db.load_records(curr_idx, curr_idx + UPDATE_STEP);
                let elapsed = timer.elapsed().as_micros();
                // println!("{elapsed}");
                times.push(elapsed);
                process_lines(rows);
                curr_idx += UPDATE_STEP;
            }
        }
        Direction::Backwards => {
            let mut curr_idx = records_count;
            while curr_idx > UPDATE_STEP {
                let timer = Instant::now();
                let rows = db.load_records(curr_idx, curr_idx + UPDATE_STEP);
                let elapsed = timer.elapsed().as_micros();
                // println!("{elapsed}");
                times.push(elapsed);
                process_lines(rows);
                curr_idx -= UPDATE_STEP;
            }
        }
    }

    let avg = times.iter().sum::<u128>() / times.len() as u128;
    println!("Paging average is {avg} microseconds");
}

fn process_lines(lines: Vec<String>) {
    // println!("len: {}", lines.len());
    std::hint::black_box(lines);

    // for line in lines {
    //     println!("{line}");
    // }
}
