use minijinja::{AutoEscape, Environment, Value};
use pulldown_cmark::{Options, Parser, html};

fn markdown_filter(value: &str) -> Value {
    let options = Options::empty();
    let parser = Parser::new_ext(value, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    // Sanitize HTML output to prevent XSS: from_safe_string bypasses MiniJinja auto-escaping,
    // so the HTML must be clean before being marked safe. pulldown-cmark passes raw inline HTML
    // from the markdown source verbatim, so a description containing <script> or event handler
    // attributes would execute in every viewer's browser (stored XSS).
    let sanitized = ammonia::clean(&html_output);
    Value::from_safe_string(sanitized)
}

pub fn make_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    env.add_filter("markdown", markdown_filter);
    env.add_template("base.html", include_str!("templates/base.html"))
        .unwrap();
    env.add_template("board.html", include_str!("templates/board.html"))
        .unwrap();
    env.add_template("issue_list.html", include_str!("templates/issue_list.html"))
        .unwrap();
    env.add_template("issue.html", include_str!("templates/issue.html"))
        .unwrap();
    env.add_template("graph.html", include_str!("templates/graph.html"))
        .unwrap();
    env
}
