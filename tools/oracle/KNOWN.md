# Known differences from Mermaid's parsers

`diff.mjs` counts the inputs listed here but does not report them. Each entry is a line of the
form `- <type> "<input as JSON>"`, under a heading that says why pele-rs differs.

## Node's URL parser leaves dot segments after a segment that starts with a dot

`@braintree/sanitize-url` runs `new URL()`. In Node (whose parser is ada), a path with a
segment that starts with a dot, such as `.b` or `.;.`, keeps its `.` and `..` segments; the
WHATWG URL standard, which browsers and pele-rs follow, resolves them. `diff.mjs` recognizes
these by resolving the segments of Mermaid's result itself, so they are not listed one by one.

## Punycode labels that IDNA's tables refuse

pele-rs checks that an `xn--` label is valid Punycode and decodes to no control, space or
ASCII character, but does not carry the Unicode tables of UTS #46 that would refuse the rest.

- url "HTTPS://xn--Example.COM/a/b?c=d#e"
