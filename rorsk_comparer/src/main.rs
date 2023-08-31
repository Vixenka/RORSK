use std::{fs, collections::HashMap, path::PathBuf, thread, slice::Iter, sync::Mutex};

lazy_static::lazy_static! {
    static ref PRINT_MUTEX: Mutex<String> = Mutex::new(String::new());

    static ref VENDOR_IDS: HashMap<u32, &'static str> = {
        let mut m = HashMap::new();
        m.insert(4098, "AMD");
        m.insert(4318, "NVIDIA");
        m.insert(32902, "Intel");
        m
    };

    static ref DEVICE_IDS: HashMap<u32, &'static str> = {
        let mut m = HashMap::new();
        m.insert(29695, "Radeon RX 6600 XT");
        m.insert(9988, "RTX 4080");
        m.insert(22176, "Arc A770 Graphics");
        m
    };
}

struct CompareTask {
    device_name: String,
    path: PathBuf
}

fn main() {
    let mut threads = Vec::new();

    for (problem_name, task) in search_tasks() {
        let a = problem_name.to_owned();
        threads.push(thread::spawn(move || {
            compare_task(a, task.0, true);
        }));
        threads.push(thread::spawn(move || {
            compare_task(problem_name, task.1, false);
        }));
    }

    for thread in threads {
        thread.join().unwrap();
    }

    let lock = PRINT_MUTEX.lock().unwrap();
    fs::write("../output/results.txt", lock.as_bytes()).unwrap();
    println!("Done! Saved result logs to `output/results.txt` file.");
}

fn search_tasks() -> HashMap<String, (Vec<CompareTask>, Vec<CompareTask>)> {
    let mut searched = HashMap::new();

    for file_wrapped in fs::read_dir("../output/").unwrap() {
        let file = file_wrapped.unwrap();
        let os_string = file.file_name();
        if os_string.to_str().unwrap() == "results.txt" {
            continue;
        }

        let mut split = os_string.to_str().unwrap().split('_');

        let problem_name = split.next().unwrap();
        let vec;
        if let Some(v) = searched.get_mut(problem_name) {
            vec = v;
        } else {
            searched.insert(problem_name.to_owned(), (Vec::new(), Vec::new()));
            vec = searched.get_mut(problem_name).unwrap();
        }

        let vendor_id: u32 = split.next().unwrap().parse().unwrap();
        split = split.next().unwrap().split('.');
        let device_id: u32 = split.next().unwrap().parse().unwrap();

        let is_conformant = match split.next().unwrap() {
            "bin" => false,
            "binc" => true,
            &_ => continue
        };

        let device_name =
            format!("{} {}", VENDOR_IDS.get(&vendor_id).unwrap(), DEVICE_IDS.get(&device_id).unwrap());

        let v = if is_conformant { &mut vec.0 } else { &mut vec.1 };
        v.push(CompareTask {
            device_name,
            path: file.path(),
        });
    }

    searched
}

fn compare_task(problem_name: String, data: Vec<CompareTask>, is_conformant: bool) {
    let type_name = problem_name.split('-').next().unwrap();
    let target = fs::read(&data[0].path).unwrap();

    let mut count: u64 = 0;

    for task in data.iter().skip(1) {
        let read = fs::read(&task.path).unwrap();
        let mut target_temp = target.iter();
        let mut read_temp = read.iter();
        while let Some(difference) = compare_difference(&mut target_temp, &mut read_temp, type_name, is_conformant) {
            if difference {
                count += 1;
            }
        }
    }

    let conformant_str = if is_conformant { "conformant" } else { "unconformant" };

    let mut message = String::new();
    message.push_str(&format!("\nProblem `{}` on {} was tested with devices:", problem_name, conformant_str));
    for task in data.iter() {
        message.push_str(&format!("\n  - {}", task.device_name));
    }

    message.push_str("\nResults:");
    message.push_str(&format!("\n  - Data count: {} bits", target.len() * 8));
    message.push_str(&format!("\n  - Number of differences: {}", count));

    let mut lock = PRINT_MUTEX.lock().unwrap();
    lock.push_str(&message);
    println!("{}", message);
}

fn compare_difference(expected: &mut Iter<'_, u8>, data: &mut Iter<'_, u8>, type_name: &str, is_conformant: bool) -> Option<bool> {
    match type_name {
        "i32" => {
            if let Some(a) = read_i32(expected) {
                if let Some(b) = read_i32(data) {
                    if is_conformant && a != b {
                        println!("i32 {} {}", a, b);
                    }

                    return Some(a != b);
                }
            }
        }
        "f32" => {
            if let Some(a) = read_f32(expected) {
                if let Some(b) = read_f32(data) {
                    let compare;
                    if a.is_finite() {
                        compare = a.to_bits() != b.to_bits();
                    } else if a.is_nan() {
                        compare = !b.is_nan();
                    } else {
                        assert!(a.is_infinite());
                        compare = !b.is_infinite();
                    }

                    if compare && is_conformant {
                        println!("f32 {:b} {:b} {}", a.to_bits(), b.to_bits(), (a - b).abs());
                    }
                    return Some(compare);
                }
            }
        }
        &_ => panic!("Unknown type name")
    }

    None
}

fn read_i32(data: &mut Iter<'_, u8>) -> Option<i32> {
    read_four_bytes(data).map(i32::from_le_bytes)
}

fn read_f32(data: &mut Iter<'_, u8>) -> Option<f32> {
    read_four_bytes(data).map(f32::from_le_bytes)
}

fn read_four_bytes(data: &mut Iter<'_, u8>) -> Option<[u8; 4]> {
    if let Some(a) = data.next() {
        if let Some(b) = data.next() {
            if let Some(c) = data.next() {
                if let Some(d) = data.next() {
                    return Some([*a, *b, *c, *d]);
                }
            }
        }
    }

    None
}
