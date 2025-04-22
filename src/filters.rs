use askama::filters::Safe;
use comrak::Options;

pub fn markdown<'a, T: std::fmt::Display>(s: T) -> askama::Result<Safe<String>> {
    let mut options = Options::default();
    options.extension.footnotes = true;
    options.extension.table = true;
    options.extension.header_ids = Some("content-".to_string());
    options.extension.strikethrough = true;
    options.extension.tagfilter = true;
    options.extension.autolink = true;
    options.render.escape = true;

    Ok(Safe(comrak::markdown_to_html(&s.to_string(), &options)))
}
