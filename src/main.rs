use std::io::{BufRead, BufReader};
use regex;
use regex::Regex;

struct Rule {
    pattern: String,
    label_name: String,
    label_value: String,
}

fn main() {
    let cfg = vec![Rule {
        pattern: r"^prefix:([\w]+):".to_string(),
        label_name: "user".to_string(),
        label_value: "$1".to_string(),
    }];

    let f = std::fs::File::open("keys.txt").unwrap();
    let b = BufReader::new(f);

    let mut name = String::new();
    let mut val = String::new();

    for line in b.lines() {
        let l = line.unwrap();

        for r in cfg.iter() {
            let pattern = Regex::new(&r.pattern).unwrap();

            if let Some(c) = pattern.captures(&l) {


                c.expand(&r.label_name, &mut name);
                c.expand(&r.label_value, &mut val);

                println!("{} = {}", name, val);
                name.clear();
                val.clear()
            }
        }
    }

}
