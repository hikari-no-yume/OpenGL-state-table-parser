#![allow(non_snake_case)] // let me capitalize the crate name, Rust!

mod types;
use types::{parse_quantity, parse_type, print_quantity, print_type, MaybeParsed, Quantity, Type};

/// Match a set of curly braces potentially containing nested curly braces.
/// Returns the content of the outermost set of braces, and the remaining text.
fn read_cell(text: &str) -> (&str, &str) {
    let mut offset = 0;
    let mut depth: u32 = 0;
    loop {
        offset += text[offset..].find(['{', '}']).unwrap();
        if text[..offset].ends_with('\\') {
            offset += 1;
            continue;
        }
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

/// A state table
#[derive(Debug)]
struct Table {
    /// This is a "string used to describe the table in the index"
    title: String,
    /// Extra text that follows `title`
    caption: Option<String>,
    /// An internal label within the LaTeX source
    label: String,
    /// Footnotes that are referenced by entries
    footnotes: Vec<String>,
    /// The entries in (rows of) the state table
    entries: Vec<Entry>,
}

/// An entry in one of the state tables, representing a state variable
#[derive(Debug)]
struct Entry {
    /// If this is [Some], the entry is only defined when this condition
    /// applies.
    condition: Option<Condition>,
    /// "Get value" (symbolic constant to pass to "Get command")
    get_value: Option<String>,
    /// Alternative "Get value", if any. This is not necessarily a synonym, e.g.
    /// `TRANSPOSE_` versions of matrices.
    ///
    /// These alternative values only seem to appear in the GL compatibility
    /// profile, so you can ignore them for core profile OpenGL and both
    /// versions of OpenGL ES. They're also mutually exclusive with `series`.
    alt_get_value: Option<String>,
    /// If this is `Some(n)`, there is a series of at least `n` values, and the
    /// symbolic constant named by `get_value` is just the first of them.
    ///
    /// Subsequent values are referred to by the numeric value of that constant
    /// plus the index of that value from zero, or alternatively by a constant
    /// whose name is formed by substituting the index for `0`. The index will
    /// be referred to in `description` as `$i$`.
    ///
    /// One example of such a series is `GL_TEXTURE0`.
    series: Option<Quantity>,
    /// "Type"
    ///
    /// There's only one case that doesn't have a type: `GetUniform`.
    type_: Option<MaybeParsed<Type>>,
    /// Index of a table footnote (if any) referenced by the type
    type_footnote: Option<usize>,
    /// "Get command" (function that can query this state variable)
    ///
    /// If this is [None], the variable is inaccessible.
    get_cmnd: Option<String>,
    /// "Initial value"
    initial_value: Option<String>,
    /// Index of a table footnote (if any) referenced by the initial value
    initial_value_footnote: Option<usize>,
    /// "Description"
    description: String,
    /// Index of a table footnote (if any) referenced by the description
    description_footnote: Option<usize>,
    /// "Attribute" (which attribute group to use with `PushAttrib`/`PopAttrib`
    /// or `PushClientAttrib`/`PopClientAttrib` as applicable)
    ///
    /// Attribute groups are a legacy feature that only exists in the OpenGL
    /// compatibility profile. Not even OpenGL ES 1.1 has them, though the state
    /// tables nonetheless include attribute group information for some reason?
    attribute: Option<String>,
}

fn unescape(cell: &str) -> String {
    // Remove group around change marker
    if let Some(offset) = cell.find("{\\ochange").or_else(|| cell.find("{\\change")) {
        let (before, after) = cell.split_at(offset);
        let (content, after) = read_cell(after);
        return unescape(&format!("{}{}{}", before, content, after));
    }
    // Remove change marker with accompanying issue number annotation
    if let Some(offset) = cell.find("\\change\\cbext{") {
        let (before, after) = cell.split_at(offset);
        let (_content, after) = read_cell(&after[after.find('{').unwrap()..]);
        return unescape(&format!("{}{}", before, after));
    }
    cell
        // Remove change markers (not a kind of escaping but annoying)
        .replace("\\change", "")
        .replace("\\ochange", "")
        // Collapse whitespace, HTML-style
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        // Unescape underscores
        .replace("\\_", "_")
        // Remove line-wrap hyphenation
        .replace("\\-", "")
        // LaTeX quotes to curly quotes
        .replace("``", "“")
        .replace("''", "”")
        // Remove small-font markup (the spec isn't consistent about using this
        // and it's not semantically useful)
        .replace("\\small ", "")
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

fn extract_footnote_ref(cell: String, table: &Table) -> (Option<String>, Option<usize>) {
    // Reference to first footnote, either from table header or from the
    // description
    let (cell, footnote_ref) = if let Some(cell) = cell
        .strip_suffix("\\fn1")
        .or_else(|| cell.strip_suffix("\\footnotemark[1]"))
    {
        (cell, 0)
    // Reference to second footnote in the table header
    } else if let Some(cell) = cell.strip_suffix("\\fnb") {
        (cell, 1)
    } else {
        return (Some(cell), None);
    };

    assert!(table.footnotes.get(footnote_ref).is_some());
    let cell = cell.trim();
    if cell.is_empty() {
        (None, Some(footnote_ref))
    } else {
        (Some(cell.to_string()), Some(footnote_ref))
    }
}

fn process_row(
    spec: &str,
    condition: Option<Condition>,
    cells: [&str; 7],
    constants: &[(String, String)],
    table: &mut Table,
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
                constants,
                table,
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
                constants,
                table,
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
                    constants,
                    table,
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
                    constants,
                    table,
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
                constants,
                table,
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
                constants,
                table,
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
                constants,
                table,
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
                constants,
                table,
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
                constants,
                table,
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
                    let type_ =
                        if let Some(stripped) = type_.strip_prefix("$\\mtexbasefmt \\times ") {
                            format!("${}", stripped)
                        } else {
                            divide(type_, expansions.len())
                        };
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
                        constants,
                        table,
                    );
                }
                return;
            }
        }
    }

    // In OpenGL ES 1.1's spec, the whole type is implicitly inline math
    let type_ = if spec == "es11" {
        Some(format!("${}$", type_))
    // Absent type (only example is GetUniform, which isn't in OpenGL ES 1.1)
    } else if type_ == "$-$" {
        None
    } else {
        Some(type_.to_string())
    };

    let (get_value, alt_get_value, series, type_) =
        // Match absent get value
        if get_value == "-" || get_value == "--" {
            (None, None, None, type_)
        // Match series like GL_TEXTUREn, GL_CLIP_PLANEn etc.
        } else if let Some(prefix) = get_value.strip_suffix("$i$") {
            let first_get_value = format!("{}0", prefix);
            // Extract minimum count from type
            let (count, type_) = type_.as_deref().unwrap().split_once(" \\times ").unwrap();
            let count = count.strip_prefix('$').unwrap();
            // Handle annoying exception
            let count = count
                .strip_prefix('{')
                .and_then(|count| count.strip_suffix('}'))
                .unwrap_or(count);
            // "*" means "at least"
            let count = count.strip_suffix('*').unwrap();
            let count = parse_quantity(count).unwrap();
            // Ensure LaTeX inline math characters are balanced in type
            let type_ = Some(format!("${}", type_));
            (Some(first_get_value), None, Some(count), type_)
        // Match alternate name
        } else if let Some((get_value, alt_get_value)) = get_value.split_once(" \\hbox{(") {
            let alt_get_value = alt_get_value.strip_suffix(")}").unwrap();
            (
                Some(get_value.to_string()),
                Some(alt_get_value.to_string()),
                None,
                type_,
            )
        } else {
            (Some(get_value), None, None, type_)
        };

    let get_cmnd = if get_cmnd == "\\vbox{\\hbox{{\\bf GetIntegerv},}\\hbox{\\bf GetFloatv}}" {
        // Weird outlier: CURRENT_COLOR has two commands listed in the
        // OpenGL ES spec, unlike every other variable in these three specs.
        // So far as I can tell there's no good reason for this, since even
        // this old spec has a specific color type (C) and there doesn't seem to
        // be any special handling for this variable. The OpenGL 4.6 spec says
        // just GetFloatv, so let's normalise to that.
        assert!(spec == "es11" && get_value.as_deref().unwrap() == "CURRENT_COLOR");
        Some("GetFloatv")
    // Absent get command
    } else if get_cmnd == "--" || get_cmnd == "-" {
        None
    // The old spec doesn't use \glr{}
    } else if spec == "es11" {
        Some(get_cmnd)
    } else {
        Some(
            get_cmnd
                .strip_prefix("\\glr{")
                .unwrap()
                .strip_suffix('}')
                .unwrap(),
        )
    }
    .map(unescape);

    let initial_value = if initial_value == "--" || initial_value == "-" {
        None
    } else {
        let mut initial_value = unescape(initial_value);

        // Replace constants. These are used for things like MAX_DRAW_BUFFERS
        // so that types can be parameterised by them. The "initial value"
        // in this case is the spec's minimum for that constant, and we don't
        // want an unhelpful recursive definition; a different approach is taken
        // in the types code.
        for (name, value) in constants {
            initial_value = initial_value.replace(name, value);
        }

        Some(initial_value)
    };

    // Extract footnote reference for description first, to avoid confusing the
    // code that extracts footnote definitions in the description.
    let (description, description_footnote) = extract_footnote_ref(unescape(description), table);
    let description = description.unwrap();

    // Extract footnote definitions from the description. This should be done
    // before extracting footnote references from the initial value or type,
    // as otherwise they'll panic because they can't find the reference.
    let description = if let Some((description, footnote)) = description
        .split_once("\\fn1")
        .or_else(|| description.split_once("\\footnotemark[1]"))
    {
        let description = description.trim();
        let footnote = footnote.trim();
        // Move this to the table header so we don't need two footnote systems.
        // Only problem is this won't work if there's also table footnotes.
        assert!(table.footnotes.is_empty());
        table.footnotes.push(footnote.to_string());
        description.to_string()
    } else {
        description
    };

    let (initial_value, initial_value_footnote) = initial_value
        .map_or((None, None), |initial_value| {
            extract_footnote_ref(initial_value, table)
        });
    let (type_, type_footnote) =
        type_.map_or((None, None), |type_| extract_footnote_ref(type_, table));

    let attribute = if attribute == "--" || attribute == "-" {
        None
    } else {
        Some(attribute.to_string())
    };

    let type_ = type_.map(|type_| {
        if let Some(parsed_type) = parse_type(&type_) {
            MaybeParsed::Parsed(parsed_type)
        } else {
            MaybeParsed::Unparsed(type_)
        }
    });

    // Note that the section is ignored because we don't have access to the
    // LaTeX source of the full spec, so we can't resolve to a section number.

    push_entry(
        &mut table.entries,
        Entry {
            condition,
            get_value,
            alt_get_value,
            series,
            type_,
            type_footnote,
            get_cmnd,
            initial_value,
            initial_value_footnote,
            description,
            description_footnote,
            attribute,
        },
    );
}

fn parse_spec(spec: &str) -> (String, Vec<Table>) {
    // Read text from file while removing comments
    let mut copyright_text = String::new();
    let mut defs_text = String::new();
    let mut body_text = String::new();
    let file =
        std::fs::File::open(format!("tables_src/gettables.{}.tex", spec)).expect("Can't open file");
    let mut hit_divider = false;
    let mut line_number = 0u32;
    for line in std::io::BufRead::lines(std::io::BufReader::new(file)) {
        let line = line.unwrap();

        if line_number < 3 {
            copyright_text.push_str(line.strip_prefix("% ").unwrap());
            copyright_text.push('\n');
            line_number += 1;
            continue;
        }

        // Split the spec into macro definitions and entries sections using
        // this divider
        if !hit_divider {
            if line == "%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%" {
                hit_divider = true;
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
            if hit_divider {
                body_text.push_str(line);
                body_text.push('\n');
            } else {
                defs_text.push_str(line);
                defs_text.push('\n');
            }
        }
    }

    // Parse definitions of some special constants
    let mut constants = Vec::new();
    let mut text: &str = &defs_text;

    while let Some(offset) = text.find('\\') {
        text = &text[offset..];

        // Constant definition
        if let Some(def_name) = text.strip_prefix("\\def\\m") {
            let (def_name, new_text) = def_name.split_at(def_name.find('{').unwrap());
            let (def_value, new_text) = read_cell(new_text);
            text = new_text;

            constants.push((format!("\\m{}", def_name), def_value.to_string()));
        // ugly hack: the only conditional constant definition (\mtexbasefmt) is
        // one we don't need the value of, so we can stop at this point. this
        // also avoids any confusion with the conditionals inside the table
        // entry macro definitions :)
        } else if text.starts_with("\\if") {
            break;
        } else {
            text = &text[1..];
        }
    }

    // Parse table headers and entries
    let mut tables = Vec::new();
    let mut current_condition: Option<Condition> = None;
    let mut text: &str = &body_text;

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
        // Probably the beginning of a table
        } else if text.starts_with("\\begin") {
            text = &text[text.find('{').unwrap()..];

            let (kind, new_text) = read_cell(text);
            text = new_text;

            if ![
                "statetable",
                "statetableindex",
                "statetabledifferentindexcaption",
            ]
            .contains(&kind)
            {
                continue;
            };

            text = text.strip_prefix("[\\dobar]").unwrap_or(text);

            // OpenGL-only macro
            let (title, caption, label) = if kind == "statetableindex" {
                let (title, new_text) = read_cell(text);
                text = new_text;
                let (caption, new_text) = read_cell(text);
                text = new_text;
                let (label, new_text) = read_cell(text);
                text = new_text;
                (unescape(title), Some(unescape(caption)), label)
            // OpenGL ES-only macro
            } else if kind == "statetabledifferentindexcaption" {
                // TODO: remove title from caption
                let (caption, new_text) = read_cell(text);
                text = new_text;
                let (label, new_text) = read_cell(text);
                text = new_text;
                let (title, new_text) = read_cell(text);
                text = new_text;
                let title = unescape(title);
                let caption = unescape(caption).strip_prefix(&title).unwrap().to_string();
                (title, Some(caption), label)
            // Common macro
            } else {
                assert!(kind == "statetable");
                let (title, new_text) = read_cell(text);
                text = new_text;
                let (label, new_text) = read_cell(text);
                text = new_text;
                if let Some((title, caption)) = title.split_once('\n') {
                    (unescape(title), Some(unescape(caption)), label)
                // Special hack for “Lighting (see also …)” in ES 1.1 spec to
                // make it consistent with GL 4.6.
                } else if title.contains(" (see also") {
                    let (title, caption) = title.split_once(' ').unwrap();
                    (unescape(title), Some(unescape(caption)), label)
                } else {
                    (unescape(title), None, label)
                }
            };

            // Extract footnotes from the caption
            let mut footnotes = Vec::new();
            let caption = if caption
                .as_deref()
                .map_or(false, |caption| caption.contains("\\fn"))
            {
                let mut remaining: &str = caption.as_deref().unwrap();
                loop {
                    remaining = remaining.trim_start();
                    let (footnote, new_remaining) = if remaining.starts_with("{\\par") {
                        read_cell(remaining)
                    } else if remaining.starts_with("\\par") {
                        let footnote_end = remaining[1..]
                            .find("\\par")
                            .map_or(remaining.len(), |i| i + 1);
                        remaining.split_at(footnote_end)
                    } else {
                        assert!(remaining.is_empty());
                        break;
                    };
                    remaining = new_remaining;

                    let footnote = footnote
                        .strip_prefix("\\par")
                        .unwrap()
                        .trim_start()
                        .strip_prefix(match footnotes.len() {
                            0 => "\\fn1",
                            1 => "\\fnb",
                            _ => unimplemented!(),
                        })
                        .unwrap()
                        .trim_start();
                    footnotes.push(footnote.to_string());
                }
                None
            } else {
                caption
            };

            tables.push(Table {
                title,
                caption,
                label: label.to_string(),
                footnotes,
                entries: Vec::new(),
            });
            continue;
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

        process_row(
            spec,
            condition,
            cells,
            &constants,
            tables.last_mut().unwrap(),
        );
    }

    (copyright_text, tables)
}

fn class_for_condition(condition: &Option<Condition>) -> &str {
    match condition {
        Some(Condition::Compatibility) => "compatibility-only",
        Some(Condition::Core) => "core-only",
        Some(Condition::ImagingSubset) => "imaging-subset",
        None => "no-condition",
    }
}

fn print_table(table: &Table) {
    fn footnote_name(table: &Table, index: usize) -> String {
        format!("{}-fn-{}", table.label, index)
    }
    fn footnote_symbol(index: usize) -> char {
        ['†', '‡'][index]
    }
    fn reference_footnote(table: &Table, index: usize) {
        print!(
            "<sup><a href=\"#{}\">{}</a></sup>",
            footnote_name(table, index),
            footnote_symbol(index)
        );
    }

    // special classes for filtering only
    let mut section_classes = String::from("section-header ");
    for entry in &table.entries {
        let class = class_for_condition(&entry.condition);
        if section_classes.contains(class) {
            continue;
        }
        section_classes.push(' ');
        section_classes.push_str("has-");
        section_classes.push_str(class);
    }

    println!("<tr class=\"{}\">", section_classes);
    println!("<td colspan=6>");
    println!("<h2 name=\"{}\">{}</h2>", table.label, table.title);
    if let Some(ref caption) = table.caption {
        println!("<p>{}</p>", caption);
    }
    if !table.footnotes.is_empty() {
        println!("<ol>");
        for (index, footnote) in table.footnotes.iter().enumerate() {
            println!(
                "<li id=\"{}\">{} {}</li>",
                footnote_name(table, index),
                footnote_symbol(index),
                footnote
            );
        }
        println!("</ol>");
    }
    println!("</td>");
    println!("</tr>");

    for entry in &table.entries {
        println!("<tr class={}>", class_for_condition(&entry.condition));

        print!("<td>");
        if let Some(ref get_value) = entry.get_value {
            print!("<code>{}</code>", get_value);
        } else {
            print!("—");
        }
        if let Some(ref alt_get_value) = entry.alt_get_value {
            print!(" <em>or</em><br> <code>{}</code>", alt_get_value);
        }
        if let Some(ref minimum) = entry.series {
            let first_value = entry.get_value.as_deref().unwrap();
            print!(
                " …<br><code>{}</code> + (<var>n</var>-1)<br>where <var>n</var> ≥ ",
                first_value
            );
            print_quantity(minimum);
        }
        println!("</td>");

        print!("<td>");
        if entry.series.is_some() {
            print!("<var>n</var> × ");
        }
        if let Some(ref type_) = entry.type_ {
            match type_ {
                MaybeParsed::Parsed(t) => print_type(t),
                MaybeParsed::Unparsed(s) => print!("{}", s),
            }
        } else if entry.type_footnote.is_none() {
            print!("—");
        }
        if let Some(footnote_index) = entry.type_footnote {
            reference_footnote(table, footnote_index);
        }
        println!("</td>");

        if let Some(ref get_cmnd) = entry.get_cmnd {
            println!("<td><code>{}</code></td>", get_cmnd);
        } else {
            println!("<td>—</td>");
        }

        print!("<td>");
        if let Some(ref initial_value) = entry.initial_value {
            print!("{}", initial_value);
        } else if entry.initial_value_footnote.is_none() {
            print!("—");
        }
        if let Some(footnote_index) = entry.initial_value_footnote {
            reference_footnote(table, footnote_index);
        }
        println!("</td>");

        print!("<td>");
        print!("{}", entry.description);
        if let Some(footnote_index) = entry.description_footnote {
            reference_footnote(table, footnote_index);
        }
        println!("</td>");

        if let Some(ref attribute) = entry.attribute {
            println!("<td>{}</td>", attribute);
        } else {
            println!("<td>—</td>");
        }

        println!("</tr>");
    }
}

fn main() {
    println!(
        "{}",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/header.html"))
    );
    let mut copyrights = String::new();
    for (suffix, name) in [
        ("es11", "OpenGL ES 1.1"),
        ("es", "OpenGL ES 3.2"),
        ("gl", "OpenGL 4.6"),
    ] {
        let (spec_copyright, tables) = parse_spec(suffix);
        use std::fmt::Write;
        write!(
            copyrights,
            "{} specification acknowledgments:<br><pre>{}</pre>",
            name, spec_copyright
        )
        .unwrap();
        println!("<details>");
        println!("<summary><h2>{} state tables</h2></summary>", name);
        println!("<table>");
        println!("<thead>");
        println!("<tr>");
        println!("<th>Get value</th>");
        println!("<th>Type</th>");
        println!("<th>Get command</th>");
        println!("<th>Initial value</th>");
        println!("<th>Description</th>");
        println!("<th>Attribute</th>");
        println!("</tr>");
        println!("</thead>");
        println!("<tbody>");
        for table in tables {
            print_table(&table);
        }
        println!("</tbody>");
        println!("</table>");
        println!("</details>");
    }
    println!("<hr>");
    println!("<p><strong>This page is not produced by the Khronos Group, and cannot substitute for the Khronos Group specifications.</strong> This page is an independently created composite and reinterpretation that may contain inaccuracies; you rely on it at your own risk. Always consult <a href=\"https://registry.khronos.org/OpenGL/\">the Khronos Group specifications</a>. OpenGL® and OpenGL ES™ are trademarks used under license by the Khronos Group.</strong></p>");
    println!("{}", copyrights);
    println!("<p><a href=\"https://github.com/hikari-no-yume/OpenGL-state-table-parser\">OpenGL-state-table-parser</a> © 2023 hikari_no_yume. The content of this page may be redistributed under <a href=\"https://spdx.org/licenses/CC-BY-4.0.html\">CC BY 4.0</a>.</p>");
}
