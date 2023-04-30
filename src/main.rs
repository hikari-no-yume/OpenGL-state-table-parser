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

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
enum Condition {
    /// This entry is only in the core profile. It might have a different
    /// definition in the compatibility profile.
    Core,
    /// This entry is only in the compatibility profile. It might have a
    /// different definition in the core profile.
    Compatibility,
    /// This entry is only in the Imaging Subset, which is only in the
    /// compatibility profile.
    ImagingSubset,
}

/// An entry in one of the state tables, representing a state variable
#[derive(Debug)]
struct Entry {
    /// If this is [Some], the entry is only defined when this condition
    /// applies.
    condition: Option<Condition>,
    /// "Get value" (symbolic constant to pass to "Get command")
    get_value: String,
    /// If this is `Some(n)`, there is a series of at least `n` values, and the
    /// symbolic constant named by `get_value` is just the first of them.
    ///
    /// Subsequent values are referred to by the numeric value of that constant
    /// plus the index of that value from zero, or alternatively by a constant
    /// whose name is formed by substituting the index for `0`. The index will
    /// be referred to in `description` as `$i$`.
    ///
    /// One example of such a series is `GL_TEXTURE0`.
    series: Option<String>,
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

/// Remove an expected multiplication in a type, i.e. turn
/// "A times B" into just "B".
fn divide(type_: &str, by: usize) -> String {
    type_.replace(&format!("{} \\times ", by), "")
}

/// The combination of the conditonal expansion and parameter expansion can
/// result in entries that have identical core and compatibility variants.
/// This function does a simple deduplication.
fn push_entry(entries: &mut Vec<Entry>, new_entry: Entry) {
    if new_entry.condition.is_some() {
        for existing_entry in entries.iter_mut().rev() {
            // These duplicates only occur within a single function. Don't waste
            // time if we're no longer in the same section etc.
            if existing_entry.condition.is_none()
                || existing_entry.type_ != new_entry.type_
                || existing_entry.get_cmnd != new_entry.get_cmnd
                || existing_entry.initial_value != new_entry.initial_value
                || existing_entry.description != new_entry.description
                || existing_entry.attribute != new_entry.attribute
            {
                break;
            }

            if existing_entry.get_value == new_entry.get_value {
                assert_ne!(existing_entry.condition, new_entry.condition);
                existing_entry.condition = None;
                return;
            }
        }
    }

    entries.push(new_entry);
}

fn process_row(
    spec: &str,
    condition: Option<Condition>,
    cells: [&str; 7],
    entries: &mut Vec<Entry>,
) {
    let [get_value, type_, get_cmnd, initial_value, description, section, attribute] = cells;

    // The description might contain a deprecation conditional. Expand both
    // branches for machine-friendliness. This is also a prerequisite for
    // expanding some parameterised get_value cases (see below).
    if let Some(dep_offset) = description.find("\\dep{") {
        let (before, after) = description.split_at(dep_offset);
        let (conditional, after) = read_cell(&after[after.find('{').unwrap()..]);
        let description_compatibility = format!("{}{}{}", before, conditional, after);
        let description_core = format!("{}{}", before, after);
        match condition {
            Some(Condition::Compatibility) => process_row(
                spec,
                condition,
                [
                    get_value,
                    type_,
                    get_cmnd,
                    initial_value,
                    &description_compatibility,
                    section,
                    attribute,
                ],
                entries,
            ),
            Some(Condition::Core) => process_row(
                spec,
                condition,
                [
                    get_value,
                    type_,
                    get_cmnd,
                    initial_value,
                    &description_core,
                    section,
                    attribute,
                ],
                entries,
            ),
            Some(Condition::ImagingSubset) => unimplemented!(),
            None => {
                process_row(
                    spec,
                    Some(Condition::Compatibility),
                    [
                        get_value,
                        type_,
                        get_cmnd,
                        initial_value,
                        &description_compatibility,
                        section,
                        attribute,
                    ],
                    entries,
                );
                process_row(
                    spec,
                    Some(Condition::Core),
                    [
                        get_value,
                        type_,
                        get_cmnd,
                        initial_value,
                        &description_core,
                        section,
                        attribute,
                    ],
                    entries,
                );
            }
        }
        return;
    }

    let get_value = unescape(get_value);

    // Some of these values are parameterised for compactness. We have to handle
    // this in one way or another, let's expand them for machine-friendliness.

    // These expansions for RGBA PixelMap values are listed in the section for
    // "The Imaging Subset" in a table labelled "PixelMap parameters".
    const PIXEL_MAP_RGBA_MODES: &[&str] = &[
        "PIXEL_MAP_I_TO_R",
        "PIXEL_MAP_I_TO_G",
        "PIXEL_MAP_I_TO_B",
        "PIXEL_MAP_I_TO_A",
        "PIXEL_MAP_R_TO_R",
        "PIXEL_MAP_G_TO_G",
        "PIXEL_MAP_B_TO_B",
        "PIXEL_MAP_A_TO_A",
    ];
    const PIXEL_MAP_INDEX_MODES: &[&str] = &["PIXEL_MAP_I_TO_I", "PIXEL_MAP_S_TO_S"];

    // TEXTURE_1D, TEXTURE_2D, TEXTURE_3D, and related enums.
    if get_value.contains("$x$D") {
        let dimensions: &[&str] = if spec == "gl" {
            &["1", "2", "3"]
        } else {
            &["2", "3"]
        };
        for dimension in dimensions {
            let get_value = get_value.replace("$x$", dimension);
            // Remove vectorness
            let type_ = divide(type_, dimensions.len());
            // Remove list of dimensions ("x is 1, 2, or 3.") from description,
            // then expand.
            let description = description
                .split_once("; $x$ is")
                .map_or(description, |(before, _after)| before)
                .replace("$x$", dimension);
            process_row(
                spec,
                condition,
                [
                    &get_value,
                    &type_,
                    get_cmnd,
                    initial_value,
                    &description,
                    section,
                    attribute,
                ],
                entries,
            );
        }
        return;
    // These expansions for the BIAS and SCALE values are listed in the section
    // for "The Imaging Subset" in a table labelled "PixelTransfer parameters".
    } else if section == "\\ref{pix:xfer}"
        && (get_value.contains("$x$_BIAS") || get_value.contains("$x$_SCALE"))
        && !description.contains(',')
    {
        for component in ["RED", "GREEN", "BLUE", "ALPHA"] {
            let get_value = get_value.replace("$x$", component);
            let description = description.replace("$x$", component);
            process_row(
                spec,
                condition,
                [
                    &get_value,
                    type_,
                    get_cmnd,
                    initial_value,
                    &description,
                    section,
                    attribute,
                ],
                entries,
            );
        }
        return;
    } else if section == "\\ref{pix:xfer}" && get_value == "$x$" && get_cmnd.contains("GetPixelMap")
    {
        let modes = if description.contains("RGBA") {
            PIXEL_MAP_RGBA_MODES
        } else {
            assert!(description.contains("Index"));
            PIXEL_MAP_INDEX_MODES
        };
        for mode in modes {
            let get_value = get_value.replace("$x$", mode);
            // Remove vectorness
            let type_ = divide(type_, modes.len());
            // Remove plural and explanation of $x$.
            let description = description.split_once("s; $x$ is").unwrap().0;
            process_row(
                spec,
                condition,
                [
                    &get_value,
                    &type_,
                    get_cmnd,
                    initial_value,
                    description,
                    section,
                    attribute,
                ],
                entries,
            );
        }
        return;
    } else if section == "\\ref{pix:xfer}"
        && get_value == "$x$_SIZE"
        && get_cmnd.contains("GetIntegerv")
    {
        for mode in PIXEL_MAP_RGBA_MODES
            .iter()
            .chain(PIXEL_MAP_INDEX_MODES.iter())
        {
            let get_value = get_value.replace("$x$", mode);
            let description = description.replace("$x$", mode);
            process_row(
                spec,
                condition,
                [
                    &get_value,
                    type_,
                    get_cmnd,
                    initial_value,
                    &description,
                    section,
                    attribute,
                ],
                entries,
            );
        }
        return;
    // These expansions for Map1/Map2 values are listed in the "Evaluators"
    // section table labelled "Values specified by the target to Map1".
    } else if (get_value == "MAP1_$x$" || get_value == "MAP2_$x$") && get_cmnd == "\\glr{IsEnabled}"
    {
        let values = [
            "VERTEX_3",
            "VERTEX_4",
            "INDEX",
            "COLOR_4",
            "NORMAL",
            "TEXTURE_COORD_1",
            "TEXTURE_COORD_2",
            "TEXTURE_COORD_3",
            "TEXTURE_COORD_4",
        ];
        for value in values {
            let get_value = get_value.replace("$x$", value);
            // Remove plural and explanation of $x$.
            let description = description.split_once("s: $x$ is").unwrap().0;
            // Remove vectorness.
            let type_ = divide(type_, values.len());
            process_row(
                spec,
                condition,
                [
                    &get_value,
                    &type_,
                    get_cmnd,
                    initial_value,
                    description,
                    section,
                    attribute,
                ],
                entries,
            );
        }
        return;
    } else if get_value.contains("$x$") {
        // Some values conveniently list their expansions in their descriptions.
        if let Some((description, expansions)) = description
            .split_once("; $x$ is one of ")
            .or_else(|| description.split_once(";\n$x$ is one of "))
            .or_else(|| description.split_once(".    $x$ is one of "))
            .or_else(|| description.split_once(". $x$ is one of "))
            .or_else(|| description.split_once("; $x$ is "))
            .or_else(|| description.split_once(" ($x$ is "))
        {
            if expansions.contains(',') {
                let expansions = expansions.strip_suffix(')').unwrap_or(expansions);
                let expansions: Vec<_> = expansions.split(',').collect();
                for expansion in expansions.iter() {
                    let expansion = expansion.trim();
                    let expansion = expansion.strip_prefix("or ").unwrap_or(expansion);
                    let expansion = expansion.strip_prefix("\\glc{").unwrap_or(expansion);
                    let expansion = expansion.strip_suffix('}').unwrap_or(expansion);

                    let get_value = get_value.replace("$x$", expansion);
                    // Remove vectorness
                    let type_ = divide(type_, expansions.len());
                    let description = description.replace("$x$", expansion);
                    process_row(
                        spec,
                        condition,
                        [
                            &get_value,
                            &type_,
                            get_cmnd,
                            initial_value,
                            &description,
                            section,
                            attribute,
                        ],
                        entries,
                    );
                }
                return;
            }
        }
    }

    // In ES 1.1's spec, the whole type is implicitly inline math
    let type_ = if spec == "es11" {
        format!("${}$", type_)
    } else {
        type_.to_string()
    };

    // Match series like GL_TEXTUREn, GL_CLIP_PLANEn etc.
    let (get_value, series, type_) = if let Some(prefix) = get_value.strip_suffix("$i$") {
        let first_get_value = format!("{}0", prefix);
        // Extract minimum count from type
        let (count, type_) = type_.split_once(" \\times ").unwrap();
        let count = count.strip_prefix('$').unwrap();
        // Handle annoying exception
        let count = count
            .strip_prefix('{')
            .and_then(|count| count.strip_suffix('}'))
            .unwrap_or(count);
        // "*" means "at least"
        let count = count.strip_suffix('*').unwrap();
        // Ensure LaTeX inline math characters are balanced in type
        let type_ = format!("${}", type_);
        (first_get_value, Some(count.to_string()), type_)
    } else {
        (get_value, None, type_)
    };

    let get_cmnd = unescape(if spec == "es11" || get_cmnd == "--" || get_cmnd == "-" {
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

    // Note that the section is ignored because we don't have access to the
    // LaTeX source of the full spec, so we can't resolve to a section number.

    push_entry(
        entries,
        Entry {
            condition,
            get_value,
            series,
            type_,
            get_cmnd,
            initial_value,
            description,
            attribute,
        },
    );
}

fn parse_spec(spec: &str) -> Vec<Entry> {
    // Read text from file while removing comments
    let mut text = String::new();
    let file =
        std::fs::File::open(format!("tables_src/gettables.{}.tex", spec)).expect("Can't open file");
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

        // Normal entry or entry marking a change from a previous version
        let condition = if text.starts_with("\\doentry")
            || text.starts_with("\\cbentry")
            || text.starts_with("\\ocbentry")
        {
            current_condition
        // Imaging subset (deprecated) entry
        } else if text.starts_with("\\graydepentry") {
            Some(Condition::ImagingSubset)
        // Deprecated entry
        } else if text.starts_with("\\depentry") {
            Some(Condition::Compatibility)
        // Conditionals
        } else {
            if text.starts_with("\\ifnum\\specdep=1") {
                assert!(current_condition.is_none());
                current_condition = Some(Condition::Compatibility);
            } else if text.starts_with("\\else") {
                assert!(current_condition == Some(Condition::Compatibility));
                current_condition = Some(Condition::Core);
            } else if text.starts_with("\\fi") {
                assert!(current_condition.is_some());
                current_condition = None;
            }
            text = &text[1..];
            continue;
        };

        text = &text[text.find('{').unwrap()..];

        let mut cells = Vec::new();
        let column_count = if spec == "es11" { 8 } else { 7 };
        for _ in 0..column_count {
            let (cell, new_text) = read_cell(text);
            cells.push(cell);
            text = new_text;
        }

        let cells = if spec == "es11" {
            [
                cells[4], cells[1], cells[3], cells[2], cells[5], cells[6], cells[7],
            ]
        } else {
            [
                cells[0], cells[1], cells[2], cells[3], cells[4], cells[5], cells[6],
            ]
        };

        process_row(spec, condition, cells, &mut entries);
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
            Condition::Compatibility => "pink",
            Condition::Core => "lightgreen",
            Condition::ImagingSubset => "silver",
        });
        if let Some(color) = color {
            println!("<tr style=\"background-color:{}\">", color);
        } else {
            println!("<tr>");
        }
        if let Some(ref minimum) = entry.series {
            println!(
                "<td>{} …<br>{} + (<i>n</i>-1)<br>where <i>n</i> ≥ {}</td>",
                entry.get_value, entry.get_value, minimum,
            );
            println!("<td><i>n</i> × {}</td>", entry.type_);
        } else {
            println!("<td>{}</td>", entry.get_value);
            println!("<td>{}</td>", entry.type_);
        }
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
        let entries = parse_spec(spec);
        println!("<h1><tt>{}</tt> state table entries</h1>", spec);
        print_table(&entries);
    }
}
