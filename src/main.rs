#![allow(non_snake_case)] // let me capitalize the crate name, Rust!

/// Match a set of curly braces potentially containing nested curly braces.
/// Returns the content of the outermost set of braces, and the remaining text.
fn read_cell(text: &str) -> (&str, &str) {
    let mut offset = 0;
    let mut depth: u32 = 0;
    loop {
        offset += text[offset..].find(['{', '}']).unwrap();
        if text[offset..].starts_with('{') {
            offset += 1;
            depth += 1;
        } else {
            assert!(text[offset..].starts_with('}'));
            offset += 1;
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
    }
    let (cell, remainder) = text.split_at(offset);
    let cell = cell
        .trim()
        .strip_prefix('{')
        .unwrap()
        .strip_suffix('}')
        .unwrap();
    (cell, remainder)
}

#[derive(PartialEq, Eq, Copy, Clone)]
enum Condition {
    /// This entry is only in the compatibility profile. It might have a
    /// different definition in the core profile.
    CoreOnly,
    /// This entry is only in the core profile. It might have a different
    /// definition in the compatibility profile.
    CompatibilityOnly,
}

struct Entry {
    condition: Option<Condition>,
    cells: Vec<String>,
}

fn main() {
    let mut args = std::env::args();
    // Skip argv[0] (binary name)
    args.next();

    let filename = args
        .next()
        .expect("Please specify a filename and number of columns");
    let column_count = args
        .next()
        .expect("Please specify a filename and number of columns");
    let column_count: u32 = column_count.parse().expect("Invalid column count");
    if args.next().is_some() {
        panic!("Too many arguments");
    }

    // Read text from file while removing comments
    let mut text = String::new();
    let file = std::fs::File::open(filename).expect("Can't open file");
    let mut hit_divider = false;
    for line in std::io::BufRead::lines(std::io::BufReader::new(file)) {
        let line = line.unwrap();

        // Ignore lines before the divider that marks the start of the entries
        // proper (rather than the macro definitions etc which aren't handled)
        if !hit_divider {
            if line == "%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%" {
                hit_divider = true;
            } else {
                continue;
            }
        }

        let line = line.trim_start();
        let line = if let Some((not_comment, _comment)) = line.split_once('%') {
            not_comment
        } else {
            line
        };
        let line = line.trim_end();
        if !line.is_empty() {
            text.push_str(line);
            text.push('\n');
        }
    }

    // Parse entries
    let mut entries = Vec::new();
    let mut current_condition: Option<Condition> = None;
    let mut text: &str = &text;
    while let Some(offset) = text.find('\\') {
        text = &text[offset..];

        // Entries
        let condition = if text.starts_with("\\doentry") {
            current_condition
        } else if text.starts_with("\\depentry") {
            Some(Condition::CompatibilityOnly)
        // Conditionals
        } else {
            if text.starts_with("\\ifnum\\specdep=1") {
                assert!(current_condition.is_none());
                current_condition = Some(Condition::CompatibilityOnly);
            } else if text.starts_with("\\else") {
                assert!(current_condition == Some(Condition::CompatibilityOnly));
                current_condition = Some(Condition::CoreOnly);
            } else if text.starts_with("\\fi") {
                assert!(current_condition.is_some());
                current_condition = None;
            }
            text = &text[1..];
            continue;
        };

        text = &text[text.find('{').unwrap()..];

        let mut cells = Vec::new();
        for _ in 0..column_count {
            let (cell, new_text) = read_cell(text);
            cells.push(cell.to_string());
            text = new_text;
        }
        entries.push(Entry { condition, cells });
    }

    println!("<table>");
    for Entry { condition, cells } in entries {
        let color = condition.map(|condition| match condition {
            Condition::CompatibilityOnly => "pink",
            Condition::CoreOnly => "lightgreen",
        });
        if let Some(color) = color {
            print!("<tr style=\"background-color:{}\">", color);
        } else {
            print!("<tr>");
        }
        println!("<td>{}</td></tr>", cells.join("</td><td>"));
    }
}
