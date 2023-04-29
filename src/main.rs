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

/// An entry in one of the state tables, representing a state variable
struct Entry {
    /// If this is [Some], the entry is only defined when this condition
    /// applies.
    condition: Option<Condition>,
    /// "Get value" (symbolic constant to pass to "Get command")
    get_value: String,
    /// "Type"
    type_: String,
    /// "Get command" (function that can query this state variable)
    get_cmnd: String,
    /// "Initial value"
    initial_value: String,
    /// "Description"
    description: String,
    /// "Attribute"
    attribute: String,
}

fn unescape(cell: &str) -> String {
    cell
        // Unescape underscores
        .replace("\\_", "_")
        // Remove line-wrap hyphenation
        .replace("\\-", "")
}

fn process_row(is_es11: bool, condition: Option<Condition>, cells: [&str; 6]) -> Entry {
    let [get_value, type_, get_cmnd, initial_value, description, attribute] = cells;

    let get_value = unescape(get_value);

    // In ES 1.1's spec, the whole type is implicitly inline math
    let type_ = if is_es11 {
        format!("${}$", type_)
    } else {
        type_.to_string()
    };

    let get_cmnd = unescape(if is_es11 || get_cmnd == "--" || get_cmnd == "-" {
        get_cmnd
    } else {
        get_cmnd
            .strip_prefix("\\glr{")
            .unwrap()
            .strip_suffix('}')
            .unwrap()
    });

    let initial_value = unescape(initial_value);
    let description = unescape(description);
    let attribute = attribute.to_string();

    Entry {
        condition,
        get_value,
        type_,
        get_cmnd,
        initial_value,
        description,
        attribute,
    }
}

fn parse_spec(filename: &std::path::Path, is_es11: bool) -> Vec<Entry> {
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
        let column_count = if is_es11 { 8 } else { 7 };
        for _ in 0..column_count {
            let (cell, new_text) = read_cell(text);
            cells.push(cell);
            text = new_text;
        }

        // The section (cells[6]/cells[5]) is ignored because we don't have
        // access to the LaTeX source of the full spec, so we can't resolve
        // the ID to a section number
        let cells = if is_es11 {
            [cells[4], cells[1], cells[3], cells[2], cells[5], cells[7]]
        } else {
            [cells[0], cells[1], cells[2], cells[3], cells[4], cells[6]]
        };
        entries.push(process_row(is_es11, condition, cells));
    }

    entries
}

fn print_table(entries: &[Entry]) {
    println!("<table>");
    println!("<thead>");
    println!("<th>Get value</th>");
    println!("<th>Type</th>");
    println!("<th>Get command</th>");
    println!("<th>Initial value</th>");
    println!("<th>Description</th>");
    println!("<th>Attribute</th>");
    println!("</thead>");
    println!("<tbody>");
    for entry in entries {
        let color = entry.condition.map(|condition| match condition {
            Condition::CompatibilityOnly => "pink",
            Condition::CoreOnly => "lightgreen",
        });
        if let Some(color) = color {
            println!("<tr style=\"background-color:{}\">", color);
        } else {
            println!("<tr>");
        }
        println!("<td>{}</td>", entry.get_value);
        println!("<td>{}</td>", entry.type_);
        println!("<td>{}</td>", entry.get_cmnd);
        println!("<td>{}</td>", entry.initial_value);
        println!("<td>{}</td>", entry.description);
        println!("<td>{}</td>", entry.attribute);
        println!("</tr>");
    }
    println!("</tbody>");
    println!("</table>");
}

fn main() {
    for spec in ["es11", "es", "gl"] {
        let filename = format!("tables_src/gettables.{}.tex", spec);
        let entries = parse_spec(std::path::Path::new(&filename), spec == "es11");
        println!("<h1>State table entries from <tt>{}</tt></h1>", filename);
        print_table(&entries);
    }
}
