#set page(paper: "a4", margin: 2cm)
#set text(font: "Liberation Serif", size: 12pt)
#set heading(numbering: "1.")

= Link Extraction Test Document

This document contains various types of links to test the link extraction functionality in Miro PDF viewer.

== External Web Links

Here are some external web links:

- Visit #link("https://www.rust-lang.org")[Rust Programming Language]
- Check out #link("http://example.com")[Example Website]
- Learn about #link("https://github.com/typst/typst")[Typst on GitHub]
- Documentation at #link("https://typst.app/docs")[Typst Documentation]

== Email Links

Contact information:

- Send feedback to #link("mailto:feedback@example.com")[feedback\@example.com]
- Technical support: #link("mailto:support@example.com?subject=Bug%20Report")[support\@example.com]
- General inquiries: #link("mailto:info@example.com")[info\@example.com]

== Internal Page References

This document has multiple pages. Here are some internal references:

- Go to @section-text[Section with Regular Text]
- See @conclusion[Conclusion]
- Reference to @links-in-tables[Links in Tables]

== Regular Text (No Links)

This section contains regular text without any links to test that the link extractor doesn't create false positives.

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.

Some text that looks like URLs but isn't linked:
- `www.example.com` (not a link)
- `https://not-a-link.com` (just text)
- `email@domain.com` (plain text email)

== Mixed Content

Here's a paragraph with both links and regular text: The #link("https://typst.app")[Typst] project is a modern typesetting system. You can find more information at their website, or contact them via #link("mailto:hello@typst.app")[email]. Regular text continues here without links.

#pagebreak()
#set page(flipped: true)

= Second Page Content <section-text>

This is the second page with more content to test multi-page link extraction.

== Links in Different Contexts

=== Links in Lists

1. First item with #link("https://www.wikipedia.org")[Wikipedia link]
2. Second item with regular text
3. Third item with #link("mailto:test@example.org")[email link]

=== Links in Tables <links-in-tables>

#table(
  columns: 3,
  [*Name*], [*Website*], [*Contact*],
  [Rust], [#link("https://www.rust-lang.org")[rust-lang.org]], [#link("mailto:community@rust-lang.org")[Email]],
  [Typst], [#link("https://typst.app")[typst.app]], [#link("mailto:contact@typst.app")[Email]],
  [Example], [Regular text], [No link here],
)

== Code Blocks and Verbatim Text

Code blocks should not contain active links:

```rust
// This URL should not be a link: https://example.com
let url = "https://not-a-link.com";
println!("Email: user@domain.com");
```

Raw text: `https://raw-text-url.com` and `email@raw.com`

== Special Characters in Links

Links with special characters and parameters:

- Search query: #link("https://www.google.com/search?q=typst+pdf")[Google Search for Typst]
- URL with fragment: #link("https://example.com/page#section1")[Page with anchor]
- Complex email: #link("mailto:user+tag@example.com?subject=Test&body=Hello")[Complex email link]

#pagebreak()

= Third Page <conclusion>

== Conclusion

This test document contains:

- *External HTTP/HTTPS links*: Should appear with blue hitboxes
- *Email links*: Should appear with orange hitboxes  
- *Internal page references*: Should appear with green hitboxes (if supported)
- *Regular text*: Should have no hitboxes

== Final Test Links

Last set of test links:

- #link("https://docs.rs")[Rust Documentation]
- #link("mailto:final@test.com")[Final Email Test]
- Back to @section-text[Second Page]

== Non-Link Text

This final section contains only regular text to ensure the link extractor properly distinguishes between linked and non-linked content.

The quick brown fox jumps over the lazy dog. This sentence contains no links whatsoever. Neither does this one. Or this one.

Some URLs that are NOT links:
- `www.not-linked.com`
- `https://plain-text-url.org`  
- `contact@not-a-link.email`

*End of test document.*
