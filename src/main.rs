#![allow(non_snake_case)] // let me capitalize the crate name, Rust!

fn main() {
    let mut args = std::env::args();
    // Skip argv[0] (binary name)
    args.next();

    let filename = args.next().expect("Please specify a filename and number of columns");
    let column_count = args.next().expect("Please specify a filename and number of columns");
    let column_count: u32 = column_count.parse().expect("Invalid column count");
    if args.next().is_some() {
        panic!("Too many arguments");
    }

    // Read text from file while removing comments
    let mut text = String::new();
    let file = std::fs::File::open(filename).expect("Can't open file");
    for line in std::io::BufRead::lines(std::io::BufReader::new(file)) {
        let line = line.unwrap();
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

    // Skip to the first actual entry (not the definition of \doentry)
    let mut text = &text[text.find("\\doentry{").unwrap()..];
    // Parse entries
    let mut entries = Vec::new();
    while let Some(entry_offset) = text.find("\\doentry") {
        text = &text[entry_offset..];
        text = &text[text.find('{').unwrap()..];

        // Match cells with nested parentheses
        let mut cells = Vec::new();
        let mut offset = 0;
        for _ in 0..column_count {
            let start_offset = offset;
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
            let cell = text[start_offset..offset].trim();
            let cell = cell.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
            cells.push(cell.to_string());
        }
        entries.push(cells);
        text = &text[offset..];
    }

    println!("<table>");
    for cells in entries {
        println!("<tr><td>{}</td></tr>", cells.join("</td><td>"));
    }
}
