use minijinja::{AutoEscape, Environment};

pub fn make_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
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
