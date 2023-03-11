macro_rules! formatter {
    ($name:ident, $fmt:literal) => {
        pub struct $name<'a>(pub &'a str);
        impl<'a> std::fmt::Display for $name<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $fmt, self.0)
            }
        }
    };
}

formatter!(
    UnderlineError,
    "<span underline='error' underline_color='red'>{}</span>"
);
formatter!(Monospace, "<tt>{}</tt>");

/// `(color, text)`
pub struct FgColor<'a, 'b>(pub &'a str, pub &'b str);

impl<'a, 'b> std::fmt::Display for FgColor<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<span fgcolor='{}'>{}</span>", self.0, self.1)
    }
}

/// Removes markup tags from the given string.
pub fn remove_markup(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    result
}
