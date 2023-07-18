OpenGL state table parser
=========================

This is an **unofficial** project with **no guarantee of accuracy** that attempts to parse the LaTeX markup for the OpenGL specifications' state table sections, which [the Khronos Group graciously released on my request](https://github.com/KhronosGroup/OpenGL-Registry/issues/571). It attempts to wrangle some kind of order from chaos and assign machine-readable meaning to most of the content, which is a lot harder than it sounds.

It currently produces only an HTML output, but it has a clean separation between the parsing and the HTML generation, so it would be easy to add JSON output in future. The goal is to provide something that both humans and machines find easier to read than the official PDFs.

Things this does right now:

* Produces nice clean semantic HTML, with rapid filtering options, hyperlinks for footnotes, and `<abbr>` tags to remind you what the type symbols mean.
* Extracts clean “Get value”, “Get command” and “Attribute” fields.
* Extracts footnotes from descriptions and table headers.
* Parses almost all types.
* Tracks which rows are conditional on particular profiles.
* Normalises all rows' “Get values” so they are either part of a series (e.g. `GL_LIGHT0 … GL_LIGHT7`) or have exactly one associated constant (e.g. `GL_TEXTURE_BINDING_2D`).
* Where something _can't_ be parsed, the original LaTeX is preserved.

Things this does not do yet:

* Parse the LaTeX markup within footnotes or the “Initial value” and “Description” fields, except insofar as is needed for features mentioned above.
* Correct for most typos in the specs. There are many and I haven't kept track of them :(

Things this will probably never do:

* Hyperlink to sections of the spec. This would be a really nice feature, but I have no way to resolve the target names in the state table LaTeX to page numbers or similar for Khronos's published PDFs. This is also why the section number column is missing.

License
-------

The spec inputs and the HTML output are [`CC-BY-4.0`](https://spdx.org/licenses/CC-BY-4.0.html).

The parser code itself is [`MPL-2.0`](https://spdx.org/licenses/MPL-2.0.html), © 2023 hikari\_no\_yume. However, any text that appears in the HTML output is _also_ `CC-BY-4.0`, as is any text taken from the Khronos specs, for which the copyright lies with the Khronos Group.

Usage
-----

```sh
cargo run > out.html
```
