/// Some fields can be parsed into a structured form, but this won't always
/// succeed. This enum is used in such cases: it either contains the parsed form
/// (`T`) or an unparsed [String].
#[derive(Debug, PartialEq, Eq)]
pub enum MaybeParsed<T> {
    Parsed(T),
    Unparsed(String),
}

/// A parsed representation of a type.
#[derive(Debug, PartialEq, Eq)]
pub struct Type {
    /// The basic type, which is usually but not always a scalar.
    basic_type: BasicType,
    /// How many copies of the basic type? If this is empty, there's just one.
    /// The OpenGL specs notate this with "n × type". Each element in this array
    /// is another multiplication (so `[a, b, c]` means "a × b × c × type").
    /// The boolean indicates whether this is a minimum ("n* × type").
    quantity: Vec<MaybeParsed<(Quantity, bool)>>,
}

/// A parsed representation of a type code. The descriptions here come from the
/// OpenGL 4.6 spec.
#[derive(Debug, PartialEq, Eq)]
enum BasicType {
    /// _B_: Boolean
    Boolean,
    /// _BMU_: Basic machine units
    Bmu,
    /// _C_: Color (floating-point R, G, B, and A values)
    Color,
    /// _E_: Enumerated value
    ///
    /// This doesn't exist in the OpenGL ES 1.1 spec, it uses variations on _Z_
    /// instead.
    Enum,
    /// _CI_: Color index (floating-point index value)
    ///
    /// This only exists in the OpenGL compatibility profile.
    ColorIndex,
    /// _T_: Texture coordinates (floating-point (_s_, _t_, _r_, _q_) values)
    ///
    /// This only exists in the OpenGL compatibility profile and OpenGL ES 1.1.
    TexCoords,
    /// _N_: Normal coordinates (floating-point (_x_, _y_, _z_) values)
    ///
    /// This only exists in the OpenGL compatibility profile and OpenGL ES 1.1.
    NormalCoords,
    /// _V_: Vertex, including associated data
    ///
    /// This only exists in the OpenGL compatibility profile and OpenGL ES 1.1.
    Vertex,
    /// _Z_: Integer
    Integer,
    /// _Z_ (with superscript plus sign): Non-negative integer
    ///
    /// The OpenGL 4.6 and OpenGL ES 1.1 specs also say this is used for
    /// enumerated values.
    NonNegativeInteger,
    /// _Z_ (with subscript constant _k_): _k_-valued integer, or
    /// _Z_ (with subscript constant _k_ followed by asterisk): the same but
    /// _k_ is a minimum.
    KValuedInteger { k: Quantity, minimum: bool },
    /// _R_: Floating-point number
    Float,
    /// _R_ (with superscript plus sign): Non-negative floating-point number
    NonNegativeFloat,
    /// _R_ (with superscript [0, 1]): Floating-point number in the range [0, 1]
    ///
    /// The spec describes this more generally (range [_a_, _b_]) but [0, 1] is
    /// the only range that's actually used.
    ZeroOneRangeFloat,
    /// _R_ (with superscript _k_): _k_-tuple of floating-point numbers
    FloatTuple { k: u32 },
    /// _R_ (with subscript _k_): _k_-valued floating-point number
    ///
    /// This only exists in OpenGL ES 1.1.
    KValuedFloat { k: u32 },
    /// _P_: Position (_x_, _y_, _z_, _w_ floating-point coordinates)
    ///
    /// This only exists in the OpenGL compatibility profile and OpenGL ES 1.1.
    Position,
    /// _D_: Direction (_x_, _y_, _z_ floating-point coordinates)
    ///
    /// This only exists in the OpenGL compatibility profile and OpenGL ES 1.1.
    Direction,
    /// _M_ (with superscript 4): 4 × 4 floating-point matrix
    ///
    /// This only exists in the OpenGL compatibility profile and OpenGL ES 1.1.
    Matrix,
    /// _S_: null-terminated string
    ///
    /// This does not exist in OpenGL ES 1.1.
    String,
    /// _I_: Image
    Image,
    /// _A_: Attribute stack entry, including mask
    ///
    /// This only exists in the OpenGL compatibility profile.
    AttributeStackEntry,
    /// _Y_: Pointer (data type unspecified)
    Pointer,
    /// `char`
    ///
    /// This isn't in the table of type codes!
    Char,
}

/// Parsed representation of a quantity.
#[derive(Debug, PartialEq, Eq)]
pub enum Quantity {
    /// Simple integer quantity.
    Integer(u32),
    /// A specification-defined constant that varies with the implementation,
    /// e.g. `MAX_DRAW_BUFFERS`.
    Constant(&'static str),
}
impl std::fmt::Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Quantity::Integer(n) => write!(f, "{}", n),
            Quantity::Constant(c) => write!(f, "{}", c),
        }
    }
}

pub fn parse_quantity(quantity: &str) -> Option<Quantity> {
    quantity
        .parse::<u32>()
        .ok()
        .map(Quantity::Integer)
        .or(match quantity {
            "\\mtexcoord" => Some(Quantity::Constant("MAX_TEXTURE_COORDS")),
            "\\mtexnum" | "\\mtexunit" => Some(Quantity::Constant("MAX_TEXTURE_UNITS")),
            "\\mteximage" => Some(Quantity::Constant("MAX_COMBINED_TEXTURE_IMAGE_UNITS")),
            "\\mimageunit" => Some(Quantity::Constant("MAX_IMAGE_UNITS")),
            "\\mvtxattr" => Some(Quantity::Constant("MAX_VERTEX_ATTRIBS")),
            "\\mdrawbuf" => Some(Quantity::Constant("MAX_DRAW_BUFFERS")),
            // this is actually "MAX_<stage>_UNIFORM_BLOCKS", but their definitions
            // are identical?
            "\\mblockstage" => Some(Quantity::Constant("MAX_VERTEX_UNIFORM_BLOCKS")),
            "\\mblockcombined" => Some(Quantity::Constant("MAX_COMBINED_UNIFORM_BLOCKS")),
            // this is the number of stages defined by OpenGL, which is 6 in GL 4.6
            // and GL ES 3.2 and doesn't have a constant
            "\\mprogstage" => Some(Quantity::Integer(6)),
            _ => None,
        })
}

fn parse_basic_type(basic_type: &str) -> Option<BasicType> {
    match basic_type {
        "B" => Some(BasicType::Boolean),
        "BMU" => Some(BasicType::Bmu),
        "C" => Some(BasicType::Color),
        "\\Enum" => Some(BasicType::Enum),
        "CI" => Some(BasicType::ColorIndex),
        "T" => Some(BasicType::TexCoords),
        "N" => Some(BasicType::NormalCoords),
        "V" => Some(BasicType::Vertex),
        "Z" => Some(BasicType::Integer),
        "Z+" | "Z^+" | "Z^{+}" | "\\Zplus" => Some(BasicType::NonNegativeInteger),
        "R" => Some(BasicType::Float),
        "R^{+}" => Some(BasicType::NonNegativeFloat),
        "R^{[0,1]}" => Some(BasicType::ZeroOneRangeFloat),
        "P" => Some(BasicType::Position),
        "D" => Some(BasicType::Direction),
        "M^{4}" => Some(BasicType::Matrix),
        "S" => Some(BasicType::String),
        "I" => Some(BasicType::Image),
        "A" => Some(BasicType::AttributeStackEntry),
        "Y" => Some(BasicType::Pointer),
        "\\glt{char}" => Some(BasicType::Char),
        _ => basic_type
            .strip_prefix("Z_")
            .map(|k| {
                k.strip_prefix('{')
                    .and_then(|k| k.strip_suffix('}'))
                    .unwrap_or(k)
            })
            .and_then(|k| {
                let (minimum, k) = if let Some(k) = k.strip_suffix('*') {
                    (true, k)
                } else {
                    (false, k)
                };
                parse_quantity(k).map(|k| {
                    // ignore minimum suffix for constants, because in the spec
                    // that's intended to express the minimum value of the constant,
                    // not that the quantity here has the constant as a minimum
                    let minimum = minimum && !matches!(k, Quantity::Constant(_));
                    BasicType::KValuedInteger { k, minimum }
                })
            })
            .or_else(|| {
                basic_type.strip_prefix("R_").and_then(|k| {
                    parse_quantity(k).map(|k| {
                        let Quantity::Integer(k) = k else { panic!() };
                        BasicType::KValuedFloat { k }
                    })
                })
            })
            .or_else(|| {
                basic_type
                    .strip_prefix("R^")
                    .map(|k| {
                        k.strip_prefix('{')
                            .and_then(|k| k.strip_suffix('}'))
                            .unwrap_or(k)
                    })
                    .and_then(|k| {
                        parse_quantity(k).map(|k| {
                            let Quantity::Integer(k) = k else { panic!() };
                            BasicType::FloatTuple { k }
                        })
                    })
            }),
    }
}

pub fn print_quantity(quantity: &Quantity) {
    match quantity {
        Quantity::Integer(n) => print!("{}", n),
        Quantity::Constant(c) => print!("<code>{}</code>", c),
    }
}

fn print_basic_type(basic_type: &BasicType) {
    match basic_type {
        BasicType::Boolean => print!("<abbr title=\"Boolean\">B</abbr>"),
        BasicType::Bmu => print!("<abbr title=\"Basic machine units\">BMU</abbr>"),
        BasicType::Color => print!("<abbr title=\"Color\">C</abbr>"),
        BasicType::Enum => print!("<abbr title=\"Enumerated value\">E</abbr>"),
        BasicType::ColorIndex => print!("<abbr title=\"Color index\">CI</abbr>"),
        BasicType::TexCoords => print!("<abbr title=\"Texture coordinates\">T</abbr>"),
        BasicType::NormalCoords => print!("<abbr title=\"Normal coordinates\">N</abbr>"),
        BasicType::Vertex => print!("<abbr title=\"Vertex\">V</abbr>"),
        BasicType::Integer => print!("<abbr title=\"Integer\">Z</abbr>"),
        BasicType::NonNegativeInteger => {
            print!("<abbr title=\"Non-negative integer\">Z<sup>+</sup></abbr>")
        }
        BasicType::KValuedInteger { k, minimum: false } => {
            print!("<abbr title=\"{}-valued integer\">Z<sub>", k);
            print_quantity(k);
            print!("</sub></abbr>");
        }
        BasicType::KValuedInteger { k, minimum: true } => {
            print!(
                "<abbr title=\"{}-valued integer ({} is a minimum)\">Z<sub>",
                k, k
            );
            print_quantity(k);
            print!("*</sub></abbr>");
        }
        BasicType::Float => print!("<abbr title=\"Floating-point number\">R</abbr>"),
        BasicType::NonNegativeFloat => {
            print!("<abbr title=\"Non-negative floating-point number\">R<sup>+</sup></abbr>")
        }
        BasicType::ZeroOneRangeFloat => print!(
            "<abbr title=\"Floating-point number in the range [0,1]\">R<sup>[0,1]</sup></abbr>"
        ),
        BasicType::FloatTuple { k } => print!(
            "<abbr title=\"{}-tuple of floating-point numbers\">R<sup>{}</sup></abbr>",
            k, k
        ),
        BasicType::KValuedFloat { k } => print!(
            "<abbr title=\"{}-valued floating-point number\">R<sub>{}</sub></abbr>",
            k, k
        ),
        BasicType::Position => print!("<abbr title=\"Position\">P</abbr>"),
        BasicType::Direction => print!("<abbr title=\"Direction\">D</abbr>"),
        BasicType::Matrix => {
            print!("<abbr title=\"4 × 4 floating-point matrix\">M<sup>4</sup></abbr>")
        }
        BasicType::String => print!("<abbr title=\"Null-terminated string\">S</abbr>"),
        BasicType::Image => print!("<abbr title=\"Image\">I</abbr>"),
        BasicType::AttributeStackEntry => print!("<abbr title=\"Attribute stack entry\">A</abbr>"),
        BasicType::Pointer => print!("<abbr title=\"Pointer\">Y</abbr>"),
        BasicType::Char => print!("<code>char</code>"),
    }
}

pub fn parse_type(type_: &str) -> Option<Type> {
    let type_ = type_.strip_prefix('$').unwrap().strip_suffix('$').unwrap();

    let mut quantity = Vec::new();
    let mut basic_type = type_;
    while let Some((term, rest)) = basic_type.split_once(" \\times ") {
        basic_type = rest;

        let (unsuffixed, minimum) = if let Some(u) = term.strip_suffix('*') {
            (u, true)
        } else {
            (term, false)
        };
        quantity.push(match parse_quantity(unsuffixed) {
            Some(parsed) => {
                // ignore minimum suffix for constants, because in the spec
                // that's intended to express the minimum value of the constant,
                // not that the quantity here has the constant as a minimum
                let minimum = minimum && !matches!(parsed, Quantity::Constant(_));
                MaybeParsed::Parsed((parsed, minimum))
            }
            None => MaybeParsed::Unparsed(term.to_string()),
        });
    }

    if parse_basic_type(basic_type).is_none() {
        eprintln!("Couldn't parse basic type: {:?}", basic_type);
    }

    Some(Type {
        basic_type: parse_basic_type(basic_type)?,
        quantity,
    })
}

pub fn print_type(type_: &Type) {
    let Type {
        basic_type,
        quantity,
    } = type_;
    for term in quantity {
        match term {
            MaybeParsed::Parsed((term, minimum)) => {
                print_quantity(term);
                if *minimum {
                    print!("<abbr title=\"quantity is a minimum\">*</abbr>");
                }
            }
            MaybeParsed::Unparsed(term) => print!("<code>{}</code>", term),
        }
        print!(" × ");
    }
    print_basic_type(basic_type);
}
