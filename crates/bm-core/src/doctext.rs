//! Document text as it is kept and as the editor shows it.
//!
//! A document is running text with its formatting codes inline, written [code] (codes can nest: [hl4[es~para.]]);
//! it holds no line breaks of its own: a line ends where a code says so, and pages are laid out from the codes.
//! The editor shows the codes in brackets and reads line by line: the new-line code [<] is shown as a line break,
//! which is also what Enter puts in (Enter inserts [<], as in the reference program), and the codes that start a
//! new line of their own ([p] new paragraph, [l] new line if not at the start of one) are followed by a line break.

/// Codes after which the editor starts a new line.
pub fn breaks_line(code: &str) -> bool {
    matches!(code, "p" | "l")
}

/// The new-line code, shown as a line break.
const NEW_LINE: &str = "<";

/// Running text to the editor's form: [<] becomes a line break, and [p] and [l] are followed by one.
pub fn to_display(running: &str) -> String {
    let mut out = String::with_capacity(running.len() + running.len() / 16);
    let mut open: Vec<usize> = Vec::new();
    for c in running.chars() {
        match c {
            '[' => {
                open.push(out.len());
                out.push('[');
            }
            ']' if !open.is_empty() => {
                let start = open.pop().unwrap_or(0);
                if open.is_empty() && &out[start + 1..] == NEW_LINE {
                    out.truncate(start);
                    out.push('\n');
                } else {
                    let breaks = open.is_empty() && breaks_line(&out[start + 1..]);
                    out.push(']');
                    if breaks {
                        out.push('\n');
                    }
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// The editor's form to running text: a line break right after [p] or [l] is the editor's own, any other line
/// break is the new-line code [<].
pub fn to_running(display: &str) -> String {
    let mut out = String::with_capacity(display.len() + display.len() / 16);
    let mut open: Vec<usize> = Vec::new();
    // Whether the text so far ends with a code that starts a new line of its own.
    let mut after_break_code = false;
    for c in display.chars() {
        match c {
            '\n' if after_break_code => after_break_code = false,
            '\n' => out.push_str("[<]"),
            '[' => {
                open.push(out.len());
                out.push('[');
                after_break_code = false;
            }
            ']' if !open.is_empty() => {
                let start = open.pop().unwrap_or(0);
                after_break_code = open.is_empty() && breaks_line(&out[start + 1..]);
                out.push(']');
            }
            c => {
                out.push(c);
                after_break_code = false;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_line_codes_are_line_breaks_in_the_editor() {
        assert_eq!(to_display("One[<]Two[<][<]Three"), "One\nTwo\n\nThree");
        assert_eq!(to_running("One\nTwo\n\nThree"), "One[<]Two[<][<]Three");
    }

    #[test]
    fn paragraph_and_line_codes_are_followed_by_a_line_break_that_is_not_a_code() {
        let running = "[es~h1.]Title[ee~h1.][p]First[l]Second[p][<]Third";
        let shown = "[es~h1.]Title[ee~h1.][p]\nFirst[l]\nSecond[p]\n\nThird";
        assert_eq!(to_display(running), shown);
        assert_eq!(to_running(shown), running);
    }

    #[test]
    fn nested_codes_and_brackets_without_a_pair_are_kept() {
        let running = "[hl4[es~para.]]Entry[<]a [b";
        assert_eq!(to_display(running), "[hl4[es~para.]]Entry\na [b");
        assert_eq!(to_running(&to_display(running)), running);
    }

    #[test]
    fn a_line_typed_after_a_paragraph_code_on_the_same_line_ends_with_a_new_line_code() {
        assert_eq!(to_running("[p]Text\nMore"), "[p]Text[<]More");
    }
}
