/// A parsed representation of a type.
#[derive(Debug, PartialEq, Eq)]
pub struct Type {
    /// The basic type, which is usually but not always a scalar.
    basic_type: BasicType,
    ///// How many copies of the basic type? If this is [None], there's just one.
    ///// The OpenGL specs notate this with `n × type`.
    //quantity: Option<MaybeParsed<TypeQuantity>>,
    quantity: Option<String>,
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
    KValuedInteger { k: u32, minimum: bool },
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

fn parse_quantity(quantity: &str) -> Option<u32> {
    quantity.parse::<u32>().ok()
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
                parse_quantity(k).map(|k| BasicType::KValuedInteger { k, minimum })
            })
            .or_else(|| {
                basic_type
                    .strip_prefix("R_")
                    .and_then(|k| parse_quantity(k).map(|k| BasicType::KValuedFloat { k }))
            })
            .or_else(|| {
                basic_type
                    .strip_prefix("R^")
                    .map(|k| {
                        k.strip_prefix('{')
                            .and_then(|k| k.strip_suffix('}'))
                            .unwrap_or(k)
                    })
                    .and_then(|k| parse_quantity(k).map(|k| BasicType::FloatTuple { k }))
            }),
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
        BasicType::KValuedInteger { k, minimum: false } => print!(
            "<abbr title=\"{}-valued integer\">Z<sub>{}</sub></abbr>",
            k, k
        ),
        BasicType::KValuedInteger { k, minimum: true } => print!(
            "<abbr title=\"{}-valued integer ({} is a minimum)\">Z<sub>{}*</sub></abbr>",
            k, k, k
        ),
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

    let (quantity, basic_type) =
        if let Some((quantity, basic_type)) = type_.rsplit_once(" \\times ") {
            (Some(quantity), basic_type)
        } else {
            (None, type_)
        };

    if parse_basic_type(basic_type).is_none() {
        eprintln!("Couldn't parse basic type: {:?}", basic_type);
    }

    Some(Type {
        basic_type: parse_basic_type(basic_type)?,
        quantity: quantity.map(|q| q.to_string()),
    })
}

pub fn print_type(type_: &Type) {
    let Type {
        basic_type,
        quantity,
    } = type_;
    if let Some(quantity) = quantity {
        print!("<code>{}</code> × ", quantity);
    }
    print_basic_type(basic_type);
}
